/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use crate::{DefaultOwner, WatchedCellCore, WatchedCore, WatchedValueCore};

/// This represents some value which will be interesting to watch. Watcher
/// functions that reference this value will be re-run when this value
/// changes.
#[derive(Default)]
pub struct Watched<T: ?Sized> {
    inner: WatchedCore<'static, T, DefaultOwner>,
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

    /// Takes the wrapped value, leaving `Default::default()` in its place.
    pub fn take(this: &mut Self) -> T
    where
        T: Default,
    {
        std::mem::take(&mut *this)
    }

    /// This function provides a way to set a value for a watched value
    /// only if is has changed.  This is useful for cases where setting a
    /// value would otherwise cause an infinite loop.
    ///
    /// # Examples
    /// The following example uses the watch system to keep two variables in
    /// sync. This would normally cause an infinite loop as each update of
    /// one would cause the other one to re-evaluate. However using set_if_neq
    /// allows it to detect that the value is the same and stop propogating.
    ///
    /// ```rust
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
    ///             Watched::set_if_neq(&mut root.left, *root.right);
    ///         });
    ///         init.watch(|root| {
    ///             Watched::set_if_neq(&mut root.right, *root.left);
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
    /// ctx.set_frame_limit(Some(10));
    /// ctx.add_watcher(&weak);
    /// *keep_balanced.borrow_mut().left = 3;
    /// ctx.update();
    /// assert_eq!(keep_balanced.borrow().right, 3);
    /// *keep_balanced.borrow_mut().right = 21;
    /// ctx.update();
    /// assert_eq!(keep_balanced.borrow().left, 21);
    /// ```
    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn set_if_neq(wrapper: &mut Watched<T>, value: T)
    where
        T: PartialEq,
    {
        wrapper.inner.set_if_neq_auto(value);
    }
}

impl<T: ?Sized> Watched<T> {
    /// Get a referenced to the wrapped value, without binding the current
    /// watch closure.
    pub fn get_unwatched(this: &Self) -> &T {
        this.inner.get_unwatched()
    }
}

impl<T: ?Sized> Deref for Watched<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.get_auto()
    }
}

impl<T: ?Sized> DerefMut for Watched<T> {
    #[cfg_attr(do_cycle_debug, track_caller)]
    fn deref_mut(&mut self) -> &mut T {
        self.inner.get_mut_auto()
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Watched<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner.get_auto(), f)
    }
}

impl<'a, T> crate::WatchedValueCore<'static, DefaultOwner> for &'a Watched<T>
where
    T: ?Sized,
{
    type Value = &'a T;

    fn get(
        self,
        ctx: crate::WatchArg<'_, 'static, DefaultOwner>,
    ) -> Self::Value {
        self.inner.get(ctx)
    }

    fn get_unwatched(self) -> Self::Value {
        self.inner.get_unwatched()
    }
}

mod watched_ops {
    use std::cmp::Ordering;
    use std::ops::*;

    use super::Watched;

