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

use std::fmt;

impl<T: fmt::Debug> fmt::Debug for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display> fmt::Display for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: PartialEq> PartialEq for Watched<T> {
    #[inline]
    fn eq(&self, other: &Watched<T>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
    #[inline]
    fn ne(&self, other: &Watched<T>) -> bool {
        PartialEq::ne(&**self, &**other)
    }
}

use core::cmp::Ordering;

impl<T: PartialOrd> PartialOrd for Watched<T> {
    #[inline]
    fn partial_cmp(&self, other: &Watched<T>) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
    #[inline]
    fn lt(&self, other: &Watched<T>) -> bool {
        PartialOrd::lt(&**self, &**other)
    }
    #[inline]
    fn le(&self, other: &Watched<T>) -> bool {
        PartialOrd::le(&**self, &**other)
    }
    #[inline]
    fn ge(&self, other: &Watched<T>) -> bool {
        PartialOrd::ge(&**self, &**other)
    }
    #[inline]
    fn gt(&self, other: &Watched<T>) -> bool {
        PartialOrd::gt(&**self, &**other)
    }
}

impl<T: Ord> Ord for Watched<T> {
    #[inline]
    fn cmp(&self, other: &Watched<T>) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T: Eq> Eq for Watched<T> {}
