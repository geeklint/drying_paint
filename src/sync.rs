/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {
    alloc::sync::{Arc, Weak},
    core::{
        cell::Cell,
        mem::{self, size_of},
        ptr,
        sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
    },
};

use super::{WatchArg, WatchedMeta};

const FLAG_COUNT: usize = size_of::<usize>() * 8;

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

impl SyncWatchedMeta {
    /// Create a new AtomicWatchedMeta
    pub fn new() -> Self {
        Self::default()
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when a trigger associated with this
    /// AtomicWatchedMeta is invoked.
    pub fn watched<'ctx, O: ?Sized>(&self, ctx: WatchArg<'_, 'ctx, O>) {
        if let Some(sctx) = ctx.frame_info.sync_context.upgrade() {
            if !self.index.get() == usize::MAX {
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

/*
pub fn watched_channel<T>() -> (WatchedSender<T>, WatchedReceiver<T>) {
    let meta = AtomicWatchedMeta::new();
    let (sender, receiver) = mpsc::channel::<T>();
    (
        WatchedSender {
            inner: sender,
            trigger: meta.create_trigger(),
        },
        WatchedReceiver {
            inner: receiver,
            meta,
        },
    )
}

/// The sender half of a watched channel.
#[derive(Clone, Debug)]
pub struct WatchedSender<T> {}

impl<T> Drop for WatchedSender<T> {
    fn drop(&mut self) {
        self.trigger.trigger();
    }
}

/// The receiver half of a watched channel.
///
/// The methods exposed on this type corospond to the non-blocking methods
/// on the
/// [std channel Receiver](https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html),
/// but they also bind watch closures, so that when new data is sent those
/// closures will be re-run.
#[derive(Debug)]
pub struct WatchedReceiver<T> {
    inner: mpsc::Receiver<T>,
    meta: AtomicWatchedMeta,
}

impl<T> WatchedReceiver<T> {
    /// Attempts to return a pending value on this receiver.
    ///
    /// This corosponds to the `try_recv` method on the std Receiver, but
    /// additionally binds enclosing watch closures, so that they will be
    /// re-run when new data might be available.
    pub fn recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.meta.watched();
        self.inner.try_recv()
    }

    /// Returns an iterator that will attempt to yield all pending values.
    ///
    /// This corosponds to the `try_iter` method on the std Receiver, but
    /// additionally binds enclosing watch closures, so that they will be
    /// re-run when new data might be available.
    pub fn iter(&self) -> mpsc::TryIter<T> {
        self.meta.watched();
        self.inner.try_iter()
    }
}
*/
