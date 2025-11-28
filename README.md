[![crates.io](https://img.shields.io/crates/v/drying_paint.svg)](https://crates.io/crates/drying_paint)
[![docs.rs](https://docs.rs/drying_paint/badge.svg)](https://docs.rs/drying_paint/)
[![Build Status](https://github.com/geeklint/drying_paint/workflows/Rust/badge.svg)](https://github.com/geeklint/drying_paint/actions)
![License](https://img.shields.io/crates/l/drying_paint?color=blueviolet)

# drying_paint

The name 'drying_paint' comes from the expression "watching paint dry".
This module provides a system to "watch" some values for changes and run
code whenever they change.

## Example

```rust
use std::{rc::Rc, cell::RefCell};
use drying_paint::{Watcher, Watched, WatcherInit, WatchContext};
// define a type to hold data
struct Content {
    dest: i32,
    source: Watched<i32>,
}

// define Watcher trait for the type
impl Watcher<'static> for Content {
    fn init(mut init: impl WatcherInit<'static, Self>) {
        // set up a callback that will be re-run when
        // the Watched data changes
        init.watch(|root| {
            root.dest = *root.source;
        });
    }
}
// instantiate the content
let content = Rc::new(RefCell::new(Content {
    dest: 0,
    source: Watched::new(37),
}));
let weak = Rc::downgrade(&content);

// create the Context
let mut ctx = WatchContext::new();

// dest was 0 when instantiated
assert_eq!(content.borrow().dest, 0);

// after adding the watcher, the callback has run (once)
ctx.add_watcher(&weak);
assert_eq!(content.borrow().dest, 37);

// we can change the "watched" value
*content.borrow_mut().source = 43;
assert_eq!(content.borrow().dest, 37);

// and it will be updated when we call
// update on the context
ctx.update();
assert_eq!(content.borrow().dest, 43);
```
