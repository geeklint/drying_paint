
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

#[macro_export]
macro_rules! bind {
    ( $watcher:expr => $root:ident , $code:block ) => {
        {
            WatcherMeta::watch($watcher, |$root| $code);
        }
    };
}

#[macro_export]
macro_rules! bind_value {
    ( $watcher:expr => $root:ident , $target:expr , $source:expr ) => {
        {
            bind!($watcher => $root, {
                Watched::set(&mut $target, $source);
            });
        }
    };
}

#[macro_export]
macro_rules! bind_event {
    ( $watcher:expr => $root:ident ,
        $event:expr => $arg:ident ,
        $code:block ) => {
        {
            bind!($watcher => $root, {
                if let Some($arg) = WatchedEvent::get_current(&$event)
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
    fn test_propogate() {
        let mut ctx = WatchContext::new();
        ctx.with(|| {
            let mut outer = Outer::new();
            outer.data().borrow_mut().set_inner(37);
            WatchContext::update_current();
            assert_eq!(outer.data().borrow().value.get(), 37);
        });
    }

    type MacroOuter = Watcher<MacroOuterData>;

    #[derive(Default)]
    struct MacroOuterData {
        value: Watched<i32>,
        inner: Inner,
    }

    impl MacroOuterData {
        fn set_inner(&mut self, value: i32) {
            self.inner.value.set(value);
        }
    }

    impl WatcherInit for MacroOuterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            bind_value!(watcher => root, root.value, root.inner.value.get());
        }
    }

    #[test]
    fn test_macro() {
        let mut ctx = WatchContext::new();
        ctx.with(|| {
            let mut outer = MacroOuter::new();
            outer.data().borrow_mut().set_inner(53);
            WatchContext::update_current();
            assert_eq!(outer.data().borrow().value.get(), 53);
        });
    }

    type EventCounter = Watcher<EventCounterData>;

    #[derive(Default)]
    struct EventCounterData {
        counter: u32,
        add: WatchedEvent<u32>,
    }

    impl WatcherInit for EventCounterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            bind_event!(watcher => root, root.add => amount, {
                root.counter += amount;
            });
        }
    }

    #[test]
    fn test_event() {
        let mut ctx = WatchContext::new();
        ctx.with(|| {
            let mut item = EventCounter::new();
            item.data().borrow_mut().add.dispatch(5);
            item.data().borrow_mut().add.dispatch(3);
            WatchContext::update_current();
            item.data().borrow_mut().add.dispatch(7);
            WatchContext::update_current();
            assert_eq!(item.data().borrow().counter, 10);
        });
    }
}
