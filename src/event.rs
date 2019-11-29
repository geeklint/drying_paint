use std::cell::Cell;
use std::collections::VecDeque;

use super::{
    WatchedMeta, Watched,
    WatcherInit, Watcher, WatcherMeta,
};

enum Container<T> {
    Fresh(T),
    Held,
    None,
}

fn with_container<T, F>(cell: &Cell<Container<T>>, func: F)
    where F: FnOnce(&mut Container<T>)
{
    let mut tmp = cell.replace(Container::None);
    (func)(&mut tmp);
    cell.set(tmp);
}

struct AlternatingData<T> {
    queue: VecDeque<T>,
    current: Watched<Cell<Container<T>>>,
    off_frame: WatchedMeta,
}

impl<T: 'static> WatcherInit for AlternatingData<T> {
    fn init(watcher: &mut WatcherMeta<Self>) {
        watcher.watch(|data| {
            data.off_frame.watched();
            let next = data.queue.pop_front();
            *data.current.get_mut() = if let Some(item) = next {
                Container::Fresh(item)
            } else {
                Container::None
            }
        });

        watcher.watch(|data| {
            let mut trigger = false;
            with_container(&data.current, |container| {
                trigger = match container {
                    Container::None => false,
                    _ => true,
                };
            });
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
            current: Watched::new(Cell::new(Container::None)),
            off_frame: WatchedMeta::new(),
        }
    }
}

pub struct WatchedEvent<T> {
    held_data: Option<T>,
    watcher: Watcher<AlternatingData<T>>,
}

impl<T: 'static> WatchedEvent<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_current(&mut self) -> Option<&T> {
        let mut hold = Container::Held;
        with_container(&self.watcher.data().current, |container| {
            if let Container::Fresh(_) = container {
                std::mem::swap(container, &mut hold);
            } else if let Container::None = container {
                hold = Container::None;
            }
        });
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
