use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use super::{WatchContext, WatchSet};

pub struct WatchedMeta {
    watchers: RefCell<WatchSet>,
}

impl WatchedMeta {
    pub fn new() -> Self {
        WatchedMeta { watchers: RefCell::new(WatchSet::new()) }
    }

    pub fn watched(&self) {
        WatchContext::expect_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.watchers.borrow_mut().add(watch);
            }
        }, "WatchedMeta.watched() called outside of WatchContext");
    }

    pub fn trigger(&mut self) {
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut self.watchers.borrow_mut());
        }, "WatchedMeta.trigger() called outside of WatchContext");
    }
}

pub struct Watched<T> {
    value: T,
    meta: WatchedMeta,
}

impl<T> Watched<T> {
    pub fn new(value: T) -> Self {
        Watched { value, meta: WatchedMeta::new() }
    }
}

impl<T> Deref for Watched<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.meta.watched();
        &self.value
    }
}


impl<T> DerefMut for Watched<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.meta.trigger();
        self.meta.watched();
        &mut self.value
    }
}

impl<T: Default> Default for Watched<T> {
    fn default() -> Self {
        Watched::new(Default::default())
    }
}
