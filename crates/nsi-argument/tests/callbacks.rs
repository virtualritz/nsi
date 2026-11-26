//! A backend in Rust reads the error handler and the status callback of
//! an argument back, with no raw pointer, and frees them when it owns
//! them.

use nsi_argument as nsi;
use nsi_argument::{
    Callback, CallbackPtr, ErrorCallback, RenderStatus, StatusCallback,
};
use std::{
    ffi::c_void,
    sync::{Arc, Mutex},
};

/// The context type of a backend whose `Self` is the context.
struct Renderer(u32);

#[test]
fn an_error_handler_argument_reads_back_and_calls_its_closure() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let arg = nsi::callback!(
        "errorhandler",
        ErrorCallback::new(move |level: log::Level, id: i32, message: &str| {
            sink.lock().unwrap().push((level, id, message.to_string()));
        })
    );

    let callback = arg.callback().expect("a callback argument");
    let error_handler = callback.error_handler().expect("an error handler");
    error_handler(log::Level::Error, 3, "no camera");

    assert_eq!(
        *seen.lock().unwrap(),
        [(log::Level::Error, 3, "no camera".to_string())]
    );
    assert!(callback.status_callback::<Renderer>().is_none());
    // SAFETY: this test is the one owner of the callback.
    drop(unsafe { callback.adopt() });
}

#[test]
fn a_status_callback_reads_back_only_for_its_context_type() {
    let seen = Arc::new(Mutex::new(None));
    let sink = seen.clone();
    let callback = Callback::new(StatusCallback::new(
        move |renderer: &Renderer, status: RenderStatus| {
            *sink.lock().unwrap() = Some((renderer.0, status));
        },
    ));

    callback
        .status_callback::<Renderer>()
        .expect("a status callback for Renderer")(
        &Renderer(5),
        RenderStatus::Aborted,
    );

    assert_eq!(*seen.lock().unwrap(), Some((5, RenderStatus::Aborted)));
    assert!(callback.status_callback::<u32>().is_none());
    assert!(callback.error_handler().is_none());
    // SAFETY: this test is the one owner of the callback.
    drop(unsafe { callback.adopt() });
}

/// A callback type that does not name itself stays opaque.
struct Opaque(Box<u8>);

impl CallbackPtr for Opaque {
    fn to_ptr(self) -> *const c_void {
        Box::into_raw(self.0) as *const c_void
    }

    unsafe fn drop_ptr(ptr: *const c_void) {
        // SAFETY: `ptr` came from `to_ptr` and is reclaimed once.
        drop(unsafe { Box::from_raw(ptr as *mut u8) });
    }
}

#[test]
fn an_opaque_callback_reads_back_as_neither() {
    let callback = Callback::new(Opaque(Box::new(1)));
    assert!(callback.error_handler().is_none());
    assert!(callback.status_callback::<Renderer>().is_none());
    // SAFETY: this test is the one owner of the callback.
    drop(unsafe { callback.adopt() });
}

#[test]
fn an_adopted_callback_frees_its_closure_once_when_dropped() {
    let token = Arc::new(());
    let held = token.clone();
    let callback = Callback::new(ErrorCallback::new(
        move |_: log::Level, _: i32, _: &str| {
            let _ = &held;
        },
    ));
    assert_eq!(Arc::strong_count(&token), 2);

    // SAFETY: this test is the one owner of the callback.
    let adopted = unsafe { callback.adopt() };
    // A plain `Callback`, and every copy of it, does not own the closure.
    {
        let _copy = callback.clone();
    }
    assert_eq!(Arc::strong_count(&token), 2);
    assert!(adopted.error_handler().is_some());

    drop(adopted);
    assert_eq!(Arc::strong_count(&token), 1);
}
