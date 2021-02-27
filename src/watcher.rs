/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::Watch;
use crate::pointer::{BorrowedPointer, OwnedPointer};

struct WatcherMetaBase<T: ?Sized> {
    data: BorrowedPointer<T>,
    watches: Vec<Watch>,
}

/// This structure is used internally by Watcher<T>. It is passed to the init
/// function of WatcherInit, the trait which is required to be implemented by
/// the data stored in Watchers.
pub struct WatcherMeta<'a, T: ?Sized> {
    base: WatcherMetaBase<T>,
    debug_name: &'static str,
    key_data: &'a mut T,
}

impl<T: 'static> WatcherMeta<'_, T> {
    /// Get a value representing a unique id for the watcher this
    /// WatcherMeta was created for. This value may outlive the watcher, and
    /// will never compare equal to a value returned by the id method of a
    /// Watcher other than this one.
    pub fn id(&self) -> WatcherId {
        WatcherId {
            ptr: self.base.data.clone().into_any(),
        }
    }
}

impl<T: ?Sized + 'static> WatcherMeta<'_, T> {
    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    pub fn watch<F>(&mut self, func: F)
    where
        F: Fn(&mut T) + 'static,
    {
        let data = self.base.data.clone();
        let watch = Watch::new(self.key_data, data, func, self.debug_name);
        self.base.watches.push(watch);
    }

    /// Watches have a debug name used in some error messages.  It defaults to
    /// the type name of the associated content (T).  This function allows
    /// overriding that name.
    pub fn set_debug_name(&mut self, debug_name: &'static str) {
        self.debug_name = debug_name;
    }
}

/// This trait is required to be implemented by the data stored in Watchers.
/// It provides a convient point to register watching functions.
pub trait WatcherInit {
    /// Implementing this method is a convient place to setup watching
    /// functions.
    fn init(watcher: &mut WatcherMeta<Self>);
}

/// Watcher is a structure designed to hold some data along with associated
/// functions which will run when watched data changes.
pub struct Watcher<T: ?Sized> {
    data: OwnedPointer<T>,
    _meta: WatcherMetaBase<T>,
}

impl<T: WatcherInit> Watcher<T> {
    /// Create a new Watcher. After creation, will run WatcherInit::init for
    /// the stored data.  Watchers need to be created inside
    /// WatchContext::allow_watcher_access() in order to observe aliasing
    /// rules.
    pub fn create(data: T) -> Self {
        let mut data = OwnedPointer::new(data);
        let meta_base = WatcherMetaBase {
            data: data.new_borrowed(),
            watches: Vec::new(),
        };
        let meta = {
            let mut meta = WatcherMeta {
                base: meta_base,
                debug_name: std::any::type_name::<T>(),
                key_data: &mut data.as_mut(),
            };
            WatcherInit::init(&mut meta);
            meta.base
        };
        Watcher { data, _meta: meta }
    }

    /// Consume the Watcher and return the data stored inside. Watch callbacks
    /// will no longer run.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: WatcherInit + ?Sized> Watcher<T> {
    /// Get an immutable reference to the data stored in this Watcher.
    /// In order to ensure aliasing rules are maintained, this function will
    /// panic if called outside a watch callback or
    /// WatchContext::allow_watcher_access
    pub fn data(&self) -> &T {
        self.data.as_ref()
    }

    /// Get an mutable reference to the data stored in this Watcher.
    /// In order to ensure aliasing rules are maintained, this function will
    /// panic if called outside a watch callback or
    /// WatchContext::allow_watcher_access
    pub fn data_mut(&mut self) -> &mut T {
        self.data.as_mut()
    }
}

impl<T: WatcherInit + Default> Watcher<T> {
    /// Create a Watcher with default data. After creation, will run
    /// WatcherInit::init for the stored data.
    pub fn new() -> Self {
        Default::default()
    }
}

impl<T: WatcherInit + Default> Default for Watcher<T> {
    fn default() -> Self {
        Watcher::create(Default::default())
    }
}

impl<T: WatcherInit + Clone> Clone for Watcher<T> {
    fn clone(&self) -> Self {
        Watcher::create(self.data.as_ref().clone())
    }
}

impl<T: 'static> Watcher<T> {
    /// Get a value representing a unique id for this watcher. This value
    /// may outlive the watcher, and will never compare equal to a value
    /// returned by the id method of a Watcher other than this one.
    pub fn id(&self) -> WatcherId {
        WatcherId {
            ptr: self.data.new_borrowed().into_any(),
        }
    }
}

/// A type representing a unique id for a particular instance
/// of a watcher. This value may outlive the watcher, and will never
/// compare equal to a value returned by the id method of a Watcher
/// other than this one.
#[derive(Clone)]
pub struct WatcherId {
    ptr: BorrowedPointer<dyn std::any::Any>,
}

impl PartialEq for WatcherId {
    fn eq(&self, other: &WatcherId) -> bool {
        self.ptr.ptr_eq(&other.ptr)
    }
}

impl Eq for WatcherId {}

impl std::fmt::Debug for WatcherId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(WatcherId)")
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for Watcher<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        T::serialize(self.data.as_ref(), serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Watcher<T>
where
    T: WatcherInit + serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::create)
    }
}
