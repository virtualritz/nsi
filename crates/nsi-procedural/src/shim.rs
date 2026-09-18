//! The trait an author implements, and the shim bodies the macro calls.

use crate::{Params, Report};
use core::{
    any::Any,
    ffi::{CStr, c_char, c_int, c_uint},
    panic::AssertUnwindSafe,
};
use nsi_ffi_wrap::{Arg, Context, FfiParam, Nsi};

/// What a procedural's own code returns when it fails.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A procedural: code the renderer runs to contribute to the scene.
///
/// Implement this, then invoke
/// [`declare_procedural!`](crate::declare_procedural) to build a library
/// any ɴsɪ renderer can load -- or, in a renderer written against
/// [`Nsi`], call it through [`execute`](crate::execute) with no C in
/// between.
pub trait Procedural: Sized + Send + Sync + 'static {
    /// Called once per render, before any `execute`. Not the place to
    /// make ɴsɪ calls: there is no context to make them on yet.
    fn load(report: &Report<'_>, renderer_version: &str)
    -> Result<Self, Error>;

    /// Contributes to the scene, through `nsi`.
    ///
    /// May be called any number of times in one render, possibly from
    /// several threads at once -- hence `&self` and `Sync`. Make every
    /// call on the `nsi` handed to *this* call; it is not the context of
    /// any earlier one.
    fn execute<N>(
        &self,
        nsi: &N,
        report: &Report<'_>,
        params: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: Nsi<Arg<'call> = Arg<'call, 'static>>;
}

/// Runs `procedural` against a renderer's own [`Nsi`] implementation.
///
/// The route for a procedural linked into a Rust renderer: the calls
/// stay Rust, with no C strings and no parameter marshalling. `args` are
/// the procedural's parameters, as the renderer holds them.
pub fn execute<P, N>(
    procedural: &P,
    nsi: &N,
    report: &Report<'_>,
    args: &[Arg<'_, '_>],
) -> Result<(), Error>
where
    P: Procedural,
    for<'call> N: Nsi<Arg<'call> = Arg<'call, 'static>>,
{
    use nsi_ffi_wrap::ParamValue;

    let raw = args
        .iter()
        // SAFETY: `Arg` always has a C view; the `Option` is for
        // `ParamValue` implementations that do not.
        .map(|arg| arg.as_c_param().expect("an Arg has a C view"))
        .collect::<Vec<_>>();
    // SAFETY: each view borrows from `args`, which outlives this call.
    let params = unsafe { Params::from_raw(raw.as_ptr(), raw.len() as _) };
    procedural.execute(nsi, report, params)
}

/// `NSIReport_t`.
#[doc(hidden)]
pub type NSIReport = Option<unsafe extern "C" fn(c_int, c_int, *const c_char)>;

/// `NSIProceduralUnload_t`.
type Unload = unsafe extern "C" fn(c_int, NSIReport, *mut Descriptor);

/// `NSIProceduralExecute_t`.
type Execute = unsafe extern "C" fn(
    c_int,
    NSIReport,
    *mut Descriptor,
    c_int,
    *const nsi_ffi_wrap::nsi_sys::NSIParam,
);

/// `struct NSIProcedural_t`, which `nsi-sys` only has as an opaque blob.
#[doc(hidden)]
#[repr(C)]
pub struct Descriptor {
    pub(crate) nsi_version: c_uint,
    unload: Option<Unload>,
    execute: Option<Execute>,
}

/// What `NSIProceduralLoad` hands the renderer.
///
/// `#[repr(C)]` with the descriptor first, so a pointer to one is a
/// pointer to the other -- the over-allocation `nsi.pdf` §2.7.2
/// suggests for exactly this.
#[repr(C)]
struct Loaded<P> {
    descriptor: Descriptor,
    /// The ɴsɪ implementation that loaded the procedural. Every call
    /// goes through it (§2.7.1).
    library: String,
    procedural: P,
}

/// A [`Report`] that goes to the renderer, for the call at hand.
fn report_to(
    context: c_int,
    report: NSIReport,
) -> impl Fn(crate::Level, &str) + Sync {
    move |level, message| {
        let Some(report) = report else { return };
        // An interior NUL would end the message early; drop them.
        let message = std::ffi::CString::new(message.replace('\0', ""))
            // SAFETY: every NUL was just removed.
            .expect("no interior NUL");
        // SAFETY: the renderer's own function, called with the context
        // it was handed for this call.
        unsafe { report(context, level as c_int, message.as_ptr()) }
    }
}

/// What a panic said, if it said anything.
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message")
}

