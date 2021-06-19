/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::collections::VecDeque;

use super::{WatchedMeta, Watcher, WatcherInit, WatcherMeta};

struct AlternatingData<T> {
    queue: VecDeque<T>,
    current_data: Option<T>,
    current_trigger: WatchedMeta,
    off_frame: WatchedMeta,
}

impl<T: 'static> WatcherInit for AlternatingData<T> {
    fn init(watcher: &mut WatcherMeta<Self>) {
        watcher.watch(|data| {
            data.off_frame.watched();
            data.current_data = data.queue.pop_front();
            data.current_trigger.trigger();
        });

        watcher.watch(|data| {
            data.current_trigger.watched();
            if data.current_data.is_some() {
                data.off_frame.trigger();
            }
        });
    }
}

impl<T> Default for AlternatingData<T> {
    fn default() -> Self {
        AlternatingData {
            queue: VecDeque::new(),
            current_data: None,
            current_trigger: WatchedMeta::new(),
            off_frame: WatchedMeta::new(),
        }
    }
}

/// A WatchedEvent uses the watch system provided by this crate to implement
/// an event disptacher. This is different from a watched value
/// ([Watched](struct.Watched.html)) in that events will fire for each value
/// passed to WatchedEvent::dispatch() and will not "store" the data.
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
///         watcher.watch(|root| {
///             if let Some(amount) = root.add.bind() {
///                 root.counter += amount;
///             }
///         });
///     }
/// }
///
/// fn main() {
///     let mut ctx = WatchContext::new();
///     ctx.with(|| {
///         let item = WatchContext::allow_watcher_access((), |()| {
///             let mut item = EventCounter::new();
///             item.data_mut().add.dispatch(7);
///             item
///         });
///         WatchContext::update_current();
///         let item = WatchContext::allow_watcher_access(item, |mut item| {
///             assert_eq!(item.data().counter, 7);
///             item.data_mut().add.dispatch(9);
///             item.data_mut().add.dispatch(3);
///             item
///         });
///         WatchContext::update_current();
///         WatchContext::allow_watcher_access(item, |mut item| {
///             assert_eq!(item.data().counter, 19);
///         });
///     });
/// }
/// ```
pub struct WatchedEvent<T> {
    watcher: Watcher<AlternatingData<T>>,
}

impl<T: 'static> WatchedEvent<T> {
    /// Create a new WatchedEvent
    pub fn new() -> Self {
        Default::default()
    }

    /// Used inside a [watch](struct.WatcherMeta.html#method.watch) closure
    /// this will return a value each time the event is dispatched
    pub fn bind(&self) -> Option<&T> {
        let borrow = self.watcher.data();
        borrow.current_trigger.watched();
        borrow.current_data.as_ref()
    }

    /// Trigger the event. The argument passed will be delivered to listeners.
    pub fn dispatch(&mut self, arg: T) {
        let data = self.watcher.data_mut();
        data.queue.push_back(arg);
        data.off_frame.trigger();
    }
}

impl<T: 'static> Default for WatchedEvent<T> {
    fn default() -> Self {
        WatchedEvent {
            watcher: Watcher::new(),
        }
    }
}
