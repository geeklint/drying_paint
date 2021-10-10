/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use core::fmt;
use core::ops::{Deref, DerefMut};

use crate::{context::Ctx, WatchedCellCore, WatchedCore};

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
#[derive(Clone, Default)]
pub struct Watched<T: ?Sized> {
    inner: WatchedCore<T, Ctx<'static>>,
}

impl<T> Watched<T> {
    /// Create a new watched value.
    pub fn new(value: T) -> Self {
        Watched {
            inner: WatchedCore::new(value),
        }
    }

    /// Consumes the `Watched`, returning the wrapped value
    pub fn into_inner(this: Self) -> T {
        this.inner.into_inner()
    }

    /// Replaces the wrapped value with a new one, returning the old value,
    /// without deinitializing either one.
    pub fn replace(this: &mut Self, value: T) -> T {
        std::mem::replace(&mut *this, value)
    }
}

impl<T: ?Sized> Watched<T> {
    /// Get a referenced to the wrapped value, without binding the current
    /// watch closure.
    pub fn get_unwatched(this: &Self) -> &T {
        &this.get_unwatched()
    }
}

impl<T: Default> Watched<T> {
    /// Takes the wrapped value, leaving `Default::default()` in its place.
    pub fn take(this: &mut Self) -> T {
        std::mem::take(&mut *this)
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
        self.inner.get_auto()
    }
}

impl<T: ?Sized> DerefMut for Watched<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.get_mut_auto()
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner.get_auto(), f)
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize + ?Sized> serde::Serialize for Watched<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
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
        D: serde::Deserializer<'de>,
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
                    $imp::$method(self.inner.get_auto())
                }
            }

            impl<'a, T: ?Sized> $imp for &'a Watched<T>
            where
                &'a T: $imp,
            {
                type Output = <&'a T as $imp>::Output;

                fn $method(self) -> <&'a T as $imp>::Output {
                    $imp::$method(&self.inner.get_auto())
                }
            }
        };
    }

    macro_rules! watched_binop {
        (impl $imp:ident, $method:ident) => {
            impl<T, U> $imp<U> for Watched<T>
            where
                T: $imp<U>,
            {
                type Output = <T as $imp<U>>::Output;

                fn $method(self, other: U) -> <T as $imp<U>>::Output {
                    $imp::$method(self.inner.get_auto(), other)
                }
            }

            impl<'a, T, U> $imp<U> for &'a Watched<T>
            where
                T: ?Sized,
                &'a T: $imp<U>,
            {
                type Output = <&'a T as $imp<U>>::Output;

                fn $method(self, other: U) -> <&'a T as $imp<U>>::Output {
                    $imp::$method(self.inner.get_auto(), other)
                }
            }
        };
    }

    macro_rules! watched_binop_assign {
        (impl $imp:ident, $method:ident) => {
            impl<T, U> $imp<U> for Watched<T>
            where
                T: $imp<U> + ?Sized,
            {
                fn $method(&mut self, rhs: U) {
                    $imp::$method(self.inner.get_mut_auto(), rhs);
                    self.meta.trigger();
                    self.meta.watched();
                }
            }
        };
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
            PartialEq::eq(self.inner.get_auto(), other)
        }

        #[allow(clippy::partialeq_ne_impl)]
        fn ne(&self, other: &U) -> bool {
            PartialEq::ne(self.inner.get_auto(), other)
        }
    }

    impl<T, U> PartialOrd<U> for Watched<T>
    where
        T: PartialOrd<U> + ?Sized,
        U: ?Sized,
    {
        fn partial_cmp(&self, other: &U) -> Option<Ordering> {
            PartialOrd::partial_cmp(self.inner.get_auto(), other)
        }
        fn lt(&self, other: &U) -> bool {
            PartialOrd::lt(self.inner.get_auto(), other)
        }
        fn le(&self, other: &U) -> bool {
            PartialOrd::le(self.inner.get_auto(), other)
        }
        fn ge(&self, other: &U) -> bool {
            PartialOrd::ge(self.inner.get_auto(), other)
        }
        fn gt(&self, other: &U) -> bool {
            PartialOrd::gt(self.inner.get_auto(), other)
        }
    }

    /*
    impl<T: Ord> Ord for Watched<T> {
        fn cmp(&self, other: &Watched<T>) -> Ordering {
            Ord::cmp(self.inner.get_auto(), other.inner.get_auto())
        }
    }

    impl<T: Eq> Eq for Watched<T> {}
    */
}

/// A Watched value which provides interior mutability.  This provides correct
/// behavior (triggering watch functions when changed) where `Watched<Cell<T>>`
/// would not, and should be slightly more performant than
/// `RefCell<Watched<T>>`.
#[derive(Default)]
pub struct WatchedCell<T: ?Sized> {
    inner: WatchedCellCore<T, Ctx<'static>>,
}

impl<T: ?Sized> WatchedCell<T> {
    /// Returns a mutable reference to the watched data.
    ///
    /// This call borrows the WatchedCell mutably (at compile-time) which
    /// guarantees that we possess the only reference.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut_auto()
    }

    /// Treat this WatchedCell as watched, without fetching the actual value.
    pub fn watched(&self) {
        self.inner.watched_auto();
    }
}

impl<T> WatchedCell<T> {
    /// Create a new WatchedCell
    pub fn new(value: T) -> Self {
        Self {
            inner: WatchedCellCore::new(value),
        }
    }

    /// Sets the watched value
    pub fn set(&self, value: T) {
        self.inner.set_auto(value);
    }

    /// Unwraps the WatchedCell, returning the contained value
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Replaces the contained value and returns the previous value
    pub fn replace(&self, value: T) -> T {
        self.inner.replace_auto(value)
    }
}

impl<T: Copy> WatchedCell<T> {
    /// Returns a copy of the watched value
    pub fn get(&self) -> T {
        self.inner.get_auto()
    }
}

impl<T: Default> WatchedCell<T> {
    /// Takes the watched value, leaving `Default::default()` in its place
    pub fn take(&self) -> T {
        self.value.take_auto()
    }
}

impl<T: Copy> Clone for WatchedCell<T> {
    fn clone(&self) -> Self {
        Self::new(self.get())
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
        ctx = ctx
            .with(|| {
                let outer = WatchContext::allow_watcher_access((), |()| {
                    let mut outer = Outer::new();
                    *outer.data_mut().inner.value = 587;
                    outer
                });
                WatchContext::update_current();
                let outer =
                    WatchContext::allow_watcher_access(outer, |mut outer| {
                        assert_eq!(outer.data().value, 587);
                        outer.data_mut().inner.value += 13;
                        outer
                    });
                WatchContext::update_current();
                WatchContext::allow_watcher_access(outer, |outer| {
                    assert_eq!(outer.data().value, 600);
                });
            })
            .0;
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
        ctx = ctx
            .with(|| {
                let outer = WatchContext::allow_watcher_access((), |()| {
                    let mut outer = Watcher::<OuterXorData>::new();
                    *outer.data_mut().inner.value = 960294194;
                    outer
                });
                WatchContext::update_current();
                WatchContext::allow_watcher_access(outer, |outer| {
                    assert_eq!(outer.data().value, 3334673101);
                });
            })
            .0;
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
