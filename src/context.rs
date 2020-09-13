/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use super::{WatchSet, WatchRef};

thread_local! {
    static CTX_STACK: RefCell<Vec<WatchContext>> = RefCell::new(Vec::new());
}

/// Most of the functions in this crate require that they are executing in
/// a context.  The context keeps track of some "global" state which enables
/// the functionality in this crate.
///
/// The following will panic if done outside of a WatchContext:
///   * Calling WatchContext::update_current() (you can use
/// WatchContext::update() to concisely update a context from outside itself).
///   * Mutating a [Watched](struct.Watched.html) value.
///   * Calling
/// [WatchedEvent::dispatch()](struct.WatchedEvent.html#method.dispatch)
///   * Calling
/// [WatchedMeta::trigger()](struct.WatchedMeta.html#method.trigger) (the two
/// above are actually just specific variations on this)
///   * Creating a [Watcher](struct.Watcher.html)
///
/// When a watched value changes, the code watching those values will be
/// queued onto the WatchContext. WatchContext::update_current() will execute
/// all pending operations.
/// Note: Because Watcher makes use of a RefCell internally to execute the
/// watching code, you should not keep references gotten from Watcher::data()
/// or Watcher::data_mut() around during WatchContext::update_current()
/// or WatchContext::update().
pub struct WatchContext {
    front_frame: WatchSet,
    back_frame: WatchSet,
    watching_stack: RefCell<Vec<WatchRef>>,
    frame_limit: Option<usize>,
}

impl WatchContext {
    /// Create a new WatchContext
    pub fn new() -> Self {
        let frame_limit = if cfg!(debug_assertions) {
            Some(16_384)
        } else {
            None
        };
        WatchContext {
            front_frame: WatchSet::new(),
            back_frame: WatchSet::new(),
            watching_stack: RefCell::new(Vec::new()),
            frame_limit,
        }
    }

    /// Set this WatchContext as the current one for the duration of the
    /// passed function. Note that it is supported (although discouraged) to
    /// nest WatchContexts within each other.
    pub fn with<R, F: FnOnce() -> R>(self, func: F) -> (Self, R) {
        CTX_STACK.with(|stack| {
            stack.borrow_mut().push(self);
            let res = (func)();
            (stack.borrow_mut().pop().unwrap(), res)
        })
    }

    /// Execute all operations which are currently pending because a value
    /// they were watching changed. 
    /// Note: Because Watcher makes use of a RefCell internally to execute
    /// the watching code, you should not keep references gotten from
    /// Watcher::data() or Watcher::data_mut() around during
    /// WatchContext::update_current() or WatchContext::update().
    ///
    /// # Panics
    /// This function will panic if called outside of WatchContext::with, or
    /// if any function queued for update panics or if the limit set by
    /// set_frame_limit is exceeded.
    pub fn update_current() {
        Self::expect_current(|ctx| {
            ctx.internal_update();
        }, "WatchContext::update_current() called outside of WatchContext");
    }

    /// The same as doing `context.with(|| WatchContext::update_current())`
    pub fn update(self) -> Self {
        self.with(Self::update_current).0
    }

    /// Set the number of cycles this watch context will execute before
    /// panicking. This is useful for catching bugs involving recursive
    /// watches. None indicates no limit. The default behaviour is to provide
    /// a high value for debug builds and no limit for release builds.
    ///
    /// # Examples
    /// ```rust,should_panic
    /// # use drying_paint::*;
    /// #[derive(Default)]
    /// struct KeepBalanced {
    ///     left: Watched<i32>,
    ///     right: Watched<i32>,
    /// }
    /// impl WatcherInit for KeepBalanced {
    ///     fn init(watcher: &mut WatcherMeta<Self>) {
    ///         watcher.watch(|root| {
    ///             *root.left = *root.right;
    ///         });
    ///         watcher.watch(|root| {
    ///             *root.right = *root.left;
    ///         });
    ///     }
    /// }
    /// fn main() {
    ///     let mut ctx = WatchContext::new();
    ///     ctx.set_frame_limit(Some(100));
    ///     ctx = ctx.with(|| {
    ///         let obj = WatchContext::allow_watcher_access((), |()| {
    ///             let mut obj = Watcher::<KeepBalanced>::new();
    ///             *obj.data_mut().left = 4;
    ///             obj
    ///         });
    ///         // because we used set_frame_limit, this will panic after
    ///         // 100 iterations.
    ///         WatchContext::update_current();
    ///     }).0;
    /// }
    pub fn set_frame_limit(&mut self, value: Option<usize>) {
        self.frame_limit = value;
    }

    /// In order to ensure the data stored in Watchers is not mutably aliased
    /// during watch callbacks, Watcher::data() and Watcher::data_mut() will
    /// panic if called outside this function or a watch callback.
    pub fn allow_watcher_access<F, U, R>(data: U, func: F) -> R
    where
        F: 'static + FnOnce(U) -> R,
        U: 'static,
        R: 'static,
    {
        crate::pointer::BorrowedPointer::allow_refs(data, func)
    }

    pub(crate) fn expect_current<F: FnOnce(&WatchContext)>(func: F, msg: &str) {
        CTX_STACK.with(|stack| {
            let borrow = stack.borrow();
            (func)(borrow.last().expect(msg));
        });
    }

    pub(crate) fn try_get_current<F: FnOnce(&WatchContext)>(func: F) {
        CTX_STACK.with(|stack| {
            let borrow = stack.borrow();
            if let Some(ptr) = borrow.last() {
                (func)(ptr);
            }
        });
    }

    fn internal_update(&self) {
        if let Some(mut frame_limit) = self.frame_limit {
            while !self.back_frame.empty() {
                if frame_limit == 0 {
                    let current_watch_names = {
                        self.back_frame.debug_names()
                    };
                    panic!(
                        "Updating a WatchContext exceeded it's \
                        limit for iteration.  This usually means there is a \
                        recursive watch.  You may be interested in \
                        Watched::set_if_neq to resolve recursive watches.  \
                        If the number of iterations was intentional, you \
                        can try increasing the limit with \
                        WatchContext::set_frame_limit.  The following types \
                        might be involved in the recursive watch:\n  {}",
                        current_watch_names,
                    );
                }
                self.front_frame.swap(&self.back_frame);
                self.front_frame.trigger();
                frame_limit -= 1;
            }
        } else {
            while !self.back_frame.empty() {
                self.front_frame.swap(&self.back_frame);
                self.front_frame.trigger();
            }
        }
    }

    pub(crate) fn bind_watch<F: FnOnce()>(&self, watch: WatchRef, func: F) {
        self.watching_stack.borrow_mut().push(watch);
        (func)();
        self.watching_stack.borrow_mut().pop();
    }

    pub(crate) fn current_watch(&self) -> Option<WatchRef> {
        Some(self.watching_stack.borrow().last()?.clone())
    }
    
    pub(crate) fn add_to_next(&self, set: &WatchSet) {
        match self.watching_stack.borrow().last() {
            Some(watch) => {
                self.back_frame.add_all(set, |to_add| !to_add.watch_eq(watch));
            },
            None => {
                self.back_frame.add_all(set, |_| true);
            },
        };
    }
}

impl Default for WatchContext {
    fn default() -> Self {
        Self::new()
    }
}
