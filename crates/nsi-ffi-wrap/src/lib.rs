#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![allow(non_snake_case)]
//#![warn(missing_docs)]
//#![warn(missing_doc_code_examples)]

use nsi_sys::*;
use std::{ffi::c_char, os::raw::c_int};

// Re-export dependencies needed by the macro
#[doc(hidden)]
/// The crate's hash map, hashed by `ahash`.
///
/// `std`'s `SipHash` resists collision attacks on adversarial input.
/// The keys here are callback and context identifiers the host itself
/// supplied, so the trade buys nothing and costs hashing speed.
///
/// **Not gated on the dynamic backend**, though it used to be:
/// `context.rs` and `c_adapter.rs` use it whichever way the renderer
/// is bound, so `link_lib3delight` did not compile at all.
pub(crate) type HashMap<K, V> = ahash::AHashMap<K, V>;

// Gated for the same reason: `dlopen2` is an optional dependency and
// is absent when the renderer is linked at build time.
#[cfg(not(feature = "link_lib3delight"))]
pub use dlopen2;
#[doc(hidden)]
pub extern crate lazy_static;
#[doc(hidden)]
pub use nsi_sys;

pub mod node;
pub use node::*;

mod handle;
pub use handle::Handle;
use handle::HandleString;

mod token;
pub use token::Token;

// Crate features -----------------------------------------------------

// `dlopen2` and `link_lib3delight` are two answers to the same
// question -- resolve the renderer at runtime, or link it at build
// time -- and `dlopen2` is on by default, so `--all-features` turns
// both on. That used to compile the linked backend against the
// dynamic one's expectations and fail with eleven `cannot find
// function NSIBegin` errors that named neither feature. Say it once,
// in a sentence, at the place the choice is made.
#[cfg(all(feature = "dlopen2", feature = "link_lib3delight"))]
compile_error!(
    "features `dlopen2` and `link_lib3delight` are mutually exclusive: \
     the first resolves lib3delight at runtime, the second links it at \
     build time. `dlopen2` is a default feature, so to link instead \
     use `--no-default-features --features link_lib3delight`. This is \
     also why `--all-features` is not a configuration this crate has; \
     test with `--features output`."
);

#[cfg(not(feature = "link_lib3delight"))]
mod dynamic;
#[cfg(feature = "link_lib3delight")]
mod linked;

#[cfg(not(feature = "link_lib3delight"))]
use self::dynamic as api;
#[cfg(feature = "link_lib3delight")]
use self::linked as api;

pub mod backend;

// Which renderer, and loading it on demand -------------------------

// **One library per name, not one per process.** The renderer used to
// be a `lazy_static` resolved the first time anything touched it, so a
// process had exactly one for its whole life and a `Context` could not
// ask for another. Two contexts naming the same renderer still share
// one library here; two naming different ones each get theirs.
//
// Keyed by the *canonical* name where there is one, so `"delight"` and
// `"3delight"` do not open the library twice. An unresolved name keys
// on itself, verbatim: it may be a path, and paths are case-sensitive
// on the systems that matter.
lazy_static::lazy_static! {
    static ref LOADED: parking_lot::Mutex<
        std::collections::HashMap<std::string::String, std::sync::Arc<api::ApiImpl>>
    > = parking_lot::Mutex::new(std::collections::HashMap::new());
}

// **Which renderer a raw context handle belongs to.**
//
// Two places get a bare `NSIContext` and no other clue: `From<NSIContext>`,
// and the status-callback trampoline, which C hands a handle rather than
// anything of ours. When there was one renderer per process the answer was
// never in doubt. Now it is, so it is recorded.
//
// **Handles are per renderer and both start at 1**, so two contexts from
// two renderers can collide. A collision cannot be resolved from a handle
// alone, so it is recorded as ambiguous rather than guessed at: the lookup
// then falls back to the default renderer and says so. Silently handing a
// callback a context wired to the wrong renderer is the failure worth
// spending this on.
lazy_static::lazy_static! {
    static ref BY_HANDLE: parking_lot::Mutex<
        std::collections::HashMap<NSIContext, Option<std::sync::Arc<dyn FfiApi>>>
    > = parking_lot::Mutex::new(std::collections::HashMap::new());
}

/// Record which renderer made a context.
pub(crate) fn remember(context: NSIContext, api: &std::sync::Arc<dyn FfiApi>) {
    BY_HANDLE
        .lock()
        .entry(context)
        .and_modify(|known| {
            // Same renderer reusing a handle it had freed is fine; a
            // different one holding the same number at the same time is
            // not answerable.
            if !matches!(known, Some(existing)
                if std::sync::Arc::ptr_eq(existing, api))
            {
                *known = None;
            }
        })
        .or_insert_with(|| Some(api.clone()));
}

/// Forget a context, once its renderer can no longer be asked about it.
pub(crate) fn forget(context: NSIContext) {
    BY_HANDLE.lock().remove(&context);
}

/// The renderer that made a context, or the default.
pub(crate) fn renderer_of(
    context: NSIContext,
) -> Result<std::sync::Arc<dyn FfiApi>, std::string::String> {
    match BY_HANDLE.lock().get(&context) {
        Some(Some(api)) => return Ok(api.clone()),
        Some(None) => eprintln!(
            "nsi: context {context} exists in more than one loaded \
             renderer, so which one it belongs to cannot be told from the \
             handle; using the default. Build the context with \
             `Context::new` and a \"renderer\" argument to avoid this."
        ),
        None => {}
    }

    renderer(None)
}

