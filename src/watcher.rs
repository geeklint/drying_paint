/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use {alloc::rc::Weak, core::cell::RefCell};

use crate::{DefaultOwner, WatchArg, WatchContext, WatchName};

pub trait Watcher<'ctx, O: ?Sized = DefaultOwner> {
    fn init(init: impl WatcherInit<'ctx, Self, O>);

    #[deprecated]
    fn debug_name() -> &'static str {
        core::any::type_name::<Self>()
    }
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

    #[cfg(feature = "std")]
    fn watch_for_new_child<F, W>(&mut self, func: F)
    where
        Self: WatcherInit<'static, T, DefaultOwner>,
        F: 'static + Fn(&mut T) -> Option<W>,
        W: 'static + WatcherHolder<'static, DefaultOwner>,
        W::Content: Watcher<'static, DefaultOwner>;

    /// Use this to set up a function which should be re-run whenever watched
    /// values referenced inside change.
    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, 'ctx, O>, &mut T);

    fn watch_for_new_child_explicit<F, W>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, 'ctx, O>, &mut T) -> Option<W>,
        W: 'ctx + WatcherHolder<'ctx, O>,
        W::Content: Watcher<'ctx, O>;
}

pub trait WatcherHolder<'ctx, O: ?Sized>: Clone {
    type Content: ?Sized;

    fn get_mut<F, R>(&self, owner: &mut O, f: F) -> Option<R>
    where
        F: FnOnce(&mut Self::Content) -> R;
}

impl<'ctx, T, O> WatcherHolder<'ctx, O> for Weak<RefCell<T>>
where
    T: ?Sized + Watcher<'ctx, O>,
    O: ?Sized,
{
    type Content = T;

    fn get_mut<F, R>(&self, _owner: &mut O, f: F) -> Option<R>
    where
        F: FnOnce(&mut Self::Content) -> R,
    {
        self.upgrade().map(|strong| f(&mut *strong.borrow_mut()))
    }
}

pub(crate) fn init_watcher<'ctx, T, O>(
    ctx: &mut WatchContext<'ctx, O>,
    holder: &T,
) where
    T: 'ctx + ?Sized + WatcherHolder<'ctx, O>,
    T::Content: Watcher<'ctx, O>,
    O: ?Sized,
{
    T::Content::init(WatcherInitImpl { ctx, path: holder });
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
{
    type Content = Res;

    fn get_mut<F, R>(&self, owner: &mut Owner, f: F) -> Option<R>
    where
        F: FnOnce(&mut Self::Content) -> R,
    {
        let map = &self.map;
        self.base.get_mut(owner, |item| f(map(item)))
    }
}

struct WatcherInitImpl<'a, 'ctx, Owner: ?Sized, Path> {
    ctx: &'a mut WatchContext<'ctx, Owner>,
    path: &'a Path,
}

impl<'a, 'ctx, Owner: ?Sized, Path, Content: ?Sized>
    WatcherInit<'ctx, Content, Owner>
    for WatcherInitImpl<'a, 'ctx, Owner, Path>
where
    Path: 'ctx + WatcherHolder<'ctx, Owner, Content = Content>,
    Content: Watcher<'ctx, Owner>,
{
    fn init_child<F, Ch>(&mut self, func: F)
    where
        F: 'static + Clone + Fn(&mut Content) -> &mut Ch,
        Ch: Watcher<'ctx, Owner>,
    {
        Ch::init(WatcherInitImpl {
            ctx: self.ctx,
            path: &MapWatcherHolder {
                base: self.path.clone(),
                map: func,
            },
        });
    }

    #[cfg(feature = "std")]
    #[cfg_attr(do_cycle_debug, track_caller)]
    fn watch<F>(&mut self, func: F)
    where
        Self: WatcherInit<'static, Content, DefaultOwner>,
        F: 'static + Fn(&mut Content),
    {
        self.watch_explicit(move |arg, content| {
            arg.use_as_current(|| func(content));
        });
    }

    #[cfg(feature = "std")]
    #[cfg_attr(do_cycle_debug, track_caller)]
    fn watch_for_new_child<F, T>(&mut self, func: F)
    where
        Self: WatcherInit<'static, Content, DefaultOwner>,
        F: 'static + Fn(&mut Content) -> Option<T>,
        T: 'static + WatcherHolder<'static, DefaultOwner>,
        T::Content: Watcher<'static, DefaultOwner>,
    {
        self.watch_for_new_child_explicit(move |arg, content| {
            arg.use_as_current(|| func(content))
        });
    }

    #[cfg_attr(do_cycle_debug, track_caller)]
    fn watch_explicit<F>(&mut self, func: F)
    where
        F: 'ctx + Fn(WatchArg<'_, 'ctx, Owner>, &mut Content),
    {
        let debug_name = WatchName::from_caller();
        let current_path = self.path.clone();
        self.ctx.add_watch_raw(debug_name, move |mut raw_arg| {
            let (owner, arg) = raw_arg.as_owner_and_arg();
            current_path.get_mut(owner, |item| {
                func(arg, item);
            });
        });
    }

    #[cfg_attr(do_cycle_debug, track_caller)]
    fn watch_for_new_child_explicit<F, T>(&mut self, func: F)
    where
        F: 'static + Fn(WatchArg<'_, 'ctx, Owner>, &mut Content) -> Option<T>,
        T: 'ctx + WatcherHolder<'ctx, Owner>,
        T::Content: Watcher<'ctx, Owner>,
    {
        let debug_name = WatchName::from_caller();
        let current_path = self.path.clone();
        self.ctx.add_watch_raw(debug_name, move |mut raw_arg| {
            let (owner, arg) = raw_arg.as_owner_and_arg();
            if let Some(watcher) = current_path
                .get_mut(owner, |item| func(arg, item))
                .flatten()
            {
                raw_arg.context().add_watcher(&watcher);
            }
        });
    }
}
