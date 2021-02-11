#![cfg_attr(
    all(debug_assertions, feature = "nightly"),
    feature(cstring_from_vec_with_nul)
)]
#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![allow(non_snake_case)]
//#![warn(missing_docs)]
//#![warn(missing_doc_code_examples)]

#[cfg(not(feature = "link_lib3delight"))]
#[macro_use]
extern crate dlopen_derive;

use nsi_sys::*;

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

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref NSI_API: api::ApiImpl = api::ApiImpl::new().expect("Could not load lib3delight.");
}

// Default modules ----------------------------------------------------

#[macro_use]
pub mod argument;
// Context should be in the crate root so we keep the module private.`
pub mod context;

#[cfg(feature = "output")]
pub mod output;

mod tests;

pub use crate::{
    argument::*,
    context::{Context, NodeType},
};

trait Api {
    fn NSIBegin(&self, nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t;
    fn NSIEnd(&self, ctx: NSIContext_t);
    fn NSICreate(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDelete(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        name: *const ::std::os::raw::c_char,
    );
    #[allow(clippy::too_many_arguments)]
    fn NSIConnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
    );
    fn NSIEvaluate(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIRenderControl(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );

    #[cfg(feature = "output")]
    fn DspyRegisterDriver(
        &self,
        driver_name: *const ::std::os::raw::c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError;
}
