use std::cell::RefCell;

use super::{WatchSet, WatchRef};

thread_local! {
    static CTX_STACK: RefCell<Vec<*const WatchContext>> = RefCell::new(Vec::new());
}

pub struct WatchContext {
    front_frame: RefCell<WatchSet>,
    back_frame: RefCell<WatchSet>,
    watching_stack: RefCell<Vec<WatchRef>>,
}

impl WatchContext {
    pub fn new() -> Self {
        WatchContext {
            front_frame: RefCell::new(WatchSet::new()),
            back_frame: RefCell::new(WatchSet::new()),
            watching_stack: RefCell::new(Vec::new()),
        }
    }

    pub fn expect_current<F: FnOnce(&WatchContext)>(func: F, msg: &str) {
        CTX_STACK.with(|stack| {
            let borrow = stack.borrow();
            let ptr = borrow.last().expect(msg);
            (func)(unsafe { ptr.as_ref().unwrap() });
        });
    }

    fn internal_update(&self) {
        while !self.back_frame.borrow().empty() {
            self.front_frame.swap(&self.back_frame);
            self.front_frame.borrow_mut().trigger();
        }
    }

    pub fn update_current() {
        Self::expect_current(|ctx| {
            ctx.internal_update();
        }, "WatchContext::update_current() called outside of WatchContext");
    }

    pub fn with<F: FnOnce()>(&mut self, func: F) {
        CTX_STACK.with(|stack| {
            stack.borrow_mut().push(self as *const Self);
            (func)();
            stack.borrow_mut().pop();
        });
    }

    pub fn bind_watch<F: FnOnce()>(&self, watch: WatchRef, func: F) {
        self.watching_stack.borrow_mut().push(watch);
        (func)();
        self.watching_stack.borrow_mut().pop();
    }

    pub fn current_watch(&self) -> Option<WatchRef> {
        Some(self.watching_stack.borrow().last()?.clone())
    }
    
    pub fn add_to_next(&self, set: &mut WatchSet) {
        self.back_frame.borrow_mut().add_all(set);
    }

    pub fn update(&mut self) {
        self.with(|| Self::update_current());
    }
}
