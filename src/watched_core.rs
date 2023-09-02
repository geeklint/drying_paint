/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use core::cell::Cell;

use crate::{DefaultOwner, WatchArg, WatchSet};

/// This provides the basic functionality behind watched values. You can use
/// it to provide functionality using the watch system for cases where
/// [Watched](struct.Watched.html) and
/// [WatchedEvent](struct.WatchedEvent.html) are not appropriate.
pub struct WatchedMeta<'ctx, O: ?Sized = DefaultOwner> {
    watchers: WatchSet<'ctx, O>,
}

impl<'ctx, O: ?Sized> Default for WatchedMeta<'ctx, O> {
    fn default() -> Self {
        Self {
            watchers: WatchSet::default(),
        }
    }
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
        self.watchers
            .add(ctx.watch.get_ref(), &ctx.frame_info.post_set);
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

    pub fn replace_external(&mut self, value: T) -> T {
        core::mem::replace(self.get_mut_external(), value)
    }

    /// Takes the wrapped value, leaving `Default::default()` in its place,
    /// and notifies watchers that the value has changed.
    pub fn take(&mut self, ctx: WatchArg<'_, 'ctx, O>) -> T
    where
        T: Default,
    {
        core::mem::take(self.get_mut(ctx))
    }

    pub fn take_external(&mut self) -> T
    where
        T: Default,
    {
        core::mem::take(self.get_mut_external())
    }

    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    pub fn set_if_neq(&mut self, value: T, ctx: WatchArg<'_, 'ctx, O>)
    where
        T: PartialEq,
    {
        if self.value != value {
            self.value = value;
            self.meta.trigger(ctx);
        }
    }

    pub fn set_if_neq_external(&mut self, value: T)
    where
        T: PartialEq,
    {
        if self.value != value {
            self.value = value;
            self.meta.trigger_external();
        }
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

    pub fn replace_auto(&mut self, value: T) -> T
    where
        T: Sized,
    {
        core::mem::replace(self.get_mut_auto(), value)
    }

    pub fn take_auto(&mut self) -> T
    where
        T: Default,
    {
        core::mem::take(self.get_mut_auto())
    }

    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    pub fn set_if_neq_auto(&mut self, value: T)
    where
        T: PartialEq + Sized,
    {
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

    pub fn get_mut_external(&mut self) -> &mut T {
        self.meta.trigger_external();
        self.value.get_mut()
    }

    /// Treat this WatchedCell as watched, without fetching the actual value.
    pub fn watched(&self, ctx: WatchArg<'_, 'ctx, O>) {
        self.meta.watched(ctx);
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

    /// Unwraps the WatchedCell, returning the contained value
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Returns a copy of the watched value
    pub fn get(&self, ctx: WatchArg<'_, 'ctx, O>) -> T
    where
        T: Copy,
    {
        self.meta.watched(ctx);
        self.value.get()
    }

    pub fn get_unwatched(&self) -> T
    where
        T: Copy,
    {
        self.value.get()
    }

    /// Sets the watched value
    pub fn set(&self, value: T, ctx: WatchArg<'_, 'ctx, O>) {
        self.meta.trigger(ctx);
        self.value.set(value);
    }

    pub fn set_external(&self, value: T) {
        self.meta.trigger_external();
        self.value.set(value);
    }

    /// Replaces the contained value and returns the previous value
    pub fn replace(&self, value: T, ctx: WatchArg<'_, 'ctx, O>) -> T {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        self.value.replace(value)
    }

    pub fn replace_external(&self, value: T) -> T {
        self.meta.trigger_external();
        self.value.replace(value)
    }

    /// Takes the watched value, leaving `Default::default()` in its place
    pub fn take(&self, ctx: WatchArg<'_, 'ctx, O>) -> T
    where
        T: Default,
    {
        self.meta.trigger(ctx);
        self.meta.watched(ctx);
        self.value.take()
    }

    pub fn take_external(&self) -> T
    where
        T: Default,
    {
        self.meta.trigger_external();
        self.value.take()
    }

    pub fn set_if_neq(&self, value: T, ctx: WatchArg<'_, 'ctx, O>)
    where
        T: Copy + PartialEq,
    {
        if self.value.get() != value {
            self.set(value, ctx);
        }
    }

    pub fn set_if_neq_external(&self, value: T)
    where
        T: Copy + PartialEq,
    {
        if self.value.get() != value {
            self.set_external(value);
        }
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized> WatchedCellCore<'static, T, DefaultOwner> {
    pub fn get_auto(&self) -> T
    where
        T: Sized + Copy,
    {
        self.meta.watched_auto();
        self.value.get()
    }

    pub fn get_mut_auto(&mut self) -> &mut T {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.get_mut()
    }

    pub fn watched_auto(&self) {
        self.meta.watched_auto();
    }

    pub fn set_auto(&self, value: T)
    where
        T: Sized,
    {
        self.meta.trigger_auto();
        self.value.set(value);
    }

    pub fn replace_auto(&self, value: T) -> T
    where
        T: Sized,
    {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.replace(value)
    }

    pub fn take_auto(&self) -> T
    where
        T: Default,
    {
        self.meta.trigger_auto();
        self.meta.watched_auto();
        self.value.take()
    }

    pub fn set_if_neq_auto(&self, value: T)
    where
        T: Copy + PartialEq,
    {
        if self.value.get() != value {
            self.set_auto(value);
        }
    }
}
