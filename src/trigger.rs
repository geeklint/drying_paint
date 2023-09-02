/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    alloc::{
        boxed::Box,
        rc::{Rc, Weak},
        vec::Vec,
    },
    core::{cell::Cell, mem},
};

use crate::context::{FrameInfo, WatchContext};

struct WatchData<F: ?Sized> {
    cycle: Cell<usize>,
    #[cfg_attr(not(do_cycle_debug), allow(dead_code))]
    debug_name: WatchName,
    update_fn: F,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatchName {
    #[cfg(do_cycle_debug)]
    pub(crate) inner: watch_name::Inner,
}

#[cfg(not(do_cycle_debug))]
mod watch_name {
    impl From<&'static str> for super::WatchName {
        fn from(value: &'static str) -> Self {
            let _unused = value;
            Self {}
        }
    }

    impl super::WatchName {
        pub fn from_caller() -> Self {
            Self {}
        }
    }
}

#[cfg(do_cycle_debug)]
pub(crate) mod watch_name {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum Inner {
        Name(&'static str),
        SpawnLocation(&'static core::panic::Location<'static>),
    }

    impl From<&'static str> for super::WatchName {
        fn from(value: &'static str) -> Self {
            Self {
                inner: Inner::Name(value),
            }
        }
    }

    impl super::WatchName {
        #[track_caller]
        pub fn from_caller() -> Self {
            let loc = core::panic::Location::caller();
            Self {
                inner: Inner::SpawnLocation(loc),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TriggerReason {
    #[cfg(do_cycle_debug)]
    location: &'static core::panic::Location<'static>,
    #[cfg(do_cycle_debug)]
    source_watch: *const (),
}

#[cfg(not(do_cycle_debug))]
impl TriggerReason {
    pub fn from_caller() -> Self {
        Self {}
    }

    pub fn with_source<O>(self, source_watch: &Watch<'_, O>) -> Self
    where
        O: ?Sized,
    {
        let _unused = source_watch;
        Self {}
    }
}

#[cfg(do_cycle_debug)]
impl TriggerReason {
    #[track_caller]
    pub fn from_caller() -> Self {
        let location = core::panic::Location::caller();
        let source_watch = core::ptr::null();
        Self {
            location,
            source_watch,
        }
    }

    pub fn with_source<O>(self, source_watch: &Watch<'_, O>) -> Self
    where
        O: ?Sized,
    {
        let source_watch = Rc::as_ptr(&source_watch.0).cast();
        Self {
            source_watch,
            ..self
        }
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

        pub fn try_with_current<F>(f: F) -> Option<()>
        where
            F: FnOnce(WatchArg<'_, 'static, DefaultOwner>),
        {
            CURRENT_ARG.with(|cell| {
                // TODO: re-entrence?
                let owned = cell.take()?;
                let OwnedWatchArg(ref watch, ref frame_info) = owned;
                f(WatchArg { watch, frame_info });
                cell.set(Some(owned));
                Some(())
            })
        }
    }
}

pub struct RawWatchArg<'a, 'ctx, O: ?Sized> {
    ctx: &'a mut WatchContext<'ctx, O>,
    watch: &'a Watch<'ctx, O>,
}

impl<'a, 'ctx, O: ?Sized> RawWatchArg<'a, 'ctx, O> {
    pub fn context(&mut self) -> &mut WatchContext<'ctx, O> {
        self.ctx
    }

    pub fn as_owner_and_arg(&mut self) -> (&mut O, WatchArg<'_, 'ctx, O>) {
        let Self { ctx, watch } = self;
        let WatchContext {
            owner, frame_info, ..
        } = ctx;
        (owner, WatchArg { watch, frame_info })
    }
}

type WatchFn<'ctx, O> = dyn 'ctx + Fn(RawWatchArg<'_, 'ctx, O>);

pub(crate) struct Watch<'ctx, O: ?Sized>(Rc<WatchData<WatchFn<'ctx, O>>>);

impl<'ctx, O: ?Sized> Clone for Watch<'ctx, O> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<'ctx, O: ?Sized> Watch<'ctx, O> {
    pub(crate) fn spawn_raw<F>(
        ctx: &mut WatchContext<'ctx, O>,
        debug_name: WatchName,
        update_fn: F,
    ) where
        F: 'ctx + Fn(RawWatchArg<'_, 'ctx, O>),
    {
        let this = Watch(Rc::new(WatchData {
            update_fn,
            debug_name,
            cycle: Cell::new(0),
        }));
        this.get_ref().execute(ctx);
    }

    pub(crate) fn get_ref(&self) -> WatchRef<'ctx, O> {
        WatchRef {
            watch: self.clone(),
            cycle: self.0.cycle.get(),
        }
    }
}

pub(crate) struct WatchRef<'ctx, O: ?Sized> {
    watch: Watch<'ctx, O>,
    cycle: usize,
}

impl<'ctx, O: ?Sized> WatchRef<'ctx, O> {
    pub fn watch_eq(&self, other: &Watch<'ctx, O>) -> bool {
        Rc::ptr_eq(&self.watch.0, &other.0)
    }

