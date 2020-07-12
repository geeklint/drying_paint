/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
  * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
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
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
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
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #     }).0;
//! # }
//! ```
//! Creating watchers and setting watched data needs to happen within a 
//! WatchContext. WatchContext::update_current() will cause all the pending
//! watcher code to run.
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
//!         let mut obj = Hello::new();
//!         *obj.data_mut().name = "Rust".to_string();
//!         WatchContext::update_current();
//!         assert_eq!(obj.data().greeting, "Hello, Rust!");
//!     }).0;
//! }
//! ```


#![warn(missing_docs)]

mod trigger;
use trigger::{Watch, WatchRef, WatchSet};

mod context;
pub use context::WatchContext;

mod watched;
pub use watched::{
    WatchedMeta, Watched
};

mod watcher;
pub use watcher::{
    WatcherMeta, WatcherInit, Watcher, WatcherId
};

mod event;
pub use event::{
    WatchedEvent
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
        ctx = ctx.with(|| {
            let mut outer = Outer::new();
            outer.data_mut().set_inner(37);
            WatchContext::update_current();
            assert_eq!(outer.data().value, 37);
        }).0;
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
        ctx = ctx.with(|| {
            let watcher: Watcher<InnerId> = Watcher::new();
            let watcher_id = Some(watcher.id());
            assert_eq!(watcher.data().value, watcher_id);

            let other: Watcher<InnerId> = Watcher::new();
            let other_id = Some(other.id());
            assert_ne!(other.data().value, watcher_id);
            assert_ne!(watcher.data().value, other_id);
        }).0;
        std::mem::drop(ctx);
    }

}
