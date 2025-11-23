/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    alloc::{
        boxed::Box,
        rc::{Rc, Weak},
        vec::Vec,
    },
    core::{any::Any, cell::Cell},
};

use crate::{
    sync::SyncContext,
    trigger::{TriggeredWatch, Watch, WatchFrame},
    RawWatchArg, WatchArg, WatchName, WatcherHolder,
};

#[cfg(all(feature = "std", doc))]
use crate::Watched;

pub(crate) struct FrameInfo<'ctx, O: ?Sized> {
    pub(crate) id: u8,
    pub(crate) post_set: Weak<WatchFrame<'ctx, O>>,
    pub(crate) sync_context: Weak<SyncContext<'ctx, O>>,
}

impl<'ctx, O: ?Sized> Clone for FrameInfo<'ctx, O> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            post_set: Weak::clone(&self.post_set),
            sync_context: Weak::clone(&self.sync_context),
        }
    }
}

pub struct WatchContext<'ctx, O: ?Sized = DefaultOwner> {
    next_frame: Rc<WatchFrame<'ctx, O>>,
    other_frame: Vec<TriggeredWatch<'ctx, O>>,
    sync_context: Rc<SyncContext<'ctx, O>>,
    pub(crate) frame_info: FrameInfo<'ctx, O>,
    pub(crate) total_watch_count: usize,
    frame_limit: Option<usize>,
    pub(crate) owner: O,
}

impl<'ctx, O> WatchContext<'ctx, O> {
    pub fn from_owner(owner: O) -> Self {
        let frame_limit = if cfg!(debug_assertions) {
            Some(1024)
        } else {
            None
        };
        let next_frame = Rc::default();
        let other_frame = Vec::new();
        let sync_context = Rc::new(SyncContext::new());
        let frame_info = FrameInfo {
            id: 0,
            post_set: Rc::downgrade(&next_frame),
            sync_context: Rc::downgrade(&sync_context),
        };
        let total_watch_count = 0;
        WatchContext {
            next_frame,
            other_frame,
            sync_context,
            frame_info,
            total_watch_count,
            frame_limit,
            owner,
        }
    }
}

impl<'ctx, O: Default> WatchContext<'ctx, O> {
    /// Create a new WatchContext
    pub fn new() -> Self {
        Self::from_owner(O::default())
    }
}

impl<'ctx, O: ?Sized> WatchContext<'ctx, O> {
    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn add_watch<F>(&mut self, func: F)
    where
        F: 'ctx + Fn(&mut O, WatchArg<'_, 'ctx, O>),
    {
        let debug_name = WatchName::from_caller();
        self.add_watch_raw(debug_name, move |mut raw_arg| {
            let (owner, arg) = raw_arg.as_owner_and_arg();
            func(owner, arg);
        });
    }

    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn add_watch_might_add_watcher<F, T>(&mut self, func: F)
    where
        F: 'ctx + Fn(&mut O, WatchArg<'_, 'ctx, O>) -> Option<T>,
        T: 'ctx + WatcherHolder<'ctx, O>,
        T::Content: crate::Watcher<'ctx, O>,
    {
        let debug_name = WatchName::from_caller();
        self.add_watch_raw(debug_name, move |mut raw_arg| {
            let (owner, arg) = raw_arg.as_owner_and_arg();
            if let Some(watcher) = func(owner, arg) {
                raw_arg.context().add_watcher(&watcher);
            }
        });
    }

    pub fn add_watch_raw<F, N>(&mut self, debug_name: N, f: F)
    where
        F: 'ctx + Fn(RawWatchArg<'_, 'ctx, O>),
        N: Into<WatchName>,
    {
        self.total_watch_count = self.total_watch_count.saturating_add(1);
        Watch::spawn_raw(self, debug_name.into(), f)
    }

    pub fn add_watcher<T>(&mut self, holder: &T)
    where
        T: 'ctx + WatcherHolder<'ctx, O>,
        T::Content: crate::Watcher<'ctx, O>,
    {
        crate::watcher::init_watcher(self, holder);
    }

    pub fn owner(&mut self) -> &mut O {
        &mut self.owner
    }

