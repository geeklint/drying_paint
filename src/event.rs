/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::VecDeque;

use super::{
    WatchedMeta,
    WatcherInit, Watcher, WatcherMeta,
};

enum Container<T> {
    Fresh(T),
    Held,
    None,
}

struct AlternatingData<T> {
    queue: VecDeque<T>,
    current_data: Container<T>,
    current_trigger: WatchedMeta,
    off_frame: WatchedMeta,
}

impl<T: 'static> WatcherInit for AlternatingData<T> {
    fn init(watcher: &mut WatcherMeta<Self>) {
        watcher.watch(|data| {
            data.off_frame.watched();
            let next = data.queue.pop_front();
            data.current_data = if let Some(item) = next {
                Container::Fresh(item)
            } else {
                Container::None
            };
            data.current_trigger.trigger();
        });

        watcher.watch(|data| {
            data.current_trigger.watched();
            let trigger = match data.current_data {
                Container::None => false,
                _ => true,
            };
            if trigger {
                data.off_frame.trigger();
            }
        });
    }
}

impl<T> Default for AlternatingData<T> {
    fn default() -> Self {
        AlternatingData {
            queue: VecDeque::new(),
            current_data: Container::None,
            current_trigger: WatchedMeta::new(),
            off_frame: WatchedMeta::new(),
        }
    }
}

/// A WatchedEvent uses the watch system provided by this crate to implement
/// an event disptacher. This is different from a watched value
/// ([Watched](struct.Watched.html)) in that events will fire for each value
/// passed to WatchedEvent::dispatch() and will not "store" the data.
/// A `bind_event` macro is provided for convience, and is the preferred way
/// to watch an event:
///
/// ```rust
/// use drying_paint::*;
///
/// type EventCounter = Watcher<EventCounterData>;
///
/// #[derive(Default)]
/// struct EventCounterData {
///     counter: u32,
///     add: WatchedEvent<u32>,
/// }
///
/// impl WatcherInit for EventCounterData {
///     fn init(watcher: &mut WatcherMeta<Self>) {
///         bind_event!(watcher => root, root.add => amount, {
///             root.counter += amount;
///         });
///     }
/// }
///
/// fn main() {
///     let mut ctx = WatchContext::new();
///     ctx.with(|| {
///         let mut item = EventCounter::new();
///         item.data_mut().add.dispatch(7);
///         WatchContext::update_current();
///         assert_eq!(item.data().counter, 7);
///         item.data_mut().add.dispatch(9);
///         item.data_mut().add.dispatch(3);
///         WatchContext::update_current();
///         assert_eq!(item.data().counter, 19);
///     });
/// }
/// ```
pub struct WatchedEvent<T> {
    held_data: Option<T>,
    watcher: Watcher<AlternatingData<T>>,
}

impl<T: 'static> WatchedEvent<T> {
    /// Create a new WatchedEvent
    pub fn new() -> Self {
        Default::default()
    }

    /// This method provides the raw functionality of listening to an event.
    /// Normally, it is preferred to use the bind_event macro.
    /// This returns a reference to the value passed to dispatch() when the
    /// function is executing as a consequence of an event dispatch. When
    /// initially binding, and in-between dispatches, it will return `None`.
    pub fn get_current(&mut self) -> Option<&T> {
        let mut borrow = self.watcher.data_mut();
        borrow.current_trigger.watched();
        let hold = match borrow.current_data {
            Container::Fresh(_) => {
                std::mem::replace(&mut borrow.current_data, Container::Held)
            },
            Container::None => Container::None,
            Container::Held => Container::Held,
        };
        match hold {
            Container::Fresh(item) => {
                self.held_data = Some(item);
            },
            Container::None => {
                self.held_data = None;
            },
            Container::Held => (),
        };
        self.held_data.as_ref()
    }

    /// Trigger the event. The argument passed will be delivered to listeners.
    pub fn dispatch(&mut self, arg: T) {
        let mut data = self.watcher.data_mut();
        data.queue.push_back(arg);
        data.off_frame.trigger();
    }
}

impl<T: 'static> Default for WatchedEvent<T> {
    fn default() -> Self {
        WatchedEvent {
            held_data: None,
            watcher: Watcher::new(),
        }
    }
}
