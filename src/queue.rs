/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {alloc::collections::VecDeque, core::cell::Cell};

use crate::{trigger::WatchArg, DefaultOwner, WatchedMeta};

pub struct WatchedQueue<'ctx, T, O: ?Sized = DefaultOwner> {
    queue: Cell<VecDeque<T>>,
    current_data: Cell<Option<T>>,
    current_meta: WatchedMeta<'ctx, O>,
    popped_frame_id: Cell<u8>,
}

impl<'ctx, T, O: ?Sized> Default for WatchedQueue<'ctx, T, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ctx, T, O: ?Sized> WatchedQueue<'ctx, T, O> {
    /// Create a new WatchedQueue
    pub fn new() -> Self {
        Self {
            queue: Cell::default(),
            current_data: Cell::default(),
            current_meta: WatchedMeta::new(),
            popped_frame_id: Cell::new(0),
        }
    }

    fn pop_front(&self) -> Option<T> {
        let mut queue = self.queue.take();
        let item = queue.pop_front();
        self.queue.set(queue);
        item
    }

    pub fn handle_item<F: FnOnce(&T)>(
        &self,
        ctx: WatchArg<'_, 'ctx, O>,
        f: F,
    ) {
        self.current_meta.watched(ctx);
        let mut current_data = self.current_data.take();
        if current_data.is_none()
            || self.popped_frame_id.get() != ctx.frame_info.id
        {
            current_data = self.pop_front();
            self.popped_frame_id.set(ctx.frame_info.id);
            if current_data.is_some() {
                self.current_meta.trigger_external();
            }
        }
        if let Some(item) = &current_data {
            f(item);
        }
        self.current_data.set(current_data);
    }

    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn push(&mut self, ctx: WatchArg<'_, 'ctx, O>, item: T) {
        self.queue.get_mut().push_back(item);
        self.current_meta.trigger(ctx);
    }

    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn push_external(&mut self, item: T) {
        self.queue.get_mut().push_back(item);
        self.current_meta.trigger_external();
    }
}

#[cfg(feature = "std")]
impl<T> WatchedQueue<'static, T, DefaultOwner> {
    pub fn push_auto(&mut self, item: T) {
        self.queue.get_mut().push_back(item);
        self.current_meta.trigger_auto();
    }
}
