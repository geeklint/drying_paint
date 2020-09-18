/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::{atomic, mpsc, Arc};

use super::{WatchedMeta, WatchContext};

#[derive(Default)]
pub(crate) struct ChannelsContext {
    activity: WatchedMeta,
    flag: Arc<atomic::AtomicBool>,
}

impl ChannelsContext {
    pub(crate) fn check_for_activity(&self) {
        if self.flag.swap(false, atomic::Ordering::AcqRel) {
            self.activity.trigger();
        }
    }
}

/// Create a new asynchronous channel which is designed to work within the
/// watch system.
///
/// See
/// [the std documentation](https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html)
/// for more information on channels.
///
/// Since the watch system is designed to be single-threaded, this channel
/// is designed for the use case where a background thread wants to post data
/// into the watch system.
///
/// ## Examples
///
/// ```rust
/// # use drying_paint::*;
/// # use std::sync::mpsc::TryRecvError::*;
/// # use std::rc::Rc;
/// # use std::cell::Cell;
/// # use std::time::Duration;
/// struct AsyncWatcher {
///     channel: WatchedReceiver<i32>,
///     received_data: Vec<i32>,
///     done: Rc<Cell<bool>>,
/// }
///
/// impl WatcherInit for AsyncWatcher {
///     fn init(watcher: &mut WatcherMeta<Self>) {
///         watcher.watch(|this| {
///             loop {
///                 eprintln!("values: {:?}", this.received_data);
///                 match this.channel.recv() {
///                     Ok(value) => this.received_data.push(value),
///                     Err(Empty) => break,
///                     Err(Disconnected) => {
///                         this.done.set(true);
///                         break;
///                     }
///                 }
///             }
///         });
///     }
/// }
/// fn main() {
///     let mut ctx = WatchContext::new();
///     ctx = ctx.with(|| {
///         let (tx, rx) = watched_channel();
///         let done = Rc::new(Cell::new(false));
///         let done2 = Rc::clone(&done);
///         let watcher = WatchContext::allow_watcher_access((), move |()| {
///             Watcher::create(AsyncWatcher {
///                 channel: rx,
///                 received_data: Vec::new(),
///                 done: done2,
///             })
///         });
///         std::thread::spawn(move || {
///             for value in &[54, 13, 71, -66, -13, -34, 12, -100, 68, 31] {
///                 tx.send(*value);
///                 std::thread::sleep(Duration::from_millis(10));
///             }
///             std::mem::drop(tx);
///         });
///         let start = std::time::Instant::now();
///         while !done.get() {
///             WatchContext::update_current();
///             assert!(start.elapsed().as_secs() < 2);
///             std::thread::sleep(Duration::from_millis(10));
///         }
///         let data = WatchContext::allow_watcher_access(watcher, |watcher| {
///             watcher.into_inner().received_data
///         });
///         assert_eq!(data, [54, 13, 71, -66, -13, -34, 12, -100, 68, 31]);
///     }).0;
/// }
/// ```
pub fn watched_channel<T>() -> (WatchedSender<T>, WatchedReceiver<T>) {
    let flag = WatchContext::expect_current(|ctx| {
        Arc::clone(&ctx.channels_context().flag)
    }, "watched_channel called outside WatchContext");
    let (sender, receiver) = mpsc::channel::<T>();
    (
        WatchedSender {
            inner: sender,
            flag,
        },
        WatchedReceiver {
            inner: receiver,
            _notsend: std::marker::PhantomData,
        }
    )
}

/// The sender half of a watched channel.
#[derive(Clone, Debug)]
pub struct WatchedSender<T> {
    inner: mpsc::Sender<T>,
    flag: Arc<atomic::AtomicBool>,
}

impl<T> WatchedSender<T> {
    /// Attempts to send a value on this channel, returning it back if it
    /// could not be sent.
    ///
    /// See
    /// [the std documentation](https://doc.rust-lang.org/std/sync/mpsc/struct.Sender.html#method.send)
    /// for more information
    pub fn send(&self, t: T) -> Result<(), mpsc::SendError<T>> {
        let ret = self.inner.send(t);
        if ret.is_ok() {
            self.flag.store(true, atomic::Ordering::Release);
        }
        ret
    }
}

impl<T> Drop for WatchedSender<T> {
    fn drop(&mut self) {
        self.flag.store(true, atomic::Ordering::Release);
    }
}

/// The receiver half of a watched channel.
///
/// The methods exposed on this type corospond to the non-blocking methods
/// on the
/// [std channel Receiver](https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html),
/// but they also bind watch closures, so that when new data is sent those
/// closures will be re-run.
#[derive(Debug)]
pub struct WatchedReceiver<T> {
    inner: mpsc::Receiver<T>,
    _notsend: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl<T> WatchedReceiver<T> {
    /// Attempts to return a pending value on this receiver.
    ///
    /// This corosponds to the `try_recv` method on the std Receiver, but
    /// additionally binds enclosing watch closures, so that they will be
    /// re-run when new data might be available.
    pub fn recv(&self) -> Result<T, mpsc::TryRecvError> {
        WatchContext::try_get_current(|ctx| {
            ctx.channels_context().activity.watched();
        });
        self.inner.try_recv()
    }

    /// Returns an iterator that will attempt to yield all pending values.
    ///
    /// This corosponds to the `try_iter` method on the std Receiver, but
    /// additionally binds enclosing watch closures, so that they will be
    /// re-run when new data might be available.
    pub fn iter(&self) -> mpsc::TryIter<T> {
        WatchContext::try_get_current(|ctx| {
            ctx.channels_context().activity.watched();
        });
        self.inner.try_iter()
    }
}
