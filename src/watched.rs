/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{
    RefCell,
};
use std::ops::{Deref, DerefMut};

use super::{WatchContext, WatchSet};

/// This provides the basic functionality behind watched values. You can use
/// it to provide functionality using the watch system for cases where
/// [Watched](struct.Watched.html) and
/// [WatchedEvent](struct.WatchedEvent.html) are not appropriate.
#[derive(Default)]
pub struct WatchedMeta {
    watchers: WatchSet,
}

impl WatchedMeta {
    /// Create a new WatchedMeta instance
    pub fn new() -> Self {
        WatchedMeta { watchers: WatchSet::new() }
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when this is triggered.
    pub fn watched(&self) {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.watchers.add(watch);
            }
        });
    }

    /// Mark this value as having changed, so that watching functions will
    /// be marked as needing to be updated.
    /// # Panics
    /// This function will panic if called outside of WatchContext::with
    pub fn trigger(&mut self) {
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut self.watchers);
        }, "WatchedMeta.trigger() called outside of WatchContext");
    }
}

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
pub struct Watched<T: ?Sized> {
    meta: WatchedMeta,
    value: T,
}

impl<T> Watched<T> {
    /// Create a new watched value.
    pub fn new(value: T) -> Self {
        Watched { value, meta: WatchedMeta::new() }
    }

    /// Consumes the `Watched`, returning the wrapped value
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Replaces the wrapped value with a new one, returning the old value,
    /// without deinitializing either one.
    pub fn replace(&mut self, value: T) -> T {
        std::mem::replace(&mut *self, value)
    }
}

impl<T: Default> Watched<T> {
    /// Takes the wrapped value, leaving `Default::default()` in its place.
    pub fn take(&mut self) -> T {
        std::mem::take(&mut *self)
    }
}

impl<T: PartialEq> Watched<T> {
    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    ///
    /// # Examples
    /// The following example uses the watch system to keep two variables in
    /// sync. This would normally cause an infinite loop as each update of
    /// one would cause the other one to re-evaluate. However using set_if_neq
    /// allows it to detect that the value is the same and stop propogating.
    /// ```rust
    /// # use drying_paint::*;
    /// #[derive(Default)]
    /// struct KeepBalanced {
    ///     left: Watched<i32>,
    ///     right: Watched<i32>,
    /// }
    /// impl WatcherInit for KeepBalanced {
    ///     fn init(watcher: &mut WatcherMeta<Self>) {
    ///         watcher.watch(|root| {
    ///             Watched::set_if_neq(&mut root.left, *root.right);
    ///         });
    ///         watcher.watch(|root| {
    ///             Watched::set_if_neq(&mut root.right, *root.left);
    ///         });
    ///     }
    /// }
    /// fn main() {
    ///     let mut ctx = WatchContext::new();
    ///     ctx.set_frame_limit(Some(100));
    ///     ctx.with(|| {
    ///         let obj = WatchContext::allow_watcher_access((), |()| {
    ///             let mut obj = Watcher::<KeepBalanced>::new();
    ///             *obj.data_mut().left = 68;
    ///             obj
    ///         });
    ///         WatchContext::update_current();
    ///         WatchContext::allow_watcher_access(obj, |obj| {
    ///             assert_eq!(*obj.data().right, 68);
    ///         });
    ///     });
    /// }
    pub fn set_if_neq(wrapper: &mut Watched<T>, value: T) {
        if wrapper.value != value {
            wrapper.value = value;
            wrapper.meta.trigger();
            wrapper.meta.watched();
        }
    }
}

impl<T: ?Sized> Deref for Watched<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.meta.watched();
        &self.value
    }
}


impl<T: ?Sized> DerefMut for Watched<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.meta.trigger();
        self.meta.watched();
        &mut self.value
    }
}

impl<T: Default> Default for Watched<T> {
    fn default() -> Self {
        Watched::new(Default::default())
    }
}

use std::fmt;

impl<T: fmt::Debug + ?Sized> fmt::Debug for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: Clone> Clone for Watched<T> {
    fn clone(&self) -> Watched<T> {
        Watched {
            value: Clone::clone(&self.value),
            meta: WatchedMeta::new(),
        }
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize + ?Sized> serde::Serialize for Watched<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        T::serialize(&self.value, serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Watched<T>
where
    T: serde::Deserialize<'de> + ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        T::deserialize(deserializer).map(Self::new)
    }
}

mod watched_ops {
    use core::cmp::Ordering;
    use core::ops::*;

    use super::Watched;

