/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use super::{WatchSet, WatchRef};

thread_local! {
    static CTX_STACK: RefCell<Vec<*const WatchContext>> = RefCell::new(Vec::new());
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
    front_frame: RefCell<WatchSet>,
    back_frame: RefCell<WatchSet>,
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
            front_frame: RefCell::new(WatchSet::new()),
            back_frame: RefCell::new(WatchSet::new()),
            watching_stack: RefCell::new(Vec::new()),
            frame_limit,
        }
    }

    /// Set this WatchContext as the current one for the duration of the
    /// passed function. Note that it is supported (although discouraged) to
    /// nest WatchContexts within each other.
    pub fn with<R, F: FnOnce() -> R>(&mut self, func: F) -> R {
        CTX_STACK.with(|stack| {
            stack.borrow_mut().push(self as *const Self);
            let res = (func)();
            stack.borrow_mut().pop();
            res
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
    pub fn update(&mut self) {
        self.with(|| Self::update_current());
    }

    pub(crate) fn expect_current<F: FnOnce(&WatchContext)>(func: F, msg: &str) {
        CTX_STACK.with(|stack| {
            let borrow = stack.borrow();
            let ptr = borrow.last().expect(msg);
            (func)(unsafe { ptr.as_ref().unwrap() });
        });
    }

    pub(crate) fn try_get_current<F: FnOnce(&WatchContext)>(func: F) {
        CTX_STACK.with(|stack| {
            let borrow = stack.borrow();
            if let Some(ptr) = borrow.last() {
                (func)(unsafe { ptr.as_ref().unwrap() });
            }
        });
    }

    fn internal_update(&self) {
        if let Some(mut frame_limit) = self.frame_limit {
            while !self.back_frame.borrow().empty() {
                if frame_limit == 0 {
                    panic!("Updating a WatchContext exceeded it's \
                    limit for iteration. This usually means there is a \
                    recursive watch. You may be interested in \
                    Watched::set_if_neq to resolve recursive watches. \
                    If the number of iterations was intentional, you \
                    can try increasing the limit with \
                    WatchContext::set_frame_limit.");
                }
                self.front_frame.swap(&self.back_frame);
                self.front_frame.borrow_mut().trigger();
                frame_limit -= 1;
            }
        } else {
            while !self.back_frame.borrow().empty() {
                self.front_frame.swap(&self.back_frame);
                self.front_frame.borrow_mut().trigger();
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
    
    pub(crate) fn add_to_next(&self, set: &mut WatchSet) {
        self.back_frame.borrow_mut().add_all(set);
    }
}
