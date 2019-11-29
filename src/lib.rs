//! The name 'drying_paint' comes from the expression "watching paint dry".
//! This module provides a system to "watch" some values for changes and run
//! code whenever they change.
//!
//! The typical usage is as follows: you first define a structure to hold
//! data, including some "watched" data.
//!
//! ```
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
//! #     ctx.with(|| {
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #     });
//! # }
//! ```
//!
//! Implementing the trait WatcherInit for that structure gives you an place
//! to set-up the code that should run when a watched value changes.
//!
//! ```
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
//! #     ctx.with(|| {
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #     });
//! # }
//! ```
//!
//! Normally you need to wrap the data struct in a Watcher, so it's common
//! to alias the watcher type to cleanup the syntax a bit:
//! ```
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
//! #     ctx.with(|| {
//! #         let mut obj = Hello::new();
//! #         *obj.data_mut().name = "Rust".to_string();
//! #         WatchContext::update_current();
//! #         assert_eq!(obj.data().greeting, "Hello, Rust!");
//! #     });
//! # }
//! ```
//! Creating watchers and setting watched data needs to happen within a 
//! WatchContext. WatchContext::update_current() will cause all the pending
//! watcher code to run.
//!
//! ```
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
//!     ctx.with(|| {
//!         let mut obj = Hello::new();
//!         *obj.data_mut().name = "Rust".to_string();
//!         WatchContext::update_current();
//!         assert_eq!(obj.data().greeting, "Hello, Rust!");
//!     });
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
    WatcherMeta, WatcherInit, Watcher
};

mod event;
pub use event::{
    WatchedEvent
};

/// An ergonomic wrapper for binding to an WatchedEvent. This is expected to
/// be used from within a WatcherInit implementation.
/// ```
/// use drying_paint::*;
///
/// type EventCounter = Watcher<EventCounterData>;
///
/// #[derive(Default)]
/// struct EventCounterData {
///     counter: u32,
///     add: WatchedEvent<u32>,
/// }
///
/// impl WatcherInit for EventCounterData {
///     fn init(watcher: &mut WatcherMeta<Self>) {
///         bind_event!(watcher => root, root.add => amount, {
///             root.counter += amount;
///         });
///     }
/// }
///
/// fn main() {
///     let mut ctx = WatchContext::new();
///     ctx.with(|| {
///         let mut item = EventCounter::new();
///         item.data_mut().add.dispatch(7);
///         WatchContext::update_current();
///         assert_eq!(item.data().counter, 7);
///         item.data_mut().add.dispatch(9);
///         item.data_mut().add.dispatch(3);
///         WatchContext::update_current();
///         assert_eq!(item.data().counter, 19);
///     });
/// }
/// ```
#[macro_export]
macro_rules! bind_event {
    ( $watcher:expr => $root:ident ,
        $event:expr => $arg:ident ,
        $code:block ) => {
        {
            $crate::WatcherMeta::watch($watcher, |$root| {
                if let Some($arg) = $crate::WatchedEvent::get_current(&mut $event)
                    $code
            });
        }
    };
}

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
        ctx.with(|| {
            let mut outer = Outer::new();
            outer.data_mut().set_inner(37);
            WatchContext::update_current();
            assert_eq!(outer.data().value, 37);
        });
    }

}