    macro_rules! watched_unop {
        (impl $imp:ident, $method:ident) => {
            impl<T: $imp> $imp for Watched<T> {
                type Output = <T as $imp>::Output;

                fn $method(self) -> <T as $imp>::Output {
                    self.meta.watched();
                    $imp::$method(self.value)
                }
            }

            impl<'a, T: ?Sized> $imp for &'a Watched<T>
                where
                    &'a T: $imp
            {
                type Output = <&'a T as $imp>::Output;

                fn $method(self) -> <&'a T as $imp>::Output {
                    self.meta.watched();
                    $imp::$method(&self.value)
                }
            }
        }
    }

    macro_rules! watched_binop {
        (impl $imp:ident, $method:ident) => {
            impl<T, U> $imp<U> for Watched<T>
                where
                    T: $imp<U>
            {
                type Output = <T as $imp<U>>::Output;

                fn $method(self, other: U) -> <T as $imp<U>>::Output {
                    self.meta.watched();
                    $imp::$method(self.value, other)
                }
            }

            impl<'a, T, U> $imp<U> for &'a Watched<T>
                where
                    T: ?Sized,
                    &'a T: $imp<U>,
            {
                type Output = <&'a T as $imp<U>>::Output;

                fn $method(self, other: U) -> <&'a T as $imp<U>>::Output {
                    self.meta.watched();
                    $imp::$method(&self.value, other)
                }
            }
        }
    }

    macro_rules! watched_binop_assign {
        (impl $imp:ident, $method:ident) => {
            impl<T, U> $imp<U> for Watched<T>
                where
                    T: $imp<U> + ?Sized
            {
                fn $method(&mut self, rhs: U) {
                    let res = $imp::$method(&mut self.value, rhs);
                    self.meta.trigger();
                    self.meta.watched();
                    res
                }
            }
        }
    }

    watched_unop!(impl Neg, neg);
    watched_unop!(impl Not, not);

    watched_binop!(impl Add, add);
    watched_binop!(impl BitAnd, bitand);
    watched_binop!(impl BitOr, bitor);
    watched_binop!(impl BitXor, bitxor);
    watched_binop!(impl Div, div);
    watched_binop!(impl Mul, mul);
    watched_binop!(impl Rem, rem);
    watched_binop!(impl Shl, shl);
    watched_binop!(impl Shr, shr);
    watched_binop!(impl Sub, sub);

    watched_binop_assign!(impl AddAssign, add_assign);
    watched_binop_assign!(impl BitAndAssign, bitand_assign);
    watched_binop_assign!(impl BitOrAssign, bitor_assign);
    watched_binop_assign!(impl BitXorAssign, bitxor_assign);
    watched_binop_assign!(impl DivAssign, div_assign);
    watched_binop_assign!(impl MulAssign, mul_assign);
    watched_binop_assign!(impl RemAssign, rem_assign);
    watched_binop_assign!(impl ShlAssign, shl_assign);
    watched_binop_assign!(impl ShrAssign, shr_assign);
    watched_binop_assign!(impl SubAssign, sub_assign);

    impl<T, U> PartialEq<U> for Watched<T>
        where
            T: PartialEq<U> + ?Sized,
            U: ?Sized,
    {
        fn eq(&self, other: &U) -> bool {
            self.meta.watched();
            PartialEq::eq(&self.value, other)
        }

        #[allow(clippy::partialeq_ne_impl)]
        fn ne(&self, other: &U) -> bool {
            self.meta.watched();
            PartialEq::ne(&self.value, other)
        }
    }

    impl<T, U> PartialOrd<U> for Watched<T>
        where
            T: PartialOrd<U> + ?Sized,
            U: ?Sized,
    {
        fn partial_cmp(&self, other: &U) -> Option<Ordering> {
            self.meta.watched();
            PartialOrd::partial_cmp(&self.value, other)
        }
        fn lt(&self, other: &U) -> bool {
            self.meta.watched();
            PartialOrd::lt(&self.value, other)
        }
        fn le(&self, other: &U) -> bool {
            self.meta.watched();
            PartialOrd::le(&self.value, other)
        }
        fn ge(&self, other: &U) -> bool {
            self.meta.watched();
            PartialOrd::ge(&self.value, other)
        }
        fn gt(&self, other: &U) -> bool {
            self.meta.watched();
            PartialOrd::gt(&self.value, other)
        }
    }

    /*
    impl<T: Ord> Ord for Watched<T> {
        fn cmp(&self, other: &Watched<T>) -> Ordering {
            self.meta.watched();
            other.meta.watched();
            Ord::cmp(&self.value, &other.value)
        }
    }

    impl<T: Eq> Eq for Watched<T> {}
    */
}

