/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    alloc::sync::{Arc, Weak},
    core::{
        cell::Cell,
        fmt, mem, ptr,
        sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
    },
};

use crate::{trigger::WatchArg, WatchedMeta};

const FLAG_COUNT: usize = usize::BITS as usize;

pub(crate) struct SyncContext<'ctx, O: ?Sized> {
    flag: Arc<AtomicUsize>,
    watched: [WatchedMeta<'ctx, O>; FLAG_COUNT],
    next_index: Cell<usize>,
}

impl<'ctx, O: ?Sized> SyncContext<'ctx, O> {
    pub fn new() -> Self {
        Self {
            flag: Arc::default(),
            watched: [0; FLAG_COUNT].map(|_| WatchedMeta::new()),
            next_index: Cell::new(0),
        }
    }

    pub fn check_for_updates(&self) {
        let set_bits = self.flag.swap(0, Ordering::Acquire);
        for i in 0..FLAG_COUNT {
            if (set_bits & (1 << i)) != 0 {
                self.watched[i].trigger_external();
            }
        }
    }
}

struct FlagPole {
    ptr: AtomicPtr<AtomicUsize>,
}

impl Drop for FlagPole {
    fn drop(&mut self) {
        let flag_ptr: *mut AtomicUsize = *self.ptr.get_mut();
        if !flag_ptr.is_null() {
            // drop one weak reference
            unsafe {
                Weak::from_raw(flag_ptr);
            }
        }
    }
}

impl Default for FlagPole {
    fn default() -> Self {
        Self {
            ptr: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl FlagPole {
    fn set(&self, value: Weak<AtomicUsize>) {
        let flag_ptr = value.into_raw() as *mut AtomicUsize;
        // Store the new value only if the current value is null
        if self
            .ptr
            .compare_exchange(
                ptr::null_mut(),
                flag_ptr,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            // If the store failed, ensure the ref count is
            // properly decremented
            unsafe {
                Weak::from_raw(flag_ptr);
            }
        }
    }

    fn get(&self) -> Weak<AtomicUsize> {
        let flag_ptr = self.ptr.load(Ordering::Acquire);
        if flag_ptr.is_null() {
            Weak::new()
        } else {
            let current = unsafe { Weak::from_raw(flag_ptr) };
            // increment one weak ref before returning, so the pointer
            // stored in the atomic remains valid
            mem::forget(Weak::clone(&current));
            current
        }
    }
}

#[derive(Default)]
struct SharedMeta {
    flag_pole: FlagPole,
    mask: AtomicUsize,
}

/// SyncWatchedMeta is like WatchedMeta, however allows you to create
/// a trigger which may be sent to other threads.
///
/// When this trigger is invoked, watch functions in the single-threaded watch
/// context will be re-run.
pub struct SyncWatchedMeta {
    data: Arc<SharedMeta>,
    index: Cell<usize>,
}

impl Default for SyncWatchedMeta {
    fn default() -> Self {
        Self {
            data: Arc::default(),
            index: Cell::new(usize::MAX),
        }
    }
}

impl fmt::Debug for SyncWatchedMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(SyncWatchedMeta)")
    }
}

impl SyncWatchedMeta {
    /// Create a new AtomicWatchedMeta
    pub fn new() -> Self {
        Self::default()
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when a trigger associated with this
    /// AtomicWatchedMeta is invoked.
    pub fn watched<O: ?Sized>(&self, ctx: WatchArg<'_, '_, O>) {
        if let Some(sctx) = ctx.frame_info.sync_context.upgrade() {
            if self.index.get() == usize::MAX {
                let index = sctx.next_index.get();
                sctx.next_index.set(index + 1 % FLAG_COUNT);
                let mask = 1 << index;
                let weak_flag = Arc::downgrade(&sctx.flag);
                self.data.mask.store(mask, Ordering::Relaxed);
                self.data.flag_pole.set(weak_flag);
                self.index.set(index);
            }
            sctx.watched[self.index.get()].watched(ctx);
        }
    }

    /// Create a trigger for this AtomicWatchedMeta which may be sent to
    /// another thread.
    pub fn create_trigger(&self) -> SyncTrigger {
        SyncTrigger {
            data: Arc::downgrade(&self.data),
        }
    }
}

/// A type which can be used from another thread to trigger watch functions
/// watching an AtomicWatchedMeta.
#[derive(Clone)]
pub struct SyncTrigger {
    data: Weak<SharedMeta>,
}

impl fmt::Debug for SyncTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(SyncTrigger)")
    }
}

impl SyncTrigger {
    /// Create an SyncTrigger which is not assocaited with any
    /// SyncWatchedMeta.  Invoking the trigger returned from this function
    /// will do nothing.  This may be useful e.g. as a placeholder value.
    pub fn new_inert() -> Self {
        Self { data: Weak::new() }
    }

    pub fn trigger(&self) {
        if let Some(data) = self.data.upgrade() {
            if let Some(flag) = data.flag_pole.get().upgrade() {
                let mask = data.mask.load(Ordering::Relaxed);
                flag.fetch_or(mask, Ordering::Release);
            }
        }
    }
}

pub fn watched_channel<S, R>(
    pair: (S, R),
) -> (WatchedSender<S>, WatchedReceiver<R>) {
    let (sender, receiver) = pair;
    let meta = SyncWatchedMeta::new();
    let trigger = meta.create_trigger();
    (
        WatchedSender { sender, trigger },
        WatchedReceiver { receiver, meta },
    )
}

/// The sender half of a watched channel.
#[derive(Clone, Debug)]
pub struct WatchedSender<S: ?Sized> {
    trigger: SyncTrigger,
    sender: S,
}

impl<S: ?Sized> Drop for WatchedSender<S> {
    fn drop(&mut self) {
        self.trigger.trigger();
    }
}

impl<S: ?Sized> WatchedSender<S> {
    pub fn sender(&self) -> SendGuard<'_, S> {
        SendGuard { origin: self }
    }

    pub fn trigger_receiver(&self) {
        self.trigger.trigger();
    }
}

pub struct SendGuard<'a, S: ?Sized> {
    origin: &'a WatchedSender<S>,
}

impl<'a, S: ?Sized> core::ops::Deref for SendGuard<'a, S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.origin.sender
    }
}

impl<'a, S: ?Sized> Drop for SendGuard<'a, S> {
    fn drop(&mut self) {
        self.origin.trigger.trigger();
    }
}

#[derive(Debug)]
pub struct WatchedReceiver<R: ?Sized> {
    meta: SyncWatchedMeta,
    receiver: R,
}

impl<R: ?Sized> WatchedReceiver<R> {
    pub fn get<O: ?Sized>(&self, ctx: WatchArg<'_, '_, O>) -> &R {
        self.meta.watched(ctx);
        &self.receiver
    }

    pub fn get_mut<O: ?Sized>(&mut self, ctx: WatchArg<'_, '_, O>) -> &mut R {
        self.meta.watched(ctx);
        &mut self.receiver
    }
}
