/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! This module exposes a smart pointer type with specific usage patterns.
//! It allows a subset of the sharing that std::rc::Rc does: There is one
//! primary owner of any data (OwnedPointer), but there may be many Weak-style
//! runtime borrows (BorrowedPointer).
//!
//! BorrowedPointers have exactly one path to obtaining a reference to their
//! data: BorrowedPointer::upgrade passes a reference to the contained data
//! to the closure it receives.
//!
//! In order to observe Rust's aliasing rules, the following invarients are
//! upheld:
//!
//! - Only one BorrowedPointer may be upgraded at a time.
//!
//! - Attempting to access an OwnedPointer while its data are currently
//! borrowed via BorrowedPointer::upgrade will panic.
//!
//! - Attempting to access an OwnedPointer while nothing is currently borrowed
//! via BorrowedPointer::upgrade (which would allow a future upgrade) will
//! also panic.
//!
//! In order to work around that last point, BorrowedPointer::allow_refs is
//! provided.  This prevents upgrades without borrowing anything specifically.
//!
//! The closures and userdata provided to BorrowedPointer::upgrade and
//! BorrowedPointer::allow_refs are 'static, which prevents references
//! borrowed from OwnedPointers from escaping their scope.

use std::rc::{
    Rc,
    Weak,
};
use std::cell::{
    Cell,
    UnsafeCell,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum BorrowState {
    NothingBorrowed,
    BorrowsBlocked,
    Borrowed(*const ()),
}

thread_local! {
    static BORROW_STATE: Cell<BorrowState>
        = Cell::new(BorrowState::NothingBorrowed);
}

struct BorrowGuard {
    _marker: std::marker::PhantomData<Rc<()>>,
}

impl Drop for BorrowGuard {
    fn drop(&mut self) {
        BORROW_STATE.with(|cell| {
            cell.set(BorrowState::NothingBorrowed);
        });
    }
}

impl BorrowGuard {
    fn new(ptr: *const ()) -> Self {
        BORROW_STATE.with(|cell| {
            assert_eq!(
                cell.get(),
                BorrowState::NothingBorrowed,
                "Attempt to create BorrowGuard::new when a BorrowGuard is already in use"
            );
            cell.set(BorrowState::Borrowed(ptr));
            Self { _marker: std::marker::PhantomData }
        })
    }

    pub fn block() -> Self {
        BORROW_STATE.with(|cell| {
            assert_eq!(
                cell.get(),
                BorrowState::NothingBorrowed,
                "Attempt to create BorrowGuard::block when a BorrowGuard is already in use"
            );
            cell.set(BorrowState::BorrowsBlocked);
            Self { _marker: std::marker::PhantomData }
        })
    }

    pub fn assert_owned_borrows_allowed(incoming: *const ()) {
        BORROW_STATE.with(|cell| {
            match cell.get() {
                BorrowState::BorrowsBlocked => (),
                BorrowState::NothingBorrowed => panic!(
                    "Owned borrows are not allowed outside BorrowedPointer::upgrade or BorrowedPointer::allow_refs"
                ),
                BorrowState::Borrowed(current) => {
                    if current == incoming {
                        panic!(
                            "OwnedPointer {:p} is already borrowed as a BorrowedPointer",
                            incoming,
                        )
                    }
                },
            }
        })
    }
}

#[derive(Default)]
pub(crate) struct OwnedPointer<T: ?Sized> {
    ptr: Rc<UnsafeCell<T>>,
}

impl<T: ?Sized> OwnedPointer<T> {
    pub fn as_ref(&self) -> &T {
        let raw = Rc::as_ptr(&self.ptr);
        BorrowGuard::assert_owned_borrows_allowed(raw as *const ());
        unsafe {
            &*self.ptr.get()
        }
    }

    pub fn as_mut(&mut self) -> &mut T {
        let raw = Rc::as_ptr(&self.ptr);
        BorrowGuard::assert_owned_borrows_allowed(raw as *const ());
        unsafe {
            &mut *self.ptr.get()
        }
    }

    pub fn new_borrowed(&self) -> BorrowedPointer<T> {
        BorrowedPointer {
            ptr: Rc::downgrade(&self.ptr),
        }
    }
}

impl<T> OwnedPointer<T> {
    pub fn new(data: T) -> Self {
        Self { ptr: Rc::new(UnsafeCell::new(data)) }
    }

    pub fn into_inner(self) -> T {
        if let Ok(cell) = Rc::try_unwrap(self.ptr) {
            cell.into_inner()
        } else {
            panic!("OwnedPointer::into_inner called while there were other refs")
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct BorrowedPointer<T: ?Sized> {
    ptr: Weak<UnsafeCell<T>>,
}

impl BorrowedPointer<()> {
    pub fn allow_refs<F, U>(mut data: U, func: F) -> U
    where
        F: FnOnce(&mut U) + 'static,
        U: 'static,
    {
        let guard = BorrowGuard::block();
        func(&mut data);
        std::mem::drop(guard);
        data
    }
}

impl<T: ?Sized> BorrowedPointer<T> {
    pub fn upgrade<F, U>(&mut self, mut data: U, func: F) -> U
    where
        F: FnOnce(&mut U, &mut T) + 'static,
        U: 'static,
    {
        if let Some(ptr) = self.ptr.upgrade() {
            let raw = Rc::as_ptr(&ptr).cast();
            let guard = BorrowGuard::new(raw);
            let value_ref = unsafe { &mut *ptr.get() };
            func(&mut data, value_ref);
            std::mem::drop(guard);
        }
        data
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.ptr.ptr_eq(&other.ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_allow_refs_allows_refs() {
        let ptr = OwnedPointer::<Option<u32>>::default();
        BorrowedPointer::allow_refs(ptr, |ptr| {
            assert!(Option::as_ref(ptr.as_ref()).is_none());
        });
    }

    #[test]
    fn pointer_allow_refs_allows_muts() {
        let ptr = OwnedPointer::<Option<u32>>::default();
        let ptr = BorrowedPointer::allow_refs(ptr, |ptr| {
            *ptr.as_mut() = Some(77);
        });
        assert_eq!(ptr.into_inner(), Some(77));
    }

    #[test]
    #[should_panic(expected = "Owned borrows are not allowed outside")]
    fn pointer_refs_outside_allow_refs_denied() {
        let ptr = OwnedPointer::<Option<u32>>::default();
        assert!(Option::as_ref(ptr.as_ref()).is_none());
    }

    #[test]
    #[should_panic(expected = "Owned borrows are not allowed outside")]
    fn pointer_muts_outside_allow_refs_denied() {
        let mut ptr = OwnedPointer::<Option<u32>>::default();
        *ptr.as_mut() = Some(879);
        println!("{:?}", ptr.into_inner());
    }

    #[test]
    fn pointer_upgrade_allows_different_muts() {
        let ptr0 = OwnedPointer::<Option<u32>>::default();
        let ptr1 = OwnedPointer::<Option<u32>>::default();
        let mut brw0 = ptr0.new_borrowed();
        let ptr1 = brw0.upgrade(ptr1, |ptr1, up0| {
            *up0 = Some(792);
            *ptr1.as_mut() = Some(446);
        });
        assert_eq!(ptr0.into_inner(), Some(792));
        assert_eq!(ptr1.into_inner(), Some(446));
    }

    #[test]
    #[should_panic(expected = "is already borrowed as a BorrowedPointer")]
    fn pointer_upgrade_prevents_same_muts() {
        let ptr0 = OwnedPointer::<Option<u32>>::default();
        let mut brw0 = ptr0.new_borrowed();
        let ptr0 = brw0.upgrade(ptr0, |ptr0, up0| {
            *ptr0.as_mut() = Some(446);
            *up0 = Some(792);
        });
        println!("{:?}", ptr0.into_inner());
    }

    #[test]
    #[should_panic(expected = "BorrowGuard is already in use")]
    fn pointer_cannot_upgrade_inside_upgrade() {
        let ptr0 = OwnedPointer::<Option<u32>>::default();
        let ptr1 = OwnedPointer::<Option<u32>>::default();
        let mut brw0 = ptr0.new_borrowed();
        let brw1 = ptr1.new_borrowed();
        brw0.upgrade(brw1, |brw1, up0| {
            brw1.upgrade((), |(), up1| {
                *up1 = Some(598);
            });
            *up0 = Some(598);
        });
        println!("{:?}", ptr0.into_inner());
        println!("{:?}", ptr1.into_inner());
    }

    #[test]
    #[should_panic(expected = "BorrowGuard is already in use")]
    fn pointer_cannot_upgrade_inside_allow_refs() {
        let ptr0 = OwnedPointer::<Option<u32>>::default();
        let brw0 = ptr0.new_borrowed();
        BorrowedPointer::allow_refs(brw0, |brw0| {
            brw0.upgrade((), |(), up0| {
                *up0 = Some(930);
            });
        });
        println!("{:?}", ptr0.into_inner());
    }
}
