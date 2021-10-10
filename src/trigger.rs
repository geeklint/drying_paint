/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use core::{cell::Cell, mem};
use std::rc::{Rc, Weak};

struct WatchData<F: ?Sized> {
    cycle: Cell<usize>,
    update_fn: F,
    //debug_name: &'static str,
}

#[derive(Clone, Copy)]
pub struct WatchArg<'a, 'ctx> {
    pub(crate) watch: &'a Watch<'ctx>,
    pub(crate) post_set: &'a Weak<WatchSet<'ctx>>,
}

struct OwnedWatchArg(Watch<'static>, Weak<WatchSet<'static>>);

thread_local! {
    static CURRENT_ARG: Cell<Option<OwnedWatchArg>> = Cell::new(None);
}

impl<'a> WatchArg<'a, 'static> {
    pub fn use_as_current<R, F: FnOnce() -> R>(&self, f: F) -> R {
        CURRENT_ARG.with(|cell| {
            let to_set =
                OwnedWatchArg(self.watch.clone(), self.post_set.clone());
            let prev = cell.replace(Some(to_set));
            let ret = f();
            cell.set(prev);
            ret
        })
    }

    pub fn try_with_current<F: FnOnce(WatchArg<'_, 'static>)>(
        f: F,
    ) -> Option<()> {
        CURRENT_ARG.with(|cell| {
            // TODO: re-entrence?
            let owned = cell.take()?;
            let ret = {
                let OwnedWatchArg(ref watch, ref post_set) = owned;
                f(WatchArg { watch, post_set })
            };
            cell.set(Some(owned));
            Some(ret)
        })
    }
}

#[derive(Clone)]
pub(crate) struct Watch<'ctx>(
    Rc<WatchData<dyn 'ctx + Fn(WatchArg<'_, 'ctx>)>>,
);

impl<'ctx> Watch<'ctx> {
    pub fn new<F>(func: F, post_set: &Weak<WatchSet<'ctx>>)
    where
        F: 'ctx + Fn(WatchArg<'_, 'ctx>),
    {
        let this = Watch(Rc::new(WatchData {
            update_fn: func,
            cycle: Cell::new(0),
        }));
        this.get_ref().execute(post_set);
    }

    pub fn get_ref(&self) -> WatchRef<'ctx> {
        WatchRef {
            watch: self.clone(),
            cycle: self.0.cycle.get(),
            //debug_name: self.0.debug_name,
        }
    }
}

#[derive(Clone)]
pub(crate) struct WatchRef<'ctx> {
    watch: Watch<'ctx>,
    cycle: usize,
    //debug_name: &'static str,
}

impl<'ctx> WatchRef<'ctx> {
    pub fn watch_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.watch.0, &other.watch.0)
    }

    pub fn watch_eq_watch(&self, other: &Watch<'ctx>) -> bool {
        Rc::ptr_eq(&self.watch.0, &other.0)
    }

    fn execute(self, post_set: &Weak<WatchSet<'ctx>>) {
        if self.cycle == self.watch.0.cycle.get() {
            self.watch.0.cycle.set(self.cycle + 1);
            (self.watch.0.update_fn)(WatchArg {
                watch: &self.watch,
                post_set,
            });
        }
    }
}

#[derive(Default)]
struct WatchSetNode<'ctx> {
    data: [Option<WatchRef<'ctx>>; 4], // TODO: analyse better len here?
    next: Option<Box<WatchSetNode<'ctx>>>,
}

#[derive(Default)]
struct WatchSetHead<'ctx> {
    node: WatchSetNode<'ctx>,
    target: Weak<WatchSet<'ctx>>,
}

#[derive(Default)]
pub struct WatchSet<'ctx> {
    list: Cell<Option<Box<WatchSetHead<'ctx>>>>,
}

impl<'ctx> WatchSet<'ctx> {
    pub fn new() -> Self {
        WatchSet {
            list: Cell::new(None),
        }
    }

    fn with<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut Option<Box<WatchSetHead<'ctx>>>) -> R,
    {
        let mut list = self.list.replace(None);
        let ret = func(&mut list);
        self.list.set(list);
        ret
    }

    pub fn empty(&self) -> bool {
        self.with(|list| list.is_none())
    }

    pub(crate) fn add(&self, watch: WatchRef<'ctx>, target: &Weak<Self>) {
        self.with(|list| {
            let head = list.get_or_insert_with(|| {
                Box::new(WatchSetHead {
                    node: WatchSetNode::default(),
                    target: target.clone(),
                })
            });
            for bucket in head.node.data.iter_mut() {
                if bucket.is_none() {
                    *bucket = Some(watch);
                    return;
                }
            }
            head.node.next = Some(Box::new(mem::take(&mut head.node)));
            head.node.data[0] = Some(watch);
        });
    }

    fn add_all<F>(&self, other: &WatchSet<'ctx>, mut filter: F)
    where
        F: FnMut(&WatchRef<'ctx>) -> bool,
    {
        if let Some(other_head) = other.list.take() {
            let target = &other_head.target;
            let mut node = other_head.node;
            loop {
                for bucket in node.data.iter_mut() {
                    if let Some(watch) = bucket.take() {
                        if filter(&watch) {
                            self.add(watch, target);
                        }
                    }
                }
                node = if let Some(next) = node.next {
                    *next
                } else {
                    break;
                };
            }
        }
    }

    pub(crate) fn trigger_with_current(&self, current: &Watch<'ctx>) {
        if let Some(target) = self
            .with(|list| list.as_mut().and_then(|head| head.target.upgrade()))
        {
            target.add_all(self, |to_add| !to_add.watch_eq_watch(current));
        }
    }

    pub fn trigger_external(&self) {
        if let Some(target) = self
            .with(|list| list.as_mut().and_then(|head| head.target.upgrade()))
        {
            target.add_all(self, |_| true);
        }
    }

    pub fn execute(&self, next_frame: &Weak<Self>) {
        let mut node = if let Some(head) = self.list.take() {
            head.node
        } else {
            return;
        };
        loop {
            for bucket in node.data.iter_mut() {
                if let Some(watch) = bucket.take() {
                    watch.execute(next_frame);
                }
            }
            node = if let Some(next) = node.next {
                *next
            } else {
                break;
            }
        }
    }

    /*
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
    */

    pub fn swap(&self, other: &WatchSet<'ctx>) {
        Cell::swap(&self.list, &other.list);
    }
}
