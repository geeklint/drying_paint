/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    core::any::Any,
    std::rc::{Rc, Weak},
};

use crate::{WatchSet, WatcherHolder};

pub struct WatchContext<'ctx, O = DefaultOwner> {
    next_frame: Rc<WatchSet<'ctx, O>>,
    next_frame_weak: Weak<WatchSet<'ctx, O>>,
    frame_limit: Option<usize>,
    owner: O,
}

impl<'ctx, O: Default> WatchContext<'ctx, O> {
    /// Create a new WatchContext
    pub fn new() -> Self {
        let frame_limit = if cfg!(debug_assertions) {
            Some(1024)
        } else {
            None
        };
        let next_frame = Rc::default();
        let next_frame_weak = Rc::downgrade(&next_frame);
        WatchContext {
            next_frame,
            next_frame_weak,
            frame_limit,
            owner: O::default(),
        }
    }
}

impl<'ctx, O> WatchContext<'ctx, O> {
    pub fn add_watcher<T>(&mut self, holder: &T)
    where
        T: 'ctx + ?Sized + WatcherHolder<'ctx, O>,
    {
        crate::watcher::init_watcher(
            &self.next_frame_weak,
            holder,
            &mut self.owner,
        );
    }

    pub fn owner(&mut self) -> &mut O {
        &mut self.owner
    }

    pub fn update(&mut self) {
        //self.chan_ctx.check_for_activity();
        if let Some(mut frame_limit) = self.frame_limit {
            while !self.next_frame.empty() {
                if frame_limit == 0 {
                    let current_watch_names = "TODO";
                    //self.back_frame.debug_names();
                    panic!(
                        "Updating a WatchContext exceeded it's \
                        limit for iteration.  This usually means there is a \
                        recursive watch.  You may be interested in \
                        Watched::set_if_neq to resolve recursive watches.  \
                        If the number of iterations was intentional, you \
                        can try increasing the limit with \
                        WatchContext::set_frame_limit.  The following types \
                        might be involved in the recursive watch:\n  {}",
                        current_watch_names,
                    );
                }
                self.next_frame
                    .execute(&mut self.owner, &self.next_frame_weak);
                frame_limit -= 1;
            }
        } else {
            while !self.next_frame.empty() {
                self.next_frame
                    .execute(&mut self.owner, &self.next_frame_weak);
            }
        }
    }

    /// Set the number of cycles this watch context will execute before
    /// panicking. This is useful for catching bugs involving recursive
    /// watches. None indicates no limit. The default behaviour is to provide
    /// a high value for debug builds and no limit for release builds.
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
