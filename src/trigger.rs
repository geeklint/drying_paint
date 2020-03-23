/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::{Rc, Weak};
use std::cell::{Cell, RefCell};

use super::WatchContext;

struct WatchData {
    // can this be a Box instead? why did I make it an Rc?
    update_fn: Rc<dyn Fn()>,
    cycle: Cell<usize>,
}

pub struct Watch(Rc<WatchData>);

impl Watch {
    pub fn new<T, F>(arg: Weak<RefCell<T>>, func: F) -> Self
        where T: ?Sized + 'static,
              F: Fn(&mut T) + 'static
    {
        let wrapper = move || {
            if let Some(strong_arg) = arg.upgrade() {
                func(&mut strong_arg.borrow_mut());
            }
        };
        let this = Watch(Rc::new(WatchData {
            update_fn: Rc::new(wrapper),
            cycle: Cell::new(0),
        }));
        this.get_ref().trigger();
        this
    }

    pub fn get_ref(&self) -> WatchRef {
        WatchRef {
            watch: Rc::downgrade(&self.0),
            cycle: self.0.cycle.get(),
        }
    }
}

#[derive(Clone)]
pub struct WatchRef {
    watch: Weak<WatchData>,
    cycle: usize,
}

impl WatchRef {
    fn trigger(self) {
        if let Some(watch) = self.watch.upgrade() {
            if self.cycle == watch.cycle.get() {
                let mut new = self.clone();
                new.cycle += 1;
                watch.cycle.set(new.cycle);
                WatchContext::expect_current(|ctx| {
                    ctx.bind_watch(new, || (watch.update_fn)());
                }, "WatchRef.trigger() called outside of WatchContext");
            }
        }
    }
}

pub struct WatchSet {
    empty: bool,
    vec: Vec<Option<WatchRef>>,
}

impl WatchSet {
    pub fn new() -> Self {
        WatchSet { empty: true, vec: Vec::new() }
    }

    pub fn empty(&self) -> bool { self.empty }

    pub fn add(&mut self, watch: WatchRef) {
        self.empty = false;
        for bucket in self.vec.iter_mut() {
            if bucket.is_none() {
                *bucket = Some(watch);
                return;
            }
        }
        self.vec.push(Some(watch));
    }

    pub fn add_all(&mut self, other: &mut WatchSet) {
        let mut src = other.vec.iter_mut();
        for dest_bucket in self.vec.iter_mut() {
            if dest_bucket.is_none() {
                loop {
                    if let Some(bucket) = src.next() {
                        if let Some(watch) = bucket.take() {
                            self.empty = false;
                            *dest_bucket = Some(watch);
                            break;
                        }
                    } else {
                        other.empty = true;
                        return;
                    }
                }
            }
        }
        for bucket in src {
            if let Some(watch) = bucket.take() {
                self.empty = false;
                self.vec.push(Some(watch));
            }
        }
        other.empty = true;
    }

    pub fn trigger(&mut self) {
        for bucket in self.vec.iter_mut() {
            if let Some(watch) = bucket.take() {
                watch.trigger();
            }
        }
        self.empty = true;
    }
}
