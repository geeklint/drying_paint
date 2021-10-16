/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use core::cell::Cell;

use crate::{DefaultOwner, WatchArg, WatchSet};

/// This provides the basic functionality behind watched values. You can use
/// it to provide functionality using the watch system for cases where
/// [Watched](struct.Watched.html) and
/// [WatchedEvent](struct.WatchedEvent.html) are not appropriate.
#[derive(Default)]
pub struct WatchedMeta<'ctx, O: ?Sized = DefaultOwner> {
    watchers: WatchSet<'ctx, O>,
}

impl<'ctx, O: ?Sized> WatchedMeta<'ctx, O> {
    /// Create a new WatchedMeta instance
    pub fn new() -> Self {
        WatchedMeta {
            watchers: WatchSet::new(),
        }
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when this is triggered.
    pub fn watched(&self, ctx: WatchArg<'_, 'ctx, O>) {
        self.watchers.add(ctx.watch.get_ref(), ctx.post_set);
    }

    /// Mark this value as having changed, so that watching functions will
    /// be marked as needing to be updated.
    pub fn trigger(&self, ctx: WatchArg<'_, 'ctx, O>) {
        self.watchers.trigger_with_current(ctx.watch);
    }

    pub fn trigger_external(&self) {
        self.watchers.trigger_external();
    }
}

#[cfg(feature = "std")]
impl WatchedMeta<'static, DefaultOwner> {
    pub fn watched_auto(&self) {
        WatchArg::try_with_current(|arg| self.watched(arg));
    }

    pub fn trigger_auto(&self) {
        if WatchArg::try_with_current(|arg| self.trigger(arg)).is_none() {
            self.trigger_external();
        }
    }
}

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
pub struct WatchedCore<'ctx, T: ?Sized, O: ?Sized = DefaultOwner> {
    meta: WatchedMeta<'ctx, O>,
    value: T,
}

impl<'ctx, T: Default, O: ?Sized> Default for WatchedCore<'ctx, T, O> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<'ctx, T, O: ?Sized> From<T> for WatchedCore<'ctx, T, O> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<'ctx, T, O: ?Sized> WatchedCore<'ctx, T, O> {
    /// Create a new watched value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            meta: WatchedMeta::new(),
        }
    }

    /// Consumes the `WatchedCore`, returning the wrapped value
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Takes the wrapped value, with a new one, returning the old value,
    /// without deinitializing either one, and notifies watchers that the
    /// value has changed.
    pub fn replace(&mut self, value: T, ctx: WatchArg<'_, 'ctx, O>) -> T {
        core::mem::replace(self.get_mut(ctx), value)
    }
}

impl<'ctx, T: ?Sized, O: ?Sized> WatchedCore<'ctx, T, O> {
    /// Get a referenced to the wrapped value, binding a watch closure.
    pub fn get(&self, ctx: WatchArg<'_, 'ctx, O>) -> &T {
        self.meta.watched(ctx);
        &self.value
    }

    /// Get a mutable referenced to the wrapped value, notifying
    /// watchers that the value has changed.
    pub fn get_mut(&mut self, ctx: WatchArg<'_, 'ctx, O>) -> &mut T {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        &mut self.value
    }

    pub fn get_mut_external(&mut self) -> &mut T {
        self.meta.trigger_external();
        &mut self.value
    }

    /// Get a referenced to the wrapped value, without binding any
    /// watch closure.
    pub fn get_unwatched(&self) -> &T {
        &self.value
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized> WatchedCore<'static, T, DefaultOwner> {
    pub fn get_auto(&self) -> &T {
        self.meta.watched_auto();
        &self.value
    }

    pub fn get_mut_auto(&mut self) -> &mut T {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        &mut self.value
    }
}

impl<'ctx, T: Default, O: ?Sized> WatchedCore<'ctx, T, O> {
    /// Takes the wrapped value, leaving `Default::default()` in its place,
    /// and notifies watchers that the value has changed.
    pub fn take(&mut self, ctx: WatchArg<'_, 'ctx, O>) -> T {
        core::mem::take(self.get_mut(ctx))
    }
}

impl<'ctx, T: PartialEq, O: ?Sized> WatchedCore<'ctx, T, O> {
    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    pub fn set_if_neq(&mut self, value: T, ctx: WatchArg<'_, 'ctx, O>) {
        if self.value != value {
            self.value = value;
            self.meta.trigger(ctx);
        }
    }
}

#[cfg(feature = "std")]
impl<T: PartialEq> WatchedCore<'static, T, DefaultOwner> {
    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    pub fn set_if_neq_auto(&mut self, value: T) {
        if self.value != value {
            self.value = value;
            self.meta.trigger_auto();
        }
    }
}