/// A Watched value which provides interior mutability.  This provides correct
/// behavior (triggering watch functions when changed) where `Watched<Cell<T>>`
/// would not, and should be slightly more performant than
/// `RefCell<Watched<T>>`.
#[derive(Default, )]
pub struct WatchedCell<T: ?Sized> {
    inner: RefCell<(WatchSet, T)>,
}

impl<T: ?Sized> WatchedCell<T> {
    /// Returns a mutable reference to the watched data.
    ///
    /// This call borrows the WatchedCell mutably (at compile-time) which
    /// guarantees that we possess the only reference.
    pub fn get_mut(&mut self) -> &mut T {
        let borrow = self.inner.get_mut();
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut borrow.0);
        }, "WatchedCell.get_mut() called outside of WatchContext");
        &mut borrow.1
    }

    /// Treat this WatchedCell as watched, without fetching the actual value.
    pub fn watched(&self) {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.inner.borrow_mut().0.add(watch);
            }
        });
    }

    fn update<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut borrow = self.inner.borrow_mut();
        let ret = f(&mut borrow.1);
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut borrow.0);
        }, "WatchedCell.update() called outside of WatchContext");
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                borrow.0.add(watch);
            }
        });
        ret
    }
}

impl<T> WatchedCell<T> {
    /// Create a new WatchedCell
    pub fn new(value: T) -> Self {
        Self {
            inner: RefCell::new((WatchSet::new(), value)),
        }
    }

    /// Sets the watched value
    pub fn set(&self, value: T) {
        self.replace(value);
    }

    /// Unwraps the WatchedCell, returning the contained value
    pub fn into_inner(self) -> T {
        self.inner.into_inner().1
    }

    /// Replaces the contained value and returns the previous value
    pub fn replace(&self, value: T) -> T {
        self.update(|current| std::mem::replace(current, value))
    }
}

impl<T: Copy> WatchedCell<T> {
    /// Returns a copy of the watched value
    pub fn get(&self) -> T {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.inner.borrow_mut().0.add(watch);
            }
        });
        self.inner.borrow().1
    }
}

impl<T: Default> WatchedCell<T> {
    /// Takes the watched value, leaving `Default::default()` in its place
    pub fn take(&self) -> T {
        self.update(|current| std::mem::take(current))
    }
}

impl<T: Copy> Clone for WatchedCell<T> {
    fn clone(&self) -> Self {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.inner.borrow_mut().0.add(watch);
            }
        });
        let clone = self.inner.borrow().1;
        Self::new(clone)
    }
}

impl<T> From<T> for WatchedCell<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_watched_add() {
        let left = Watched::new(587);
        assert_eq!((&left) + 13, 600);
        assert_eq!(left + 13, 600);
    }

    #[derive(Default)]
    struct Inner {
        value: Watched<u32>,
    }

    type Outer = Watcher<OuterData>;

    #[derive(Default)]
    struct OuterData {
        value: u32,
        inner: Inner,
    }

    impl WatcherInit for OuterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            watcher.watch(|root| {
                root.value = *root.inner.value;
            });
        }
    }

    #[test]
    fn test_add_to_watched() {
        let mut ctx = WatchContext::new();
        ctx = ctx.with(|| {
            let outer = WatchContext::allow_watcher_access((), |()| {
                let mut outer = Outer::new();
                *outer.data_mut().inner.value = 587;
                outer
            });
            WatchContext::update_current();
            let outer = WatchContext::allow_watcher_access(outer, |mut outer| {
                assert_eq!(outer.data().value, 587);
                outer.data_mut().inner.value += 13;
                outer
            });
            WatchContext::update_current();
            WatchContext::allow_watcher_access(outer, |outer| {
                assert_eq!(outer.data().value, 600);
            });
        }).0;
        std::mem::drop(ctx);
    }

    #[derive(Default)]
    struct OuterXorData {
        value: u32,
        inner: Inner,
    }

    impl WatcherInit for OuterXorData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            watcher.watch(|root| {
                root.value = &root.inner.value ^ 0xffffffff;
            });
        }
    }

    #[test]
    fn test_xor_watch() {
        let mut ctx = WatchContext::new();
        ctx = ctx.with(|| {
            let outer = WatchContext::allow_watcher_access((), |()| {
                let mut outer = Watcher::<OuterXorData>::new();
                *outer.data_mut().inner.value = 960294194;
                outer
            });
            WatchContext::update_current();
            WatchContext::allow_watcher_access(outer, |outer| {
                assert_eq!(outer.data().value, 3334673101);
            });
        }).0;
        std::mem::drop(ctx);
    }

    #[test]
    fn watched_reasonably_sized() {
        assert_eq!(
            std::mem::size_of::<Watched<usize>>(),
            2 * std::mem::size_of::<usize>(),
        );
    }
}
