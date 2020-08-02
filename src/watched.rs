/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

use super::{WatchContext, WatchSet};

/// This provides the basic functionality behind watched values. You can use
/// it to provide functionality using the watch system for cases where
/// [Watched](struct.Watched.html) and
/// [WatchedEvent](struct.WatchedEvent.html) are not appropriate.
#[derive(Default)]
pub struct WatchedMeta {
    watchers: RefCell<WatchSet>,
}

impl WatchedMeta {
    /// Create a new WatchedMeta instance
    pub fn new() -> Self {
        WatchedMeta { watchers: RefCell::new(WatchSet::new()) }
    }

    /// When run in a function designed to watch a value, will bind so that
    /// function will be re-run when this is triggered.
    pub fn watched(&self) {
        WatchContext::try_get_current(|ctx| {
            if let Some(watch) = ctx.current_watch() {
                self.watchers.borrow_mut().add(watch);
            }
        });
    }

    /// Mark this value as having changed, so that watching functions will
    /// be marked as needing to be updated.
    /// # Panics
    /// This function will panic if called outside of WatchContext::with
    pub fn trigger(&mut self) {
        WatchContext::expect_current(|ctx| {
            ctx.add_to_next(&mut self.watchers.borrow_mut());
        }, "WatchedMeta.trigger() called outside of WatchContext");
    }
}

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
pub struct Watched<T> {
    value: T,
    meta: WatchedMeta,
}

impl<T> Watched<T> {
    /// Create a new watched value.
    pub fn new(value: T) -> Self {
        Watched { value, meta: WatchedMeta::new() }
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

impl<T> Deref for Watched<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.meta.watched();
        &self.value
    }
}


impl<T> DerefMut for Watched<T> {
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

impl<T: fmt::Debug> fmt::Debug for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display> fmt::Display for Watched<T> {
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
impl<T: serde::Serialize> serde::Serialize for Watched<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        T::serialize(&self.value, serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Watched<T> {
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

            impl<'a, T> $imp for &'a Watched<T>
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
                    &'a T: $imp<U>
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
                    T: $imp<U>
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
            T: PartialEq<U>
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
            T: PartialOrd<U>
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
}
