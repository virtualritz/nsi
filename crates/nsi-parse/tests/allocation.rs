//! Does the steady state allocate?
//!
//! R6 says no allocation per argument or per statement once the scratch
//! buffers have grown. A counting allocator answers that; reading the
//! code does not.

use nsi_parse::parse_stream;
use nsi_trait::{Action, Nsi};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Sink;

impl Nsi for Sink {
    type Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>;
    type Error = core::convert::Infallible;

    fn create(
        &self,
        _: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete(
        &self,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_attribute(
        &self,
        _: &str,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_attribute_at_time(
        &self,
        _: &str,
        _: f64,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete_attribute(&self, _: &str, _: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn connect(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn disconnect(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn evaluate(&self, _: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn render_control(
        &self,
        _: Action,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn corpus(meshes: usize) -> Vec<u8> {
    let mut out = String::new();
    for mesh in 0..meshes {
        out.push_str(&format!("Create \"mesh{mesh}\" \"mesh\"\n"));
        // No string *argument* here: those allocate inside
        // `nsi_ffi_wrap::Arg`, which owns its C strings, and that cost
        // is measured separately rather than blamed on the parser.
        out.push_str(&format!(
            "SetAttribute \"mesh{mesh}\"\n  \"P\" \"point\" 2 [ 0 0 0 1 2 3 ]\n  \"n\" \"int\" 1 4\n"
        ));
    }
    out.into_bytes()
}

/// One test, deliberately: the counter is global and `cargo test` runs
/// tests in parallel threads, so two of these would count each other's
/// allocations and report nonsense. That is not hypothetical -- split in
/// two, this measured 539 against 4910 and looked like a leak.
#[test]
fn allocation_behaviour() {
    let measure = |source: &[u8]| {
        parse_stream(source, &Sink).expect("warm");
        let before = ALLOCATIONS.load(Ordering::Relaxed);
        parse_stream(source, &Sink).expect("parse");
        ALLOCATIONS.load(Ordering::Relaxed) - before
    };

    // The parser itself must not allocate as a scene grows: the scratch
    // buffers are cleared rather than freed, parameter names and string
    // values are borrowed from the input, and the argument list lives on
    // the stack.
    let small = measure(&corpus(100));
    let large = measure(&corpus(1_000));
    println!("100 nodes: {small}; 1000 nodes: {large}");
    assert_eq!(
        small, large,
        "the parser must not allocate as the scene grows"
    );

    // String *arguments* are a different matter, and it is worth knowing
    // whose. `nsi_ffi_wrap::StringSlice` owns a `Vec<CString>` and a
    // pointer vector, because ɴsɪ's C boundary needs NUL-terminated
    // strings -- three allocations per string argument, inside the
    // argument type rather than in this parser.
    let mut with_strings = String::new();
    for mesh in 0..1_000 {
        with_strings.push_str(&format!(
            "Create \"m{mesh}\" \"mesh\"\nSetAttribute \"m{mesh}\"\n  \"a\" \"string\" 1 \"v{mesh}\"\n"
        ));
    }
    let strings = measure(&with_strings.into_bytes());
    println!("one string parameter, 1000 nodes: {strings} allocations");
    assert!(
        (2900..3100).contains(&strings),
        "expected about three per string argument, got {strings}"
    );
}
