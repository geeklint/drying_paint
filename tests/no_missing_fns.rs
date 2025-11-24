/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

use drying_paint::{WatchedCellCore, WatchedCore, WatchedQueue};

fn function_exists<F>(_f: F) {}

#[allow(dead_code, clippy::extra_unused_lifetimes)]
fn test_no_missing_fns<'ctx>() {
    function_exists(<WatchedCore<'ctx, f32>>::get);
    function_exists(<WatchedCore<'ctx, f32>>::get_unwatched);
    #[cfg(feature = "std")]
    function_exists(<WatchedCore<'static, f32>>::get_auto);

    function_exists(<WatchedCore<'ctx, f32>>::get_mut);
    function_exists(<WatchedCore<'ctx, f32>>::get_mut_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCore<'static, f32>>::get_mut_auto);

    function_exists(<WatchedCore<'ctx, f32>>::replace);
    function_exists(<WatchedCore<'ctx, f32>>::replace_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCore<'static, f32>>::replace_auto);

    function_exists(<WatchedCore<'ctx, f32>>::take);
    function_exists(<WatchedCore<'ctx, f32>>::take_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCore<'static, f32>>::take_auto);

    function_exists(<WatchedCore<'ctx, f32>>::set_if_neq);
    function_exists(<WatchedCore<'ctx, f32>>::set_if_neq_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCore<'static, f32>>::set_if_neq_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::get);
    function_exists(<WatchedCellCore<'ctx, f32>>::get_unwatched);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::get_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::get_mut);
    function_exists(<WatchedCellCore<'ctx, f32>>::get_mut_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::get_mut_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::replace);
    function_exists(<WatchedCellCore<'ctx, f32>>::replace_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::replace_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::take);
    function_exists(<WatchedCellCore<'ctx, f32>>::take_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::take_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::set);
    function_exists(<WatchedCellCore<'ctx, f32>>::set_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::set_auto);

    function_exists(<WatchedCellCore<'ctx, f32>>::set_if_neq);
    function_exists(<WatchedCellCore<'ctx, f32>>::set_if_neq_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedCellCore<'static, f32>>::set_if_neq_auto);

    function_exists(<WatchedQueue<'ctx, f32>>::push);
    function_exists(<WatchedQueue<'ctx, f32>>::push_external);
    #[cfg(feature = "std")]
    function_exists(<WatchedQueue<'static, f32>>::push_auto);
}