    pub fn update(&mut self) {
        self.sync_context.check_for_updates();
        let mut current_frame = core::mem::take(&mut self.other_frame);
        self.next_frame.swap(Cell::from_mut(&mut current_frame));
        if let Some(mut frame_limit) = self.frame_limit {
            let panic_msg =
                "\nUpdating a WatchContext exceeded its limit for iteration.\nSee \
                `WatchContext::set_frame_limit` for more information.\nThis usually \
                means there are cyclical watch triggers."
            ;
            #[cfg(do_cycle_debug)]
            let mut debug = crate::cycle_debug::CycleDiagnostic::new();
            while !current_frame.is_empty() {
                #[cfg(do_cycle_debug)]
                {
                    if frame_limit < 5 {
                        debug.track_frame(&current_frame);
                    }
                    if frame_limit == 0 {
                        debug.do_panic(panic_msg, current_frame);
                    }
                }
                if frame_limit == 0 {
                    panic!("{}", panic_msg)
                }
                for item in current_frame.drain(..) {
                    item.execute(self);
                }
                self.next_frame.swap(Cell::from_mut(&mut current_frame));
                self.frame_info.id = self.frame_info.id.wrapping_add(1);
                frame_limit -= 1;
            }
        } else {
            while !current_frame.is_empty() {
                for item in current_frame.drain(..) {
                    item.execute(self);
                }
                self.next_frame.swap(Cell::from_mut(&mut current_frame));
                self.frame_info.id = self.frame_info.id.wrapping_add(1);
            }
        }
        self.other_frame = current_frame;
    }

    /// Set the number of cycles this watch context will execute before
    /// panicking. This is useful for catching bugs involving cyclical
    /// watch triggers. None indicates no limit. The default behaviour is to
    /// provide a high value for debug builds and no limit for release builds.
    ///
    /// If you get an unwanted panic because your use case runs up against
    /// the default limit without any truely unbounded cycle, you can use
    /// this function to increase or disable the limit.
    ///
    /// # Avoiding cyclical watch triggers
    ///
    /// Generally recursive watches should be avoided, but one valid use case
    /// is to keep two values in a kind of mutual sync where changing either
    /// value updates the other.  For this purpose, you may be interested in
    /// a function which only triggers a watch if the value has actually
    /// changed, such as [`Watched::set_if_neq`].  The following example
    /// panics, but it wouldn't if used [`Watched::set_if_neq`].
    ///
    /// # Examples
    /// ```rust,should_panic
    ///# use std::{rc::Rc, cell::RefCell};
    ///# use drying_paint::{Watcher, Watched, WatcherInit, WatchContext};
    /// #[derive(Default)]
    /// struct KeepBalanced {
    ///     left: Watched<i32>,
    ///     right: Watched<i32>,
    /// }
    ///
    /// impl Watcher<'static> for KeepBalanced {
    ///     fn init(mut init: impl WatcherInit<'static, Self>) {
    ///         init.watch(|root| {
    ///             *root.left = *root.right;
    ///         });
    ///         init.watch(|root| {
    ///             *root.right = *root.left;
    ///         });
    ///     }
    /// }
    ///
    /// let keep_balanced = Rc::new(RefCell::new(KeepBalanced {
    ///     left: Watched::new(7),
    ///     right: Watched::new(7),
    /// }));
    /// let weak = Rc::downgrade(&keep_balanced);
    /// let mut ctx = WatchContext::new();
    /// ctx.set_frame_limit(Some(1000));
    /// ctx.add_watcher(&weak);
    /// ctx.update();
    /// ```
    pub fn set_frame_limit(&mut self, value: Option<usize>) {
        self.frame_limit = value;
    }

    /*
    pub(crate) fn channels_context(&self) -> &ChannelsContext {
        &self.chan_ctx
    }
    */
}

impl<'ctx, O: Default> Default for WatchContext<'ctx, O> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct DefaultOwner {
    owners: Vec<Box<dyn Any>>,
}

impl DefaultOwner {
    pub fn add_owner<T: Any>(&mut self, owner: T) {
        self.owners.push(Box::new(owner));
    }

    pub fn get_owner<T: Any>(&mut self) -> impl Iterator<Item = &mut T> {
        self.owners
            .iter_mut()
            .filter_map(|boxed| boxed.downcast_mut::<T>())
    }
}
