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

//#![warn(missing_docs)]
#![deny(rust_2018_idioms)]
#![allow(clippy::needless_doctest_main)]

mod trigger;
pub use trigger::WatchArg;
use trigger::{Watch, WatchSet};

mod context;
pub use context::{DefaultOwner, WatchContext};

mod watched_core;
pub use watched_core::{WatchedCellCore, WatchedCore, WatchedMeta};

mod watched;
pub use watched::{Watched, WatchedCell};

mod watcher;
pub use watcher::{Watcher, WatcherHolder, WatcherInit};

/*
mod event;
pub use event::WatchedEvent;

mod channels;
pub use channels::{
    watched_channel, AtomicWatchedMeta, AtomicWatchedMetaTrigger,
    WatchedReceiver, WatchedSender,
};
*/

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn simple_propogate_core() {
        struct Content {
            dest: i32,
            source: WatchedCore<'static, i32>,
        }

        impl Watcher<'static> for Content {
            fn init(mut init: impl WatcherInit<'static, Self>) {
                init.watch_explicit(|arg, root| {
                    root.dest = *root.source.get(arg);
                });
            }
        }
        let content = Rc::new(RefCell::new(Content {
            dest: 0,
            source: WatchedCore::new(37),
        }));
        let weak = Rc::downgrade(&content);

        let mut ctx = WatchContext::new();
        assert_eq!(content.borrow().dest, 0);
        ctx.add_watcher(&weak);
        assert_eq!(content.borrow().dest, 37);
        *content.borrow_mut().source.get_mut_external() = 43;
        assert_eq!(content.borrow().dest, 37);
        ctx.update();
        assert_eq!(content.borrow().dest, 43);
        ctx.update();
        assert_eq!(content.borrow().dest, 43);
    }

    #[test]
    fn simple_propogate() {
        struct Content {
            dest: i32,
            source: Watched<i32>,
        }

        impl Watcher<'static> for Content {
            fn init(mut init: impl WatcherInit<'static, Self>) {
                init.watch(|root| {
                    root.dest = *root.source;
                });
            }
        }
        let content = Rc::new(RefCell::new(Content {
            dest: 0,
            source: Watched::new(37),
        }));
        let weak = Rc::downgrade(&content);

        let mut ctx = WatchContext::new();
        assert_eq!(content.borrow().dest, 0);
        ctx.add_watcher(&weak);
        assert_eq!(content.borrow().dest, 37);
        *content.borrow_mut().source = 43;
        assert_eq!(content.borrow().dest, 37);
        ctx.update();
        assert_eq!(content.borrow().dest, 43);
        ctx.update();
        assert_eq!(content.borrow().dest, 43);
    }

    #[test]
    fn double_mut_in_watch() {
        #[derive(Default)]
        struct MutsTwice {
            value: Watched<i32>,
        }

        impl Watcher<'static> for MutsTwice {
            fn init(mut init: impl WatcherInit<'static, Self>) {
                init.watch(|root| {
                    root.value += 1;
                    root.value += 1;
                });
            }
        }

        let content = Rc::new(RefCell::new(MutsTwice {
            value: Watched::new(0_i32),
        }));
        let weak = Rc::downgrade(&content);

        let mut ctx = WatchContext::new();
        ctx.set_frame_limit(Some(100));
        ctx.add_watcher(&weak);
        assert_eq!(*content.borrow().value, 2);
        ctx.update();
        assert_eq!(*content.borrow().value, 2);
        *content.borrow_mut().value = 41;
        ctx.update();
        assert_eq!(*content.borrow().value, 43);
    }
}
