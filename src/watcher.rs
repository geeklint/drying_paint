/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::rc::Weak;

use crate::{Watch, WatchArg, WatchSet, WatcherOwner};

pub trait WatcherContent<O: ?Sized = dyn WatcherOwner> {
    fn init(init: impl WatcherInit<Self, O>);
}

pub trait WatcherInit<T: ?Sized, O: ?Sized = dyn WatcherOwner> {
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut T) -> &mut Ch,
        Ch: WatcherContent<O>;

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch<F>(&mut self, func: F)
    where
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

pub(crate) trait WatcherPath<O: ?Sized>: Clone {
    type Item: ?Sized;

    fn get_mut<F>(&self, owner: &mut O, f: F)
    where
        F: FnOnce(&mut Self::Item);

    fn map<U, F>(self, map: F) -> MapWatcherPath<Self, F>
    where
        F: Fn(&mut Self::Item) -> &mut U,
    {
        MapWatcherPath { base: self, map }
    }
}

#[derive(Clone)]
pub(crate) struct MapWatcherPath<Base, Map> {
    base: Base,
    map: Map,
}

impl<Base, Map, Res: ?Sized, Owner: ?Sized> WatcherPath<Owner>
    for MapWatcherPath<Base, Map>
where
    Base: WatcherPath<Owner>,
    Map: Clone + Fn(&mut Base::Item) -> &mut Res,
{
    type Item = Res;

    fn get_mut<F>(&self, owner: &mut Owner, f: F)
    where
        F: FnOnce(&mut Self::Item),
    {
        let map = &self.map;
        self.base.get_mut(owner, |item| f(map(item)));
    }
}

struct WatcherInitImpl<'a, Owner: ?Sized, Path> {
    post_set: &'a Weak<WatchSet<Owner>>,
    owner: &'a mut Owner,
    path: Path,
}

impl<'a, Owner: ?Sized, Path, Content: ?Sized> WatcherInit<Content, Owner>
    for WatcherInitImpl<'a, Owner, Path>
where
    Path: 'static + WatcherPath<Owner, Item = Content>,
{
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut Content) -> &mut Ch,
        Ch: WatcherContent<Owner>,
    {
        Ch::init(WatcherInitImpl {
            post_set: self.post_set,
            owner: self.owner,
            path: self.path.clone().map(func),
        });
    }

    fn watch<F>(&mut self, func: F)
    where
        F: 'static + Fn(&mut Content),
    {
        todo!()
    }

    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, Owner>, &mut Content),
    {
        let current_path = self.path.clone();
        Watch::new(self.owner, self.post_set, move |owner, arg| {
            current_path.get_mut(owner, |item| {
                func(arg, item);
            });
        });
    }
}
