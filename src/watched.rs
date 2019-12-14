/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use super::{WatchContext, WatchSet};

/// This provides the basic functionality behind watched values. You can use
/// it to provide functionality using the watch system for cases where
/// [Watched](struct.Watched.html) and
/// [WatchedEvent](struct.WatchedEvent.html) are not appropriate.
pub struct WatchedMeta {
    watchers: RefCell<WatchSet>,
}

impl WatchedMeta {
    /// Create a new WatchedMeta instance
    pub fn new() -> Self {
        WatchedMeta { watchers: RefCell::new(WatchSet::new()) }
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when this is triggered.
    pub fn watched(&self) {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.watchers.borrow_mut().add(watch);
            }
        });
    }

    /// Mark this value as having changed, so that watching functions will
    /// be marked as needing to be updated.
    /// # Panics
    /// This function will panic if called outside of WatchContext::with
    pub fn trigger(&mut self) {
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut self.watchers.borrow_mut());
        }, "WatchedMeta.trigger() called outside of WatchContext");
    }
}

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
pub struct Watched<T> {
    value: T,
    meta: WatchedMeta,
}

impl<T> Watched<T> {
    /// Create a new watched value.
    pub fn new(value: T) -> Self {
        Watched { value, meta: WatchedMeta::new() }
    }
}

impl<T: PartialEq> Watched<T> {
    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    ///
    /// # Examples
    /// The following example uses the watch system to keep two variables in
    /// sync. This would normally cause an infinite loop as each update of
    /// one would cause the other one to re-evaluate. However using set_if_neq
    /// allows it to detect that the value is the same and stop propogating.
    /// ```rust
    /// # use drying_paint::*;
    /// #[derive(Default)]
    /// struct KeepBalanced {
    ///     left: Watched<i32>,
    ///     right: Watched<i32>,
    /// }
    /// impl WatcherInit for KeepBalanced {
    ///     fn init(watcher: &mut WatcherMeta<Self>) {
    ///         watcher.watch(|root| {
    ///             Watched::set_if_neq(&mut root.left, *root.right);
    ///         });
    ///         watcher.watch(|root| {
    ///             Watched::set_if_neq(&mut root.right, *root.left);
    ///         });
    ///     }
    /// }
    /// fn main() {
    ///     let mut ctx = WatchContext::new();
    ///     ctx.set_frame_limit(Some(100));
    ///     ctx.with(|| {
    ///         let mut obj = Watcher::<KeepBalanced>::new();
    ///         *obj.data_mut().left = 68;
    ///         WatchContext::update_current();
    ///         assert_eq!(*obj.data().right, 68);
    ///     });
    /// }
    pub fn set_if_neq(wrapper: &mut Watched<T>, value: T) {
        if wrapper.value != value {
            wrapper.value = value;
            wrapper.meta.trigger();
            wrapper.meta.watched();
        }
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

impl<T: Clone> Clone for Watched<T> {
    fn clone(&self) -> Watched<T> {
        Watched {
            value: Clone::clone(&self.value),
            meta: WatchedMeta::new(),
        }
    }
}