/// A Watched value which provides interior mutability.  This provides correct
/// behavior (triggering watch functions when changed) where `Watched<Cell<T>>`
/// would not, and should be slightly more performant than
/// `RefCell<Watched<T>>`.
pub struct WatchedCellCore<'ctx, T: ?Sized, O: ?Sized = DefaultOwner> {
    meta: WatchedMeta<'ctx, O>,
    value: Cell<T>,
}

impl<'ctx, T: Default, O: ?Sized> Default for WatchedCellCore<'ctx, T, O> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<'ctx, T, O: ?Sized> From<T> for WatchedCellCore<'ctx, T, O> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<'ctx, T: ?Sized, O: ?Sized> WatchedCellCore<'ctx, T, O> {
    /// Returns a mutable reference to the watched data.
    ///
    /// This call borrows the WatchedCell mutably (at compile-time) which
    /// guarantees that we possess the only reference.
    pub fn get_mut(&mut self, ctx: WatchArg<'_, 'ctx, O>) -> &mut T {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        self.value.get_mut()
    }

    /// Treat this WatchedCell as watched, without fetching the actual value.
    pub fn watched(&self, ctx: WatchArg<'_, 'ctx, O>) {
        self.meta.watched(ctx);
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized> WatchedCellCore<'static, T, DefaultOwner> {
    pub fn get_mut_auto(&mut self) -> &mut T {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.get_mut()
    }

    pub fn watched_auto(&self) {
        self.meta.watched_auto();
    }
}

impl<'ctx, T, O: ?Sized> WatchedCellCore<'ctx, T, O> {
    /// Create a new WatchedCell
    pub fn new(value: T) -> Self {
        Self {
            meta: WatchedMeta::new(),
            value: Cell::new(value),
        }
    }

    /// Sets the watched value
    pub fn set(&self, value: T, ctx: WatchArg<'_, 'ctx, O>) {
        self.meta.trigger(ctx);
        self.value.set(value);
    }

    /// Unwraps the WatchedCell, returning the contained value
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Replaces the contained value and returns the previous value
    pub fn replace(&self, value: T, ctx: WatchArg<'_, 'ctx, O>) -> T {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        self.value.replace(value)
    }
}

#[cfg(feature = "std")]
impl<T> WatchedCellCore<'static, T, DefaultOwner> {
    pub fn set_auto(&self, value: T) {
        self.meta.trigger_auto();
        self.value.set(value);
    }

    pub fn replace_auto(&self, value: T) -> T {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.replace(value)
    }
}

impl<'ctx, T: Copy, O: ?Sized> WatchedCellCore<'ctx, T, O> {
    /// Returns a copy of the watched value
    pub fn get(&self, ctx: WatchArg<'_, 'ctx, O>) -> T {
        self.meta.watched(ctx);
        self.value.get()
    }

    pub fn get_unwatched(&self) -> T {
        self.value.get()
    }
}

#[cfg(feature = "std")]
impl<T: Copy> WatchedCellCore<'static, T, DefaultOwner> {
    pub fn get_auto(&self) -> T {
        self.meta.watched_auto();
        self.value.get()
    }
}

impl<'ctx, T: Default, O: ?Sized> WatchedCellCore<'ctx, T, O> {
    /// Takes the watched value, leaving `Default::default()` in its place
    pub fn take(&self, ctx: WatchArg<'_, 'ctx, O>) -> T {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        self.value.take()
    }
}

#[cfg(feature = "std")]
impl<T: Default> WatchedCellCore<'static, T, DefaultOwner> {
    pub fn take_auto(&self) -> T {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.take()
    }
}
