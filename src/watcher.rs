/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::Watch;

/// This structure is used internally by Watcher<T>. It is passed to the init
/// function of WatcherInit, the trait which is required to be implemented by
/// the data stored in Watchers.
pub struct WatcherMeta<T: ?Sized> {
    data: Weak<RefCell<T>>,
    watches: Vec<Watch>,
}


impl<T: ?Sized + 'static> WatcherMeta<T> {
    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    pub fn watch<F>(&mut self, func: F)
        where F: Fn(&mut T) + 'static
    {
        let data = self.data.clone();
        let watch = Watch::new(data, func);
        self.watches.push(watch);
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
    data: Rc<RefCell<T>>,
    meta: WatcherMeta<T>,
}

impl<T: WatcherInit> Watcher<T> {
    /// Create a new Watcher. After creation, will run WatcherInit::init for
    /// the stored data.
    pub fn create(data: T) -> Self {
        let data = Rc::new(RefCell::new(data));
        let mdata = Rc::downgrade(&data);
        let mut this = Watcher {
            data: data,
            meta: WatcherMeta {
                data: mdata,
                watches: Vec::new(),
            },
        };
        WatcherInit::init(&mut this.meta);
        this
    }
}

impl<T: WatcherInit + ?Sized> Watcher<T> {
    /// Get an immutable reference to the data stored in this Watcher.
    /// Note that this follows the same rules as RefCell, and may panic if
    /// the runtime borrow checker detects and invalid borrow.
    pub fn data(&self) -> std::cell::Ref<T> {
        self.data.borrow()
    }

    /// Get an mutable reference to the data stored in this Watcher.
    /// Note that this follows the same rules as RefCell, and may panic if
    /// the runtime borrow checker detects and invalid borrow.
    pub fn data_mut(&mut self) -> std::cell::RefMut<T> {
        self.data.borrow_mut()
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
        Watcher::create(self.data.borrow().clone())
    }
}
