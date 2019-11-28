
mod trigger;
use trigger::{Watch, WatchRef, WatchSet};

mod watched;
pub use watched::{
    WatchedMeta, Watched, RefWatched, WatchedEvent
};

mod watcher;
pub use watcher::{
    WatcherMeta, WatcherInit, Watcher
};

mod context;
pub use context::WatchContext;

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
        value: Watched<i32>,
        inner: Inner,
    }

    impl OuterData {
        fn set_inner(&mut self, value: i32) {
            self.inner.value.set(value);
        }
    }

    impl WatcherInit for OuterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            watcher.watch(|root| {
                root.value.set(root.inner.value.get());
            });
        }
    }

    #[test]
    fn test_add() {
        let mut ctx = WatchContext::new();
        ctx.with(|| {
            let mut outer = Outer::new();
            outer.data().borrow_mut().set_inner(37);
            WatchContext::update_current();
            assert_eq!(outer.data().borrow().value.get(), 37);
        });
    }
}