    macro_rules! watched_unop {
        (impl $imp:ident, $method:ident) => {
            /*
            impl<T: $imp> $imp for Watched<T> {
                type Output = <T as $imp>::Output;

                fn $method(self) -> <T as $imp>::Output {
                    $imp::$method(self.inner.get_auto())
                }
            }
            */

            impl<'a, T: ?Sized> $imp for &'a Watched<T>
            where
                &'a T: $imp,
            {
                type Output = <&'a T as $imp>::Output;

                fn $method(self) -> <&'a T as $imp>::Output {
                    $imp::$method(self.inner.get_auto())
                }
            }
        };
    }

    macro_rules! watched_binop {
        (impl $imp:ident, $method:ident) => {
            /*
            impl<T, U> $imp<U> for Watched<T>
            where
                T: $imp<U>,
            {
                type Output = <T as $imp<U>>::Output;

                fn $method(self, other: U) -> <T as $imp<U>>::Output {
                    $imp::$method(self.inner.get_auto(), other)
                }
            }
            */

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
                #[cfg_attr(do_cycle_debug, track_caller)]
                fn $method(&mut self, rhs: U) {
                    $imp::$method(self.inner.get_mut_auto(), rhs);
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
/// would not, with lower overhead than `RefCell<Watched<T>>`.
#[derive(Default)]
pub struct WatchedCell<T: ?Sized> {
    inner: WatchedCellCore<'static, T, DefaultOwner>,
}

impl<T: ?Sized> WatchedCell<T> {
    /// Returns a mutable reference to the watched data.
    ///
    /// This call borrows the WatchedCell mutably (at compile-time) which
    /// guarantees that we possess the only reference.
    #[cfg_attr(do_cycle_debug, track_caller)]
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
    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn set(&self, value: T) {
        self.inner.set_auto(value);
    }

    /// Unwraps the WatchedCell, returning the contained value
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Replaces the contained value and returns the previous value
    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn replace(&self, value: T) -> T {
        self.inner.replace_auto(value)
    }

    /// Returns a copy of the watched value
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        self.inner.get_auto()
    }

    /// Takes the watched value, leaving `Default::default()` in its place
    #[cfg_attr(do_cycle_debug, track_caller)]
    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.inner.take_auto()
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

impl<T> WatchedValueCore<'static, DefaultOwner> for &WatchedCell<T>
where
    T: Copy,
{
    type Value = T;

    fn get(
        self,
        ctx: crate::WatchArg<'_, 'static, DefaultOwner>,
    ) -> Self::Value {
        self.inner.get(ctx)
    }

    fn get_unwatched(self) -> Self::Value {
        self.inner.get_unwatched()
    }
}

pub trait WatchedValue:
    crate::WatchedValueCore<'static, DefaultOwner>
{
    fn get_auto(self) -> Self::Value;
}

impl<W> WatchedValue for W
where
    W: crate::WatchedValueCore<'static, DefaultOwner>,
{
    fn get_auto(self) -> Self::Value {
        let mut this = Some(self);
        let mut result = None;
        crate::trigger::WatchArg::try_with_current(|ctx| {
            result = Some(this.take().unwrap().get(ctx));
        });
        result.unwrap_or_else(|| this.unwrap().get_unwatched())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::*;

    #[test]
    fn watched_add() {
        let left = Watched::new(587);
        assert_eq!(&left + 13, 600);
    }

    #[test]
    fn add_to_watched() {
        struct Content {
            dest: u32,
            source: Watched<u32>,
        }

        impl Watcher<'static> for Content {
            fn init(mut init: impl WatcherInit<'static, Self>) {
                init.watch(|root| {
                    root.dest = *root.source;
                });
            }
        }
        let content = Rc::new(RefCell::new(Content {
            dest: 0,
            source: Watched::new(587),
        }));
        let weak = Rc::downgrade(&content);
        let mut ctx = WatchContext::new();
        ctx.add_watcher(&weak);
        assert_eq!(content.borrow().dest, 587);
        content.borrow_mut().source += 13;
        ctx.update();
        assert_eq!(content.borrow().dest, 600);
    }

    #[test]
    fn watched_xor() {
        #[derive(Default)]
        struct Content {
            dest: u32,
            source: Watched<u32>,
        }

        impl Watcher<'static> for Content {
            fn init(mut init: impl WatcherInit<'static, Self>) {
                init.watch(|root| {
                    root.dest = &root.source ^ 0xffffffff;
                });
            }
        }
        let content = Rc::new(RefCell::new(Content::default()));
        let weak = Rc::downgrade(&content);
        let mut ctx = WatchContext::new();
        ctx.add_watcher(&weak);
        *content.borrow_mut().source = 960294194;
        ctx.update();
        assert_eq!(content.borrow().dest, 3334673101);
    }

    #[test]
    fn watched_reasonably_sized() {
        assert_eq!(
            std::mem::size_of::<Watched<usize>>(),
            2 * std::mem::size_of::<usize>(),
        );
    }
}
