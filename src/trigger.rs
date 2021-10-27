/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    alloc::{
        boxed::Box,
        rc::{Rc, Weak},
    },
    core::{cell::Cell, mem},
};

use crate::{FrameInfo, WatchContext};

struct WatchData<F: ?Sized> {
    cycle: Cell<usize>,
    update_fn: F,
    //debug_name: &'static str,
}

pub struct WatchArg<'a, 'ctx, O: ?Sized> {
    pub(crate) watch: &'a Watch<'ctx, O>,
    pub(crate) frame_info: &'a FrameInfo<'ctx, O>,
}

impl<'a, 'ctx, O: ?Sized> Copy for WatchArg<'a, 'ctx, O> {}
impl<'a, 'ctx, O: ?Sized> Clone for WatchArg<'a, 'ctx, O> {
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(feature = "std")]
mod watcharg_current {
    use crate::DefaultOwner;

    use super::*;

    struct OwnedWatchArg(
        Watch<'static, DefaultOwner>,
        FrameInfo<'static, DefaultOwner>,
    );

    thread_local! {
        static CURRENT_ARG: Cell<Option<OwnedWatchArg>> = Cell::new(None);
    }

    impl<'a> WatchArg<'a, 'static, DefaultOwner> {
        pub fn use_as_current<R, F: FnOnce() -> R>(&self, f: F) -> R {
            CURRENT_ARG.with(|cell| {
                let to_set =
                    OwnedWatchArg(self.watch.clone(), self.frame_info.clone());
                let prev = cell.replace(Some(to_set));
                let ret = f();
                cell.set(prev);
                ret
            })
        }

        pub fn try_with_current<
            F: FnOnce(WatchArg<'_, 'static, DefaultOwner>),
        >(
            f: F,
        ) -> Option<()> {
            CURRENT_ARG.with(|cell| {
                // TODO: re-entrence?
                let owned = cell.take()?;
                let ret = {
                    let OwnedWatchArg(ref watch, ref frame_info) = owned;
                    f(WatchArg { watch, frame_info })
                };
                cell.set(Some(owned));
                Some(ret)
            })
        }
    }
}

type WatchFn<'ctx, O> =
    dyn 'ctx + Fn(&mut WatchContext<'ctx, O>, &Watch<'ctx, O>);

pub(crate) struct Watch<'ctx, O: ?Sized>(Rc<WatchData<WatchFn<'ctx, O>>>);

impl<'ctx, O: ?Sized> Clone for Watch<'ctx, O> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<'ctx, O: ?Sized> Watch<'ctx, O> {
    pub(crate) fn spawn<F>(ctx: &mut WatchContext<'ctx, O>, func: F)
    where
        F: 'ctx + Fn(&mut O, WatchArg<'_, 'ctx, O>),
    {
        let update_fn = {
            move |ctx: &mut WatchContext<'ctx, O>, watch: &Self| {
                let WatchContext {
                    owner, frame_info, ..
                } = ctx;
                func(owner, WatchArg { watch, frame_info });
            }
        };
        let this = Watch(Rc::new(WatchData {
            update_fn,
            cycle: Cell::new(0),
        }));
        this.get_ref().execute(ctx);
    }

    pub(crate) fn get_ref(&self) -> WatchRef<'ctx, O> {
        WatchRef {
            watch: self.clone(),
            cycle: self.0.cycle.get(),
            //debug_name: self.0.debug_name,
        }
    }
}

#[derive(Clone)]
pub(crate) struct WatchRef<'ctx, O: ?Sized> {
    watch: Watch<'ctx, O>,
    cycle: usize,
    //debug_name: &'static str,
}

impl<'ctx, O: ?Sized> WatchRef<'ctx, O> {
    pub fn watch_eq(&self, other: &Watch<'ctx, O>) -> bool {
        Rc::ptr_eq(&self.watch.0, &other.0)
    }

    fn execute(self, ctx: &mut WatchContext<'ctx, O>) {
        if self.cycle == self.watch.0.cycle.get() {
            self.watch.0.cycle.set(self.cycle + 1);
            (self.watch.0.update_fn)(ctx, &self.watch);
        }
    }
}

struct WatchSetNode<'ctx, O: ?Sized> {
    data: [Option<WatchRef<'ctx, O>>; 4], // TODO: analyse better len here?
    next: Option<Box<WatchSetNode<'ctx, O>>>,
}

impl<'ctx, O: ?Sized> Default for WatchSetNode<'ctx, O> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            next: None,
        }
    }
}

struct WatchSetHead<'ctx, O: ?Sized> {
    node: WatchSetNode<'ctx, O>,
    target: Weak<WatchSet<'ctx, O>>,
}

impl<'ctx, O: ?Sized> Default for WatchSetHead<'ctx, O> {
    fn default() -> Self {
        Self {
            node: WatchSetNode::default(),
            target: Weak::default(),
        }
    }
}

pub(crate) struct WatchSet<'ctx, O: ?Sized> {
    list: Cell<Option<Box<WatchSetHead<'ctx, O>>>>,
}

impl<'ctx, O: ?Sized> Default for WatchSet<'ctx, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ctx, O: ?Sized> WatchSet<'ctx, O> {
    pub fn new() -> Self {
        WatchSet {
            list: Cell::new(None),
        }
    }

    fn with<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut Option<Box<WatchSetHead<'ctx, O>>>) -> R,
    {
        let mut list = self.list.replace(None);
        let ret = func(&mut list);
        self.list.set(list);
        ret
    }

    pub fn empty(&self) -> bool {
        self.with(|list| list.is_none())
    }

    pub(crate) fn add(&self, watch: WatchRef<'ctx, O>, target: &Weak<Self>) {
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

    fn add_all<F>(&self, other: &WatchSet<'ctx, O>, mut filter: F)
    where
        F: FnMut(&WatchRef<'ctx, O>) -> bool,
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

    pub(crate) fn trigger_with_current(&self, current: &Watch<'ctx, O>) {
        if let Some(target) = self
            .with(|list| list.as_mut().and_then(|head| head.target.upgrade()))
        {
            target.add_all(self, |to_add| !to_add.watch_eq(current));
        }
    }

    pub fn trigger_external(&self) {
        if let Some(target) = self
            .with(|list| list.as_mut().and_then(|head| head.target.upgrade()))
        {
            target.add_all(self, |_| true);
        }
    }

    pub fn take(&self) -> Self {
        Self {
            list: Cell::new(self.list.take()),
        }
    }

    pub fn execute(self, ctx: &mut WatchContext<'ctx, O>) {
        let mut node = if let Some(head) = self.list.take() {
            head.node
        } else {
            return;
        };
        loop {
            for bucket in node.data.iter_mut() {
                if let Some(watch) = bucket.take() {
                    watch.execute(ctx);
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
}