/// Reads a C string argument the renderer passed, or reports why not.
///
/// # Safety
/// `value` is null or a NUL-terminated C string valid for `'a`.
unsafe fn argument<'a>(
    value: *const c_char,
    what: &str,
    report: &Report<'_>,
) -> Option<&'a str> {
    if value.is_null() {
        report.error(&format!("the renderer passed no {what}"));
        return None;
    }
    // SAFETY: per this function's contract.
    let value = unsafe { CStr::from_ptr(value) }.to_str();
    if value.is_err() {
        report.error(&format!("the {what} is not UTF-8"));
    }
    value.ok()
}

/// The body of `NSIProceduralLoad`.
///
/// # Safety
/// The arguments the renderer passes `NSIProceduralLoad` (`nsi.pdf`
/// §2.7.1).
#[doc(hidden)]
pub unsafe fn shim_load<P: Procedural>(
    context: c_int,
    report: NSIReport,
    nsi_library_path: *const c_char,
    renderer_version: *const c_char,
) -> *mut Descriptor {
    let sink = report_to(context, report);
    let report = Report::new(&sink);

    // SAFETY: per the renderer's contract.
    let Some(library) =
        (unsafe { argument(nsi_library_path, "ɴsɪ library path", &report) })
    else {
        return core::ptr::null_mut();
    };
    // SAFETY: per the renderer's contract.
    let renderer_version =
        unsafe { argument(renderer_version, "renderer version", &report) }
            .unwrap_or_default();

    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        P::load(&report, renderer_version)
    })) {
        Ok(Ok(procedural)) => Box::into_raw(Box::new(Loaded {
            descriptor: Descriptor {
                nsi_version: nsi_ffi_wrap::nsi_sys::NSI_VERSION,
                unload: Some(shim_unload::<P>),
                execute: Some(shim_execute::<P>),
            },
            library: library.to_string(),
            procedural,
        }))
        .cast(),
        Ok(Err(error)) => {
            report.error(&format!("procedural failed to load: {error}"));
            core::ptr::null_mut()
        }
        Err(payload) => {
            report.error(&format!(
                "procedural panicked while loading: {}",
                panic_message(&*payload)
            ));
            core::ptr::null_mut()
        }
    }
}

/// The body of `NSIProceduralExecute_t`.
///
/// # Safety
/// The arguments the renderer passes (`nsi.pdf` §2.7.2), with
/// `descriptor` as returned by [`shim_load::<P>`].
unsafe extern "C" fn shim_execute<P: Procedural>(
    context: c_int,
    report: NSIReport,
    descriptor: *mut Descriptor,
    count: c_int,
    params: *const nsi_ffi_wrap::nsi_sys::NSIParam,
) {
    let sink = report_to(context, report);
    let report = Report::new(&sink);

    // SAFETY: `descriptor` is the first field of the `Loaded<P>` that
    // `shim_load::<P>` returned, and it lives until `shim_unload`.
    let loaded = unsafe { &*descriptor.cast::<Loaded<P>>() };

    // SAFETY: `context` is live for this call, and made by `library`.
    let nsi = match unsafe {
        Context::from_renderer_context(context, &loaded.library)
    } {
        Ok(nsi) => nsi,
        Err(error) => {
            report.error(&format!(
                "procedural cannot reach {}: {error}",
                loaded.library
            ));
            return;
        }
    };
    // SAFETY: `NSIParam_t` and `FfiParam` are layout-identical, and the
    // renderer guarantees `count` of them for this call.
    let params = unsafe { Params::from_raw(params.cast::<FfiParam>(), count) };

    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        loaded.procedural.execute(&nsi, &report, params)
    })) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => report.error(&format!("procedural failed: {error}")),
        Err(payload) => report.error(&format!(
            "procedural panicked: {}",
            panic_message(&*payload)
        )),
    }
}

/// The body of `NSIProceduralUnload_t`.
///
/// # Safety
/// `descriptor` as returned by [`shim_load::<P>`], unloaded once.
unsafe extern "C" fn shim_unload<P: Procedural>(
    context: c_int,
    report: NSIReport,
    descriptor: *mut Descriptor,
) {
    // SAFETY: per this function's contract; the renderer unloads once.
    let loaded = unsafe { Box::from_raw(descriptor.cast::<Loaded<P>>()) };
    if let Err(payload) =
        std::panic::catch_unwind(AssertUnwindSafe(move || drop(loaded)))
    {
        let sink = report_to(context, report);
        Report::new(&sink).error(&format!(
            "procedural panicked while unloading: {}",
            panic_message(&*payload)
        ));
    }
}
