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

    pub fn set(&mut self, value: T) {
        self.value = value;
        self.meta.trigger();
    }

    pub fn get_ref(&self) -> &T {
        self.meta.watched();
        &self.value
    }

    pub fn replace(&mut self, mut value: T) -> T {
        self.meta.watched();
        std::mem::swap(&mut self.value, &mut value);
        self.meta.trigger();
        value
    }

    pub fn into_inner(self) -> T {
        self.meta.watched();
        self.value
    }
}

impl<T: Copy> Watched<T> {
    pub fn get(&self) -> T {
        self.meta.watched();
        self.value
    }
}

impl<T: Default> Watched<T> {
    pub fn take(&mut self) -> T {
        self.meta.watched();
        let mut value = Default::default();
        std::mem::swap(&mut self.value, &mut value);
        self.meta.trigger();
        value
    }
}

impl<T: Default> Default for Watched<T> {
    fn default() -> Self {
        Watched::new(Default::default())
    }
}

pub struct RefWatched<T> {
    value: T,
    meta: WatchedMeta,
}

impl<T> RefWatched<T> {
    pub fn new(value: T) -> Self {
        RefWatched { value, meta: WatchedMeta::new() }
    }
}

impl<T> Deref for RefWatched<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.meta.watched();
        &self.value
    }
}


impl<T> DerefMut for RefWatched<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.meta.watched();
        self.meta.trigger();
        &mut self.value
    }
}

impl<T: Default> Default for RefWatched<T> {
    fn default() -> Self {
        RefWatched::new(Default::default())
    }
}

pub struct WatchedEvent<T> {
    inner: Watched<Option<T>>,
}

impl<T> WatchedEvent<T> {
    pub fn get_current(&self) -> Option<&T> {
        self.inner.get_ref().as_ref()
    }

    pub fn dispatch(&mut self, arg: T) {
        self.inner.set(Some(arg));
    }
}

impl<T> Default for WatchedEvent<T> {
    fn default() -> Self {
        WatchedEvent {
            inner: Watched::new(None),
        }
    }
}
