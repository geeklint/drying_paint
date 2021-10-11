/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::rc::{Rc, Weak};

use crate::{WatchSet, WatcherOwner};

pub struct WatchContext<O: ?Sized> {
    next_frame: Rc<WatchSet<O>>,
    frame_limit: Option<usize>,
}

impl<O: ?Sized> WatchContext<O> {
    /// Create a new WatchContext
    pub fn new() -> Self {
        let frame_limit = if cfg!(debug_assertions) {
            Some(1024)
        } else {
            None
        };
        WatchContext {
            next_frame: Rc::default(),
            frame_limit,
        }
    }

    pub fn update(&mut self) {
        //self.chan_ctx.check_for_activity();
        let weak_next = Rc::downgrade(&self.next_frame);
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
                self.next_frame.execute(todo!(), &weak_next);
                frame_limit -= 1;
            }
        } else {
            while !self.next_frame.empty() {
                self.next_frame.execute(todo!(), &weak_next);
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
    /// # use drying_paint::*;
    /// #[derive(Default)]
    /// struct KeepBalanced {
    ///     left: Watched<i32>,
    ///     right: Watched<i32>,
    /// }
    /// impl WatcherInit for KeepBalanced {
    ///     fn init(watcher: &mut WatcherMeta<Self>) {
    ///         watcher.watch(|root| {
    ///             *root.left = *root.right;
    ///         });
    ///         watcher.watch(|root| {
    ///             *root.right = *root.left;
    ///         });
    ///     }
    /// }
    /// fn main() {
    ///     let mut ctx = WatchContext::new();
    ///     ctx.set_frame_limit(Some(100));
    ///     ctx = ctx.with(|| {
    ///         let obj = WatchContext::allow_watcher_access((), |()| {
    ///             let mut obj = Watcher::<KeepBalanced>::new();
    ///             *obj.data_mut().left = 4;
    ///             obj
    ///         });
    ///         // because we used set_frame_limit, this will panic after
    ///         // 100 iterations.
    ///         WatchContext::update_current();
    ///     }).0;
    /// }
    pub fn set_frame_limit(&mut self, value: Option<usize>) {
        self.frame_limit = value;
    }

    /*
    pub(crate) fn channels_context(&self) -> &ChannelsContext {
        &self.chan_ctx
    }
    */
}

impl<O: ?Sized> Default for WatchContext<O> {
    fn default() -> Self {
        Self::new()
    }
}