    fn is_fresh(&self) -> bool {
        self.cycle == self.watch.0.cycle.get()
    }

    fn execute(self, ctx: &mut WatchContext<'ctx, O>) {
        if self.is_fresh() {
            self.watch.0.cycle.set(self.cycle + 1);
            let raw_arg = RawWatchArg {
                ctx,
                watch: &self.watch,
            };
            (self.watch.0.update_fn)(raw_arg);
        }
    }
}

pub(crate) struct TriggeredWatch<'ctx, O: ?Sized> {
    watch: WatchRef<'ctx, O>,
    #[cfg_attr(not(do_cycle_debug), allow(dead_code))]
    reason: TriggerReason,
}

impl<'ctx, O: ?Sized> TriggeredWatch<'ctx, O> {
    pub(crate) fn execute(self, ctx: &mut WatchContext<'ctx, O>) {
        self.watch.execute(ctx);
    }
}

#[cfg(do_cycle_debug)]
impl<'ctx, O: ?Sized> TriggeredWatch<'ctx, O> {
    pub(crate) fn is_fresh(&self) -> bool {
        self.watch.is_fresh()
    }

    pub(crate) fn order(&self) -> impl Ord {
        (self.watch.watch.0.debug_name, self.reason)
    }

    pub(crate) fn watch_name(&self) -> WatchName {
        self.watch.watch.0.debug_name
    }

    pub(crate) fn to_edge(&self) -> (*const (), *const ()) {
        (
            self.reason.source_watch,
            Rc::as_ptr(&self.watch.watch.0).cast(),
        )
    }

    pub(crate) fn trigger_location(
        &self,
    ) -> &'static core::panic::Location<'static> {
        self.reason.location
    }

    pub(crate) fn clone_watch(&self) -> Watch<'ctx, O> {
        self.watch.watch.clone()
    }
}

pub(crate) type WatchFrame<'ctx, O> = Cell<Vec<TriggeredWatch<'ctx, O>>>;

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
    target: Weak<WatchFrame<'ctx, O>>,
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

    pub(crate) fn add(
        &self,
        watch: WatchRef<'ctx, O>,
        target: &Weak<WatchFrame<'ctx, O>>,
    ) {
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

    fn trigger_filtered<F>(&self, reason: TriggerReason, mut filter: F)
    where
        F: FnMut(&WatchRef<'ctx, O>) -> bool,
    {
        if let Some(head) = self.list.take() {
            if let Some(target_box) = head.target.upgrade() {
                let mut target = target_box.take();
                let mut node = head.node;
                loop {
                    for bucket in node.data.iter_mut() {
                        if let Some(watch) = bucket.take().filter(&mut filter)
                        {
                            target.push(TriggeredWatch { watch, reason })
                        }
                    }
                    node = if let Some(next) = node.next {
                        *next
                    } else {
                        break;
                    };
                }
                target_box.set(target);
            }
        }
    }

    pub(crate) fn trigger_with_current(
        &self,
        current: &Watch<'ctx, O>,
        reason: TriggerReason,
    ) {
        self.trigger_filtered(reason, |to_add| !to_add.watch_eq(current));
    }

    pub fn trigger_external(&self, reason: TriggerReason) {
        self.trigger_filtered(reason, |_| true);
    }
}
