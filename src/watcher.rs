use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::Watch;

pub struct WatcherMeta<T: ?Sized> {
    data: Weak<RefCell<T>>,
    watches: Vec<Watch>,
}


impl<T: ?Sized + 'static> WatcherMeta<T> {
    pub fn watch<F>(&mut self, func: F)
        where F: Fn(&mut T) + 'static
    {
        let data = self.data.clone();
        let watch = Watch::new(data, func);
        self.watches.push(watch);
    }
}

pub trait WatcherInit {
    fn init(watcher: &mut WatcherMeta<Self>);
}

pub struct Watcher<T: WatcherInit + ?Sized> {
    data: Rc<RefCell<T>>,
    meta: WatcherMeta<T>,
}

impl<T: WatcherInit> Watcher<T> {
    pub fn create(data: T) -> Self {
        let data = Rc::new(RefCell::new(data));
        let mdata = Rc::downgrade(&data);
        let mut this = Watcher {
            data: data,
            meta: WatcherMeta {
                data: mdata,
                watches: Vec::new(),
            },
        };
        this.init();
        this
    }
}

impl<T: WatcherInit + ?Sized> Watcher<T> {
    fn init(&mut self) {
        WatcherInit::init(&mut self.meta);
    }

    pub fn data(&mut self) -> &RefCell<T> { &self.data }
}

impl<T: WatcherInit + Default> Watcher<T> {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<T: WatcherInit + Default> Default for Watcher<T> {
    fn default() -> Self {
        Watcher::create(Default::default())
    }
}
