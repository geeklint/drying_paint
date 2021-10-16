/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2021 Violet Leonard */

//! The name 'drying_paint' comes from the expression "watching paint dry".
//! This module provides a system to "watch" some values for changes and run
//! code whenever they change.
//!
//! ```rust
//! use std::{rc::Rc, cell::RefCell};
//! use drying_paint::{Watcher, Watched, WatcherInit, WatchContext};
//! // define a type to hold data
//! struct Content {
//!     dest: i32,
//!     source: Watched<i32>,
//! }
//!
//! // define Watcher trait for the type
//! impl Watcher<'static> for Content {
//!     fn init(mut init: impl WatcherInit<'static, Self>) {
//!         // set up a callback that will be re-run when
//!         // the Watched data changes
//!         init.watch(|root| {
//!             root.dest = *root.source;
//!         });
//!     }
//! }
//! // instantiate the content
//! let content = Rc::new(RefCell::new(Content {
//!     dest: 0,
//!     source: Watched::new(37),
//! }));
//! let weak = Rc::downgrade(&content);
//!
//! // create the Context
//! let mut ctx = WatchContext::new();
//!
//! // dest was 0 when instantiated
//! assert_eq!(content.borrow().dest, 0);
//!
//! // after adding the watcher, the callback has run (once)
//! ctx.add_watcher(&weak);
//! assert_eq!(content.borrow().dest, 37);
//!
//! // we can change the "watched" value
//! *content.borrow_mut().source = 43;
//! assert_eq!(content.borrow().dest, 37);
//!
//! // and it will be updated when we call
//! // update on the context
//! ctx.update();
//! assert_eq!(content.borrow().dest, 43);
//! ```

#![cfg_attr(not(any(test, feature = "std")), no_std)]
//#![warn(missing_docs)]
#![deny(rust_2018_idioms)]
#![allow(clippy::needless_doctest_main)]

extern crate alloc;

mod trigger;
pub use trigger::WatchArg;
use trigger::{Watch, WatchSet};

mod context;
pub use context::{DefaultOwner, WatchContext};

mod watched_core;
pub use watched_core::{WatchedCellCore, WatchedCore, WatchedMeta};

#[cfg(feature = "std")]
mod watched;
#[cfg(feature = "std")]
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
