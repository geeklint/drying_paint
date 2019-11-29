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
/// WatchContext::update() to update a context from outside itself).
///   * Mutating a Watched<T> value.
///   * Calling WatchedEvent::dispatch()
///   * Calling WatchedMeta::trigger() (the two above are actually just
/// specific variations on this)
///   * Creating a Watcher<T>
pub struct WatchContext {
    front_frame: RefCell<WatchSet>,
    back_frame: RefCell<WatchSet>,
    watching_stack: RefCell<Vec<WatchRef>>,
}

impl WatchContext {
    /// Create a new WatchContext
    pub fn new() -> Self {
        WatchContext {
            front_frame: RefCell::new(WatchSet::new()),
            back_frame: RefCell::new(WatchSet::new()),
            watching_stack: RefCell::new(Vec::new()),
        }
    }

    pub fn with<F: FnOnce()>(&mut self, func: F) {
        CTX_STACK.with(|stack| {
            stack.borrow_mut().push(self as *const Self);
            (func)();
            stack.borrow_mut().pop();
        });
    }

    pub fn update_current() {
        Self::expect_current(|ctx| {
            ctx.internal_update();
        }, "WatchContext::update_current() called outside of WatchContext");
    }

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
        while !self.back_frame.borrow().empty() {
            self.front_frame.swap(&self.back_frame);
            self.front_frame.borrow_mut().trigger();
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
