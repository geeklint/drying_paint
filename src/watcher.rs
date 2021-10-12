/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::rc::Weak;

use crate::{DefaultOwner, Watch, WatchArg, WatchSet};

pub trait WatcherContent<O: ?Sized = DefaultOwner> {
    fn init(init: impl WatcherInit<Self, O>);
}

pub trait WatcherInit<T: ?Sized, O: ?Sized = DefaultOwner> {
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut T) -> &mut Ch,
        Ch: WatcherContent<O>;

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch<F>(&mut self, func: F)
    where
        Self: WatcherInit<T, DefaultOwner>,
        F: 'static + Fn(&mut T);

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, O>, &mut T);

    /*
        /// Watches have a debug name used in some error messages.  It defaults to
        /// the type name of the associated content (T).  This function allows
        /// overriding that name.
        pub fn set_debug_name(&mut self, debug_name: &'static str) {
            self.debug_name = debug_name;
        }
    */
}

pub trait WatcherHolder<O: ?Sized>: Clone {
    type Content: ?Sized + WatcherContent<O>;

    fn get_mut<F>(&self, owner: &mut O, f: F)
    where
        F: FnOnce(&mut Self::Content);
}

impl<T, O> WatcherHolder<O> for Weak<core::cell::RefCell<T>>
where
    T: ?Sized + WatcherContent<O>,
    O: ?Sized,
{
    type Content = T;

    fn get_mut<F>(&self, _owner: &mut O, f: F)
    where
        F: FnOnce(&mut Self::Content),
    {
        if let Some(strong) = self.upgrade() {
            f(&mut *strong.borrow_mut());
        }
    }
}

pub(crate) fn init_watcher<T, O>(
    post_set: &Weak<WatchSet<O>>,
    holder: &T,
    owner: &mut O,
) where
    T: 'static + ?Sized + WatcherHolder<O>,
    O: ?Sized,
{
    T::Content::init(WatcherInitImpl {
        owner,
        path: holder,
        post_set,
    });
}

#[derive(Clone)]
struct MapWatcherHolder<Base, Map> {
    base: Base,
    map: Map,
}

impl<Base, Map, Res: ?Sized, Owner: ?Sized> WatcherHolder<Owner>
    for MapWatcherHolder<Base, Map>
where
    Base: WatcherHolder<Owner>,
    Map: Clone + Fn(&mut Base::Content) -> &mut Res,
    Res: WatcherContent<Owner>,
{
    type Content = Res;

    fn get_mut<F>(&self, owner: &mut Owner, f: F)
    where
        F: FnOnce(&mut Self::Content),
    {
        let map = &self.map;
        self.base.get_mut(owner, |item| f(map(item)));
    }
}

struct WatcherInitImpl<'a, Owner: ?Sized, Path> {
    post_set: &'a Weak<WatchSet<Owner>>,
    owner: &'a mut Owner,
    path: &'a Path,
}

impl<'a, Owner: ?Sized, Path, Content: ?Sized> WatcherInit<Content, Owner>
    for WatcherInitImpl<'a, Owner, Path>
where
    Path: 'static + WatcherHolder<Owner, Content = Content>,
{
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut Content) -> &mut Ch,
        Ch: WatcherContent<Owner>,
    {
        Ch::init(WatcherInitImpl {
            post_set: self.post_set,
            owner: self.owner,
            path: &MapWatcherHolder {
                base: self.path.clone(),
                map: func,
            },
        });
    }

    fn watch<F>(&mut self, func: F)
    where
        Self: WatcherInit<Content, DefaultOwner>,
        F: 'static + Fn(&mut Content),
    {
        self.watch_explicit(move |arg, content| {
            arg.use_as_current(|| func(content));
        });
    }

    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, Owner>, &mut Content),
    {
        let current_path = self.path.clone();
        Watch::spawn(self.owner, self.post_set, move |owner, arg| {
            current_path.get_mut(owner, |item| {
                func(arg, item);
            });
        });
    }
}
