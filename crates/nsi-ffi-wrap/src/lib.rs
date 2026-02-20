#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![allow(non_snake_case)]
//#![warn(missing_docs)]
//#![warn(missing_doc_code_examples)]

use nsi_sys::*;
use std::{ffi::c_char, os::raw::c_int};

// Re-export dependencies needed by the macro
#[cfg(not(feature = "link_lib3delight"))]
#[doc(hidden)]
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

#[cfg(not(feature = "link_lib3delight"))]
mod dynamic;
#[cfg(feature = "link_lib3delight")]
mod linked;

#[cfg(not(feature = "link_lib3delight"))]
use self::dynamic as api;
#[cfg(feature = "link_lib3delight")]
use self::linked as api;

// API initalization/on-demand loading of lib3delight -----------------

#[cfg(not(feature = "manual_init"))]
lazy_static::lazy_static! {
    static ref NSI_API: api::ApiImpl =
        api::ApiImpl::new().expect("Could not load lib3delight");
}

// Default modules ----------------------------------------------------

#[macro_use]
pub mod argument;
pub use argument::*;

pub mod nsi_trait;
// Re-export public types at crate root
pub use nsi_trait::{Action, NodeType, Nsi, NsiExt};

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
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
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
