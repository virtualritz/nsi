#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![allow(non_snake_case)]
//#![warn(missing_docs)]
//#![warn(missing_doc_code_examples)]

use nsi_sys::*;

pub mod node;
pub use node::*;

#[cfg(feature = "ustr_handles")]
mod handle_ustr;
#[cfg(feature = "ustr_handles")]
use handle_ustr::HandleString;

#[cfg(not(feature = "ustr_handles"))]
mod handle_cstring;
#[cfg(not(feature = "ustr_handles"))]
use handle_cstring::HandleString;

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

#[cfg(not(feature = "manual_init"))]
lazy_static! {
    static ref NSI_API: api::ApiImpl =
        api::ApiImpl::new().expect("Could not load lib3delight");
}

// Default modules ----------------------------------------------------

#[macro_use]
pub mod argument;
pub use argument::*;

// Context should be in the crate root so we keep the module private.
pub mod context;
pub use context::*;

#[cfg(feature = "output")]
pub mod output;

#[cfg(feature = "output")]
pub use output::*;

mod tests;

trait Api {
    fn NSIBegin(
        &self,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) -> NSIContext;
    fn NSIEnd(&self, ctx: NSIContext);
    fn NSICreate(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const ::std::os::raw::c_char,
    );
    #[allow(clippy::too_many_arguments)]
    fn NSIConnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
    );
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    );
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
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
