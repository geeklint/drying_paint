/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use std::rc::Weak;

use crate::{Watch, WatchArg, WatchSet};

pub trait WatcherContent<'ctx> {
    fn init(init: impl WatcherInit<'ctx, Self>);
}

pub trait WatcherInit<'ctx, T: ?Sized> {
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut T) -> &mut Ch,
        Ch: 'ctx + WatcherContent<'ctx>;

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch<F>(&mut self, func: F)
    where
        F: 'static + Fn(&mut T);

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg, &mut T);

    /*
        /// Watches have a debug name used in some error messages.  It defaults to
        /// the type name of the associated content (T).  This function allows
        /// overriding that name.
        pub fn set_debug_name(&mut self, debug_name: &'static str) {
            self.debug_name = debug_name;
        }
    */
}

pub(crate) trait WatcherPath<Owner: ?Sized>: Clone {
    type Item: ?Sized;

    fn get_mut<F>(&self, owner: &mut Owner, f: F)
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

struct WatcherInitImpl<'a, 'ctx, Owner: ?Sized, Path> {
    post_set: &'a Weak<WatchSet<'ctx>>,
    owner: &'a mut Owner,
    path: Path,
}

impl<'a, 'ctx, Owner: ?Sized, Path, Content: ?Sized> WatcherInit<'ctx, Content>
    for WatcherInitImpl<'a, 'ctx, Owner, Path>
where
    Path: 'ctx + WatcherPath<Owner, Item = Content>,
    Content: 'ctx,
{
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut Content) -> &mut Ch,
        Ch: 'ctx + WatcherContent<'ctx>,
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
        F: 'static + Fn(WatchArg, &mut Content),
    {
        let current_path = self.path.clone();
        Watch::new(
            move |arg| {
                current_path.get_mut(todo!(), |item| {
                    func(arg, item);
                });
            },
            self.post_set,
        );
    }
}
