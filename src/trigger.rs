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

#[derive(Default)]
struct WatchSetNode {
    data: [Option<WatchRef>; 4],  // TODO: analyse better len here?
    next: Option<Box<WatchSetNode>>,
}

pub struct WatchSet {
    list: Cell<Option<Box<WatchSetNode>>>,
}

impl WatchSet {
    pub fn new() -> Self {
        WatchSet { list: Cell::new(None) }
    }

    fn with<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut Option<Box<WatchSetNode>>) -> R,
    {
        let mut list = self.list.replace(None);
        let ret = func(&mut list);
        self.list.set(list);
        ret
    }

    pub fn empty(&self) -> bool {
        self.with(|list| list.is_none())
    }

    pub fn add(&self, watch: WatchRef) {
        self.with(|list| {
            loop {
                let node = list.get_or_insert_with(Box::default);
                for bucket in node.data.iter_mut() {
                    if bucket.is_none() {
                        *bucket = Some(watch);
                        return;
                    }
                }
                let mut new = Box::new(WatchSetNode::default());
                new.next = list.take();
                *list = Some(new);
            }
        });
    }

    pub fn add_all<F>(&self, other: &WatchSet, mut filter: F)
    where
        F: FnMut(&WatchRef) -> bool,
    {
        let mut other_list = other.list.take();
        while let Some(mut node) = other_list {
            for bucket in node.data.iter_mut() {
                if let Some(watch) = bucket.take() {
                    if filter(&watch) {
                        self.add(watch);
                    }
                }
            }
            other_list = node.next;
        }
    }

    pub fn trigger(&self) {
        let mut list = self.list.take();
        while let Some(mut node) = list {
            for bucket in node.data.iter_mut() {
                if let Some(watch) = bucket.take() {
                    watch.trigger();
                }
            }
            list = node.next;
        }
    }

    pub fn debug_names(&self) -> String {
        self.with(|mut list| {
            let mut names = Vec::new();
            while let Some(node) = list {
                names.extend(node.data.iter().filter_map(|bucket| {
                    bucket.as_ref().map(|watch| watch.debug_name)
                }));
                list = &mut node.next;
            }
            names.join("\n  ")
        })
    }
}

impl Default for WatchSet {
    fn default() -> Self {
        Self::new()
    }
}
