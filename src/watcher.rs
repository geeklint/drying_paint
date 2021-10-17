/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {alloc::rc::Weak, core::cell::RefCell};

use crate::{DefaultOwner, Watch, WatchArg, WatchSet};

pub trait Watcher<'ctx, O: ?Sized = DefaultOwner> {
    fn init(init: impl WatcherInit<'ctx, Self, O>);
}

pub trait WatcherInit<'ctx, T: ?Sized, O: ?Sized = DefaultOwner> {
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut T) -> &mut Ch,
        Ch: Watcher<'ctx, O>;

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    #[cfg(feature = "std")]
    fn watch<F>(&mut self, func: F)
    where
        Self: WatcherInit<'static, T, DefaultOwner>,
        F: 'static + Fn(&mut T);

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, 'ctx, O>, &mut T);

    /*
        /// Watches have a debug name used in some error messages.  It defaults to
        /// the type name of the associated content (T).  This function allows
        /// overriding that name.
        pub fn set_debug_name(&mut self, debug_name: &'static str) {
            self.debug_name = debug_name;
        }
    */
}

pub trait WatcherHolder<'ctx, O: ?Sized>: Clone {
    type Content: ?Sized + Watcher<'ctx, O>;

    fn get_mut<F>(&self, owner: &mut O, f: F)
    where
        F: FnOnce(&mut Self::Content);
}

impl<'ctx, T, O> WatcherHolder<'ctx, O> for Weak<RefCell<T>>
where
    T: ?Sized + Watcher<'ctx, O>,
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

pub(crate) fn init_watcher<'ctx, T, O>(
    post_set: &Weak<WatchSet<'ctx, O>>,
    holder: &T,
    owner: &mut O,
    frame_id: u8,
) where
    T: 'ctx + ?Sized + WatcherHolder<'ctx, O>,
    O: ?Sized,
{
    T::Content::init(WatcherInitImpl {
        owner,
        post_set,
        frame_id,
        path: holder,
    });
}

#[derive(Clone)]
struct MapWatcherHolder<Base, Map> {
    base: Base,
    map: Map,
}

impl<'ctx, Base, Map, Res: ?Sized, Owner: ?Sized> WatcherHolder<'ctx, Owner>
    for MapWatcherHolder<Base, Map>
where
    Base: WatcherHolder<'ctx, Owner>,
    Map: Clone + Fn(&mut Base::Content) -> &mut Res,
    Res: Watcher<'ctx, Owner>,
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

struct WatcherInitImpl<'a, 'ctx, Owner: ?Sized, Path> {
    post_set: &'a Weak<WatchSet<'ctx, Owner>>,
    owner: &'a mut Owner,
    frame_id: u8,
    path: &'a Path,
}

impl<'a, 'ctx, Owner: ?Sized, Path, Content: ?Sized>
    WatcherInit<'ctx, Content, Owner>
    for WatcherInitImpl<'a, 'ctx, Owner, Path>
where
    Path: 'ctx + WatcherHolder<'ctx, Owner, Content = Content>,
{
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut Content) -> &mut Ch,
        Ch: Watcher<'ctx, Owner>,
    {
        Ch::init(WatcherInitImpl {
            post_set: self.post_set,
            owner: self.owner,
            frame_id: self.frame_id,
            path: &MapWatcherHolder {
                base: self.path.clone(),
                map: func,
            },
        });
    }

    #[cfg(feature = "std")]
    fn watch<F>(&mut self, func: F)
    where
        Self: WatcherInit<'static, Content, DefaultOwner>,
        F: 'static + Fn(&mut Content),
    {
        self.watch_explicit(move |arg, content| {
            arg.use_as_current(|| func(content));
        });
    }

    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, 'ctx, Owner>, &mut Content),
    {
        let current_path = self.path.clone();
        Watch::spawn(
            self.owner,
            self.post_set,
            self.frame_id,
            move |owner, arg| {
                current_path.get_mut(owner, |item| {
                    func(arg, item);
                });
            },
        );
    }
}
