/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::{Rc, Weak};
use std::cell::{Cell, RefCell};

use crate::pointer::{
    BorrowedPointer,
};
use super::WatchContext;

struct WatchData {
    update_fn: RefCell<Box<dyn Fn()>>,
    cycle: Cell<usize>,
    debug_name: &'static str,
}

pub(crate) struct Watch(Rc<WatchData>);

impl Watch {
    pub fn new<T, F>(
        key_arg: &mut T,
        arg: BorrowedPointer<T>,
        func: F,
        debug_name: &'static str,
    ) -> Self
    where
        T: ?Sized + 'static,
        F: Fn(&mut T) + 'static
    {
        let this = Watch(Rc::new(WatchData {
            update_fn: RefCell::new(Box::new(|| ())),
            cycle: Cell::new(0),
            debug_name,
        }));
        WatchContext::expect_current(|ctx| {
            ctx.bind_watch(this.get_ref(), || {
                func(key_arg);
            });
        }, "Watch::new() called outside of WatchContext");
        let func_cell = Cell::new(Some(func));
        *this.0.update_fn.borrow_mut() = Box::new(move || {
            let func = func_cell.take().unwrap();
            let func = arg.upgrade(func, move |func, strong_arg| {
                func(strong_arg);
            });
            func_cell.set(Some(func));
        });
        this
    }

    pub fn get_ref(&self) -> WatchRef {
        WatchRef {
            watch: Rc::downgrade(&self.0),
            cycle: self.0.cycle.get(),
            debug_name: self.0.debug_name,
        }
    }
}

#[derive(Clone)]
pub struct WatchRef {
    watch: Weak<WatchData>,
    cycle: usize,
    debug_name: &'static str,
}

impl WatchRef {
    pub fn watch_eq(&self, other: &Self) -> bool {
        self.watch.ptr_eq(&other.watch)
    }

    fn trigger(self) {
        if let Some(watch) = self.watch.upgrade() {
            if self.cycle == watch.cycle.get() {
                let mut new = self;
                new.cycle += 1;
                watch.cycle.set(new.cycle);
                WatchContext::expect_current(|ctx| {
                    ctx.bind_watch(new, || {
                        (watch.update_fn.borrow())()
                    });
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

    pub fn add_all<F>(&mut self, other: &mut WatchSet, mut filter: F)
    where
        F: FnMut(&WatchRef) -> bool,
    {
        let mut src = other.vec.iter_mut();
        for dest_bucket in self.vec.iter_mut() {
            if dest_bucket.is_none() {
                loop {
                    if let Some(bucket) = src.next() {
                        if let Some(watch) = bucket.take() {
                            if filter(&watch) {
                                self.empty = false;
                                *dest_bucket = Some(watch);
                                break;
                            }
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
                if filter(&watch) {
                    self.empty = false;
                    self.vec.push(Some(watch));
                }
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

    pub fn debug_names(&self) -> String {
        self.vec.iter()
            .filter_map(|bucket| {
                bucket.as_ref().map(|watch| watch.debug_name)
            })
            .collect::<Vec<_>>()
            .join("\n  ")
    }
}

impl Default for WatchSet {
    fn default() -> Self {
        Self::new()
    }
}
