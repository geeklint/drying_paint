/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

//! The name 'drying_paint' comes from the expression "watching paint dry".
//! This module provides a system to "watch" some values for changes and run
//! code whenever they change.
//!
//! The typical usage is as follows: you first define a structure to hold
//! data, including some "watched" data.
//!
//! ```rust
//! # use drying_paint::*;
//! # type Hello = Watcher<HelloData>;
//! #[derive(Default)]
//! struct HelloData {
//!     name: Watched<String>,
//!     greeting: String,
//! }
//! # impl WatcherInit for HelloData {
//! #     fn init(watcher: &mut WatcherMeta<Self>) {
//! #         watcher.watch(|root| {
//! #             root.greeting = format!("Hello, {}!", root.name);
//! #         });
//! #     }
//! # }
//! # fn main() {
//! #     let mut ctx = WatchContext::new();
//! #     ctx = ctx.with(|| {
//! #         let obj = WatchContext::allow_watcher_access((), |()| {
//! #             let mut obj = Hello::new();
//! #             *obj.data_mut().name = "Rust".to_string();
//! #             obj
//! #         });
//! #         WatchContext::update_current();
//! #         let obj = WatchContext::allow_watcher_access(obj, |obj| {
//! #             assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #         });
//! #     }).0;
//! # }
//! ```
//!
//! Implementing the trait WatcherInit for that structure gives you an place
//! to set-up the code that should run when a watched value changes.
//!
//! ```rust
//! # use drying_paint::*;
//! # type Hello = Watcher<HelloData>;
//! # #[derive(Default)]
//! # struct HelloData {
//! #     name: Watched<String>,
//! #     greeting: String,
//! # }
//! impl WatcherInit for HelloData {
//!     fn init(watcher: &mut WatcherMeta<Self>) {
//!         watcher.watch(|root| {
//!             root.greeting = format!("Hello, {}!", root.name);
//!         });
//!     }
//! }
//! # fn main() {
//! #     let mut ctx = WatchContext::new();
//! #     ctx = ctx.with(|| {
//! #         let obj = WatchContext::allow_watcher_access((), |()| {
//! #             let mut obj = Hello::new();
//! #             *obj.data_mut().name = "Rust".to_string();
//! #             obj
//! #         });
//! #         WatchContext::update_current();
//! #         let obj = WatchContext::allow_watcher_access(obj, |obj| {
//! #             assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #         });
//! #     }).0;
//! # }
//! ```
//!
//! Normally you need to wrap the data struct in a Watcher, so it's common
//! to alias the watcher type to cleanup the syntax a bit:
//! ```rust
//! # use drying_paint::*;
//! type Hello = Watcher<HelloData>;
//! # #[derive(Default)]
//! # struct HelloData {
//! #     name: Watched<String>,
//! #     greeting: String,
//! # }
//! # impl WatcherInit for HelloData {
//! #     fn init(watcher: &mut WatcherMeta<Self>) {
//! #         watcher.watch(|root| {
//! #             root.greeting = format!("Hello, {}!", root.name);
//! #         });
//! #     }
//! # }
//! # fn main() {
//! #     let mut ctx = WatchContext::new();
//! #     ctx = ctx.with(|| {
//! #         let obj = WatchContext::allow_watcher_access((), |()| {
//! #             let mut obj = Hello::new();
//! #             *obj.data_mut().name = "Rust".to_string();
//! #             obj
//! #         });
//! #         WatchContext::update_current();
//! #         let obj = WatchContext::allow_watcher_access(obj, |obj| {
//! #             assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #         });
//! #     }).0;
//! # }
//! ```
//! Creating watchers and setting watched data needs to happen within a
//! WatchContext. WatchContext::update_current() will cause all the pending
//! watcher code to run.  WatchContext::allow_watcher_access() is used to
//! create and access the Watcher,  This is required in order to comply with
//! aliasing rules.
//!
//!
//! ```rust
//! # use drying_paint::*;
//! # type Hello = Watcher<HelloData>;
//! # #[derive(Default)]
//! # struct HelloData {
//! #     name: Watched<String>,
//! #     greeting: String,
//! # }
//! # impl WatcherInit for HelloData {
//! #     fn init(watcher: &mut WatcherMeta<Self>) {
//! #         watcher.watch(|root| {
//! #             root.greeting = format!("Hello, {}!", root.name);
//! #         });
//! #     }
//! # }
//! fn main() {
//!     let mut ctx = WatchContext::new();
//!     ctx = ctx.with(|| {
//!         let obj = WatchContext::allow_watcher_access((), |()| {
//!             let mut obj = Hello::new();
//!             *obj.data_mut().name = "Rust".to_string();
//!             obj
//!         });
//!         WatchContext::update_current();
//!         let obj = WatchContext::allow_watcher_access(obj, |obj| {
//!             assert_eq!(obj.data().greeting, "Hello, Rust!");
//!         });
//!     }).0;
//! }
//! ```

