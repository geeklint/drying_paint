[![crates.io](https://img.shields.io/crates/v/drying_paint.svg)](https://crates.io/crates/drying_paint)
[![docs.rs](https://docs.rs/drying_paint/badge.svg)](https://docs.rs/drying_paint/)
[![Build Status](https://github.com/geeklint/drying_paint/workflows/Rust/badge.svg)](https://github.com/geeklint/drying_paint/actions)
![License](https://img.shields.io/crates/l/drying_paint?color=blueviolet)

The name 'drying_paint' comes from the expression "watching paint dry".
This module provides a system to "watch" some values for changes and run
code whenever they change.

The typical usage is as follows: you first define a structure to hold
data, including some "watched" data.

```rust
struct HelloData {
    name: Watched<String>,
    greeting: String,
}
```

Implementing the trait WatcherInit for that structure gives you an place
to set-up the code that should run when a watched value changes.

```rust
impl WatcherInit for HelloData {
    fn init(watcher: &mut WatcherMeta<Self>) {
        watcher.watch(|root| {
            root.greeting = format!("Hello, {}!", root.name);
        });
    }
}
```

Normally you need to wrap the data struct in a Watcher, so it's common
to alias the watcher type to cleanup the syntax a bit:
```rust
type Hello = Watcher<HelloData>;
```
Creating watchers and setting watched data needs to happen within a 
WatchContext. WatchContext::update_current() will cause all the pending
watcher code to run.

```rust
fn main() {
    let mut ctx = WatchContext::new();
    ctx.with(|| {
        let mut obj = Hello::new();
        *obj.data_mut().name = "Rust".to_string();
        WatchContext::update_current();
        assert_eq!(obj.data().greeting, "Hello, Rust!");
    });
}
```
