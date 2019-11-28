
mod trigger;
use trigger::{Watch, WatchRef, WatchSet};

mod context;
pub use context::WatchContext;

mod watched;
pub use watched::{
    WatchedMeta, Watched, RefWatched
};

mod watcher;
pub use watcher::{
    WatcherMeta, WatcherInit, Watcher
};

mod event;
pub use event::{
    WatchedEvent
};

#[macro_export]
macro_rules! bind {
    ( $watcher:expr => $root:ident , $code:block ) => {
        {
            $crate::WatcherMeta::watch($watcher, |$root| $code);
        }
    };
}

#[macro_export]
macro_rules! bind_value {
    ( $watcher:expr => $root:ident , $target:expr , $source:expr ) => {
        {
            $crate::bind!($watcher => $root, {
                $crate::Watched::set(&mut $target, $source);
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
            outer.data_mut().set_inner(37);
            WatchContext::update_current();
            assert_eq!(outer.data().value.get(), 37);
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
            outer.data_mut().set_inner(53);
            WatchContext::update_current();
            assert_eq!(outer.data().value.get(), 53);
        });
    }

    #[derive(Default)]
    struct RefInner {
        value: RefWatched<(i32, i32)>,
    }

    type RefOuter = Watcher<RefOuterData>;

    #[derive(Default)]
    struct RefOuterData {
        value: (i32, i32),
        inner: RefInner,
    }

    impl RefOuterData {
        fn set_inner(&mut self, value: (i32, i32)) {
            self.inner.value.0 = value.0;
            self.inner.value.1 = value.1;
        }
    }

    impl WatcherInit for RefOuterData {
        fn init(watcher: &mut WatcherMeta<Self>) {
            bind!(watcher => root, {
                root.value.0 = root.inner.value.1;
                root.value.1 = root.inner.value.0;
            });
        }
    }

    #[test]
    fn test_ref() {
        let mut ctx = WatchContext::new();
        ctx.with(|| {
            let mut outer = RefOuter::new();
            outer.data_mut().set_inner((11, 13));
            WatchContext::update_current();
            assert_eq!(outer.data().value, (13, 11));
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
            item.data_mut().add.dispatch(7);
            WatchContext::update_current();
            assert_eq!(item.data().counter, 7);
            item.data_mut().add.dispatch(9);
            item.data_mut().add.dispatch(3);
            WatchContext::update_current();
            assert_eq!(item.data().counter, 19);
        });
    }
}