/// The renderer a name refers to, loading it if this is the first ask.
///
/// `None` means whatever [`backend::from_environment`] says, or
/// 3Delight.
pub(crate) fn renderer(
    name: Option<&str>,
) -> Result<std::sync::Arc<dyn FfiApi>, std::string::String> {
    let requested = match name {
        Some(name) => name.trim().to_string(),
        None => backend::from_environment()
            .unwrap_or_else(|| backend::KNOWN[0].name.to_string()),
    };

    let key = backend::lookup(&requested)
        .map(|backend| backend.name.to_string())
        .unwrap_or_else(|| requested.clone());

    let mut loaded = LOADED.lock();
    if let Some(api) = loaded.get(&key) {
        return Ok(api.clone() as _);
    }

    let api = std::sync::Arc::new(load_renderer(&requested)?);
    loaded.insert(key, api.clone());
    Ok(api as _)
}

#[cfg(not(feature = "link_lib3delight"))]
fn load_renderer(name: &str) -> Result<api::ApiImpl, std::string::String> {
    api::ApiImpl::load(name).map_err(|error| error.to_string())
}

// **Linked means one renderer, by construction.** The symbols are
// resolved by the linker at build time, so there is nothing to choose
// between at runtime and a name that asks for something else has to be
// refused rather than silently ignored -- a scene rendering in
// 3Delight when it asked for MoonRay is the failure this whole
// mechanism exists to prevent.
#[cfg(feature = "link_lib3delight")]
fn load_renderer(name: &str) -> Result<api::ApiImpl, std::string::String> {
    if backend::lookup(name).map(|backend| backend.name) != Some("3delight") {
        return Err(format!(
            "this build links 3Delight at build time, so it cannot also \
             load the ɴsɪ renderer {name:?} at runtime; rebuild without \
             the `link_lib3delight` feature to choose a renderer by name"
        ));
    }

    api::ApiImpl::new().map_err(|error| error.to_string())
}

// Default modules ----------------------------------------------------

#[macro_use]
pub mod argument;
pub use argument::*;

// The canonical NSI trait, the Attribute<T> typed-name machinery, type
// aliases (Point3F32/Color3F32/Matrix4F64/…) and standard node-type
// constants all live in `nsi-trait`. Re-export everything at this crate's
// root so users can write `nsi_ffi_wrap::Nsi`, `nsi_ffi_wrap::Point3F32`,
// `nsi_ffi_wrap::P`, etc. without depending on `nsi-trait` directly.
pub use ::nsi_trait::*;

// Bridges `Arg` onto `nsi_trait::ParamValue`. Reaches into `Arg`'s
// `pub(crate)` fields, so it has to live inside this crate.
mod param_value;

// `impl Nsi for Context`. Kept out of `context.rs` so the trait bridge
// stays reviewable next to the `ParamValue` one.
mod nsi_impl;

pub mod c_adapter;
pub use c_adapter::FfiApiAdapter;

pub mod c_api;

// Context should be in the crate root so we keep the module private.
pub mod context;
pub use context::*;

#[cfg(feature = "output")]
pub mod output;

#[cfg(feature = "output")]
pub use output::*;

mod tests;

#[macro_use]
pub mod macros;

/// Helper function to register output drivers for an API implementation.
#[cfg(feature = "output")]
pub fn register_output_drivers<A: FfiApi>(api: &A) {
    // Register typed drivers for each pixel type
    api.DspyRegisterDriver(
        b"ferris_f32\0" as *const u8 as _,
        Some(output::image_open::<f32>),
        Some(output::image_write::<f32>),
        Some(output::image_close::<f32>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_u32\0" as *const u8 as _,
        Some(output::image_open::<u32>),
        Some(output::image_write::<u32>),
        Some(output::image_close::<u32>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_i32\0" as *const u8 as _,
        Some(output::image_open::<i32>),
        Some(output::image_write::<i32>),
        Some(output::image_close::<i32>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_u16\0" as *const u8 as _,
        Some(output::image_open::<u16>),
        Some(output::image_write::<u16>),
        Some(output::image_close::<u16>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_i16\0" as *const u8 as _,
        Some(output::image_open::<i16>),
        Some(output::image_write::<i16>),
        Some(output::image_close::<i16>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_u8\0" as *const u8 as _,
        Some(output::image_open::<u8>),
        Some(output::image_write::<u8>),
        Some(output::image_close::<u8>),
        Some(output::image_query),
    );
    api.DspyRegisterDriver(
        b"ferris_i8\0" as *const u8 as _,
        Some(output::image_open::<i8>),
        Some(output::image_write::<i8>),
        Some(output::image_close::<i8>),
        Some(output::image_query),
    );
}

/// Trait abstracting the NSI C API functions.
///
/// This trait is implemented by both dynamic and linked API implementations,
/// allowing the rest of the code to be generic over the loading mechanism.
///
/// Renderer implementations use this trait to provide the underlying C API
/// functions, either through dynamic loading (dlopen) or static linking.
pub trait FfiApi: Send + Sync {
    fn NSIBegin(&self, nparams: c_int, params: *const NSIParam) -> NSIContext;
    fn NSIEnd(&self, ctx: NSIContext);
    fn NSICreate(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const c_char,
    );
    #[allow(clippy::too_many_arguments)]
    fn NSIConnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
    );
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    );

    #[cfg(feature = "output")]
    fn DspyRegisterDriver(
        &self,
        driver_name: *const c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError;
}
