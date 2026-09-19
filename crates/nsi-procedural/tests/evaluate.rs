//! Evaluates the example procedural in 3Delight, proving the entry point
//! is what a renderer actually resolves and calls.
//!
//! Needs 3Delight installed; evaluating a procedural renders nothing,
//! so no license is involved.
use nsi_ffi_wrap as nsi;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

/// Builds the example and returns the library's path.
fn build_example() -> PathBuf {
    let status = Command::new(env!("CARGO"))
        .args(["build", "--example", "hello_procedural"])
        .status()
        .expect("cargo build");
    assert!(status.success());

    // The build honours `CARGO_TARGET_DIR`, so the lookup must too.
    env::var("CARGO_TARGET_DIR")
        .ok()
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
        })
        .join("debug/examples/libhello_procedural.so")
}

/// A context whose messages are collected, and the collection.
fn new_context() -> (nsi::Context<'static>, Arc<Mutex<Vec<String>>>) {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let sink = messages.clone();
    let error_handler =
        nsi::ErrorCallback::new(move |_: log::Level, _: i32, message: &str| {
            sink.lock().unwrap().push(message.to_string())
        });
    let context = nsi::Context::new(Some(&[nsi::callback!(
        "errorhandler",
        error_handler
    )]))
    .expect("context");
    (context, messages)
}

fn evaluate(context: &nsi::Context<'_>, library: &Path, count: i32) {
    context.evaluate(&[
        nsi::string!("type", "dynamiclibrary"),
        nsi::string!("filename", library.to_str().unwrap()),
        nsi::integer_i32!("count", count),
    ]);
}

fn said(messages: &Mutex<Vec<String>>, needle: &str) -> bool {
    messages
        .lock()
        .unwrap()
        .iter()
        .any(|message| message.contains(needle))
}

#[test]
fn three_delight_loads_and_executes_the_procedural() {
    let library = build_example();
    let (context, messages) = new_context();

    evaluate(&context, &library, 3);

    assert!(
        said(&messages, "hello_procedural loaded by"),
        "load reports through the renderer: {:?}",
        messages.lock().unwrap()
    );
    assert!(
        said(&messages, "created 3 spheres"),
        "execute read `count` and reported: {:?}",
        messages.lock().unwrap()
    );
}

/// The procedural's `create` reached 3Delight's scene: re-creating its
/// node as another type is refused, naming the type the procedural gave
/// it. Without the procedural, the same call is accepted silently -- so
/// the refusal is caused by the procedural and nothing else.
#[test]
fn the_procedurals_calls_land_in_the_renderers_scene() {
    let library = build_example();
    let clash = "already exists as type 'particles'";

    let (context, messages) = new_context();
    context.create("row", nsi::MESH, None);
    assert!(
        !said(&messages, clash),
        "control: {:?}",
        messages.lock().unwrap()
    );

    let (context, messages) = new_context();
    evaluate(&context, &library, 2);
    context.create("row", nsi::MESH, None);
    assert!(said(&messages, clash), "{:?}", messages.lock().unwrap());
}

/// An `Err` from `execute` is reported through the renderer, and the
/// process carries on.
#[test]
fn an_execute_error_is_reported_through_the_renderer() {
    let library = build_example();
    let (context, messages) = new_context();

    evaluate(&context, &library, -1);

    assert!(
        said(&messages, "procedural failed: count must not be negative"),
        "{:?}",
        messages.lock().unwrap()
    );
    assert!(!said(&messages, "created"));
}
