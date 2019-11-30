The name 'drying_paint' comes from the expression "watching paint dry".
This module provides a system to "watch" some values for changes and run
code whenever they change.

The typical usage is as follows: you first define a structure to hold
data, including some "watched" data.

```rust
# use drying_paint::*;
# type Hello = Watcher<HelloData>;
#[derive(Default)]
struct HelloData {
    name: Watched<String>,
    greeting: String,
}
# impl WatcherInit for HelloData {
#     fn init(watcher: &mut WatcherMeta<Self>) {
#         watcher.watch(|root| {
#             root.greeting = format!("Hello, {}!", root.name);
#         });
#     }
# }
# fn main() {
#     let mut ctx = WatchContext::new();
#     ctx.with(|| {
#         let mut obj = Hello::new();
#         *obj.data_mut().name = "Rust".to_string();
#         WatchContext::update_current();
#         assert_eq!(obj.data().greeting, "Hello, Rust!");
#     });
# }
```

Implementing the trait WatcherInit for that structure gives you an place
to set-up the code that should run when a watched value changes.

```rust
# use drying_paint::*;
# type Hello = Watcher<HelloData>;
# #[derive(Default)]
# struct HelloData {
#     name: Watched<String>,
#     greeting: String,
# }
impl WatcherInit for HelloData {
    fn init(watcher: &mut WatcherMeta<Self>) {
        watcher.watch(|root| {
            root.greeting = format!("Hello, {}!", root.name);
        });
    }
}
# fn main() {
#     let mut ctx = WatchContext::new();
#     ctx.with(|| {
#         let mut obj = Hello::new();
#         *obj.data_mut().name = "Rust".to_string();
#         WatchContext::update_current();
#         assert_eq!(obj.data().greeting, "Hello, Rust!");
#     });
# }
```

Normally you need to wrap the data struct in a Watcher, so it's common
to alias the watcher type to cleanup the syntax a bit:
```rust
# use drying_paint::*;
type Hello = Watcher<HelloData>;
# #[derive(Default)]
# struct HelloData {
#     name: Watched<String>,
#     greeting: String,
# }
# impl WatcherInit for HelloData {
#     fn init(watcher: &mut WatcherMeta<Self>) {
#         watcher.watch(|root| {
#             root.greeting = format!("Hello, {}!", root.name);
#         });
#     }
# }
# fn main() {
#     let mut ctx = WatchContext::new();
#     ctx.with(|| {
#         let mut obj = Hello::new();
#         *obj.data_mut().name = "Rust".to_string();
#         WatchContext::update_current();
#         assert_eq!(obj.data().greeting, "Hello, Rust!");
#     });
# }
```
Creating watchers and setting watched data needs to happen within a 
WatchContext. WatchContext::update_current() will cause all the pending
watcher code to run.

```rust
# use drying_paint::*;
# type Hello = Watcher<HelloData>;
# #[derive(Default)]
# struct HelloData {
#     name: Watched<String>,
#     greeting: String,
# }
# impl WatcherInit for HelloData {
#     fn init(watcher: &mut WatcherMeta<Self>) {
#         watcher.watch(|root| {
#             root.greeting = format!("Hello, {}!", root.name);
#         });
#     }
# }
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