#![warn(missing_docs)]
#![allow(clippy::needless_doctest_main)]

mod trigger;
use trigger::{Watch, WatchRef, WatchSet};

mod context;
pub use context::WatchContext;

mod watched;
pub use watched::{Watched, WatchedCell, WatchedMeta};

mod pointer;

mod watcher;
pub use watcher::{Watcher, WatcherId, WatcherInit, WatcherMeta};

mod event;
pub use event::WatchedEvent;

mod channels;
pub use channels::{
    watched_channel, AtomicWatchedMeta, AtomicWatchedMetaTrigger,
    WatchedReceiver, WatchedSender,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Inner {
        value: Watched<i32>,
    }

    type Outer = Watcher<OuterData>;

    #[derive(Default)]
    struct OuterData {
        value: i32,
        inner: Inner,
    }

    impl OuterData {
        fn set_inner(&mut self, value: i32) {
            *self.inner.value = value;
        }
    }

    impl WatcherInit for OuterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            watcher.watch(|root| {
                root.value = *root.inner.value;
            });
        }
    }

    #[test]
    fn test_propogate() {
        let mut ctx = WatchContext::new();
        ctx = ctx
            .with(|| {
                let outer = WatchContext::allow_watcher_access((), |()| {
                    let mut outer = Outer::new();
                    outer.data_mut().set_inner(37);
                    outer
                });
                WatchContext::update_current();
                WatchContext::allow_watcher_access(outer, |outer| {
                    assert_eq!(outer.data().value, 37);
                });
            })
            .0;
        std::mem::drop(ctx);
    }

    #[derive(Default)]
    struct InnerId {
        value: Option<WatcherId>,
    }

    impl WatcherInit for InnerId {
        fn init(watcher: &mut WatcherMeta<Self>) {
            let id = watcher.id();
            watcher.watch(move |root| {
                root.value = Some(id.clone());
            });
        }
    }

    #[test]
    fn test_meta_id() {
        let mut ctx = WatchContext::new();
        ctx = ctx
            .with(|| {
                let watcher: Watcher<InnerId> =
                    WatchContext::allow_watcher_access((), |()| {
                        Watcher::new()
                    });
                let watcher_id = Some(watcher.id());
                let (watcher, watcher_id) = WatchContext::allow_watcher_access(
                    (watcher, watcher_id),
                    |(watcher, watcher_id)| {
                        assert_eq!(watcher.data().value, watcher_id);
                        (watcher, watcher_id)
                    },
                );

                let other: Watcher<InnerId> =
                    WatchContext::allow_watcher_access((), |()| {
                        Watcher::new()
                    });
                let other_id = Some(other.id());
                WatchContext::allow_watcher_access(
                    (watcher, other),
                    move |(watcher, other)| {
                        assert_ne!(other.data().value, watcher_id);
                        assert_ne!(watcher.data().value, other_id);
                    },
                );
            })
            .0;
        std::mem::drop(ctx);
    }

    #[derive(Default)]
    struct MutsTwice {
        value: Watched<i32>,
    }

    impl WatcherInit for MutsTwice {
        fn init(watcher: &mut WatcherMeta<Self>) {
            watcher.watch(|root| {
                root.value += 1;
                root.value += 1;
            });
        }
    }

    #[test]
    fn double_mut_in_watch() {
        let mut ctx = WatchContext::new();
        ctx.set_frame_limit(Some(100));
        ctx = ctx
            .with(|| {
                let watcher: Watcher<MutsTwice> =
                    WatchContext::allow_watcher_access((), |()| {
                        Watcher::new()
                    });
                WatchContext::update_current();
                WatchContext::allow_watcher_access(watcher, move |watcher| {
                    assert_eq!(watcher.data().value, 2);
                });
            })
            .0;
        std::mem::drop(ctx);
    }
}
