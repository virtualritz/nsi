use crate::FfiApi;
use nsi_sys::*;
use std::{error::Error, ffi::c_char, os::raw::c_int};

pub type ApiImpl = LinkedApi;

#[derive(Debug)]
pub struct LinkedApi {}

impl LinkedApi {
    #[inline]
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let api = LinkedApi {};

        #[cfg(feature = "output")]
        super::register_output_drivers(&api);

        Ok(api)
    }
}

impl FfiApi for LinkedApi {
    #[inline]
    fn NSIBegin(&self, nparams: c_int, params: *const NSIParam) -> NSIContext {
        // SAFETY: NSIBegin is an external C function from the NSI library.
        // The params pointer is valid for at least nparams elements as guaranteed
        // by the caller through get_c_param_vec().
        unsafe { NSIBegin(nparams, params) }
    }

    #[inline]
    fn NSIEnd(&self, ctx: NSIContext) {
        // SAFETY: NSIEnd is an external C function from the NSI library.
        // The ctx handle is valid as it was created by NSIBegin.
        unsafe { NSIEnd(ctx) };
    }

    #[inline]
    fn NSICreate(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSICreate is an external C function from the NSI library.
        // All pointers (handle, type_, params) are valid as guaranteed by the
        // caller through HandleString and get_c_param_vec().
        unsafe { NSICreate(ctx, handle, type_, nparams, params) };
    }

    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSIDelete is an external C function from the NSI library.
        // The handle and params pointers are valid as guaranteed by the caller.
        unsafe { NSIDelete(ctx, handle, nparams, params) };
    }

    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSISetAttribute is an external C function from the NSI library.
        // The object handle and params pointers are valid as guaranteed by the caller.
        unsafe { NSISetAttribute(ctx, object, nparams, params) };
    }

    #[inline]
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSISetAttributeAtTime is an external C function from the NSI library.
        // The object handle and params pointers are valid as guaranteed by the caller.
        unsafe { NSISetAttributeAtTime(ctx, object, time, nparams, params) };
    }

    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const c_char,
    ) {
        // SAFETY: NSIDeleteAttribute is an external C function from the NSI library.
        // The object handle and name pointer are valid C strings as guaranteed
        // by HandleString and Ustr types.
        unsafe { NSIDeleteAttribute(ctx, object, name) };
    }

    #[inline]
    fn NSIConnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSIConnect is an external C function from the NSI library.
        // All pointers are valid C strings or parameter arrays as guaranteed
        // by HandleString, Ustr, and get_c_param_vec().
        unsafe {
            NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params)
        };
    }

    #[inline]
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
    ) {
        // SAFETY: NSIDisconnect is an external C function from the NSI library.
        // All pointers are valid C strings as guaranteed by HandleString and Ustr.
        unsafe { NSIDisconnect(ctx, from, from_attr, to, to_attr) };
    }

    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSIEvaluate is an external C function from the NSI library.
        // The params pointer is valid for nparams elements as guaranteed by the caller.
        unsafe { NSIEvaluate(ctx, nparams, params) };
    }

    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        // SAFETY: NSIRenderControl is an external C function from the NSI library.
        // The params pointer is valid for nparams elements as guaranteed by the caller.
        unsafe { NSIRenderControl(ctx, nparams, params) };
    }

    #[cfg(feature = "output")]
    #[inline]
    fn DspyRegisterDriver(
        &self,
        driver_name: *const c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError {
        // SAFETY: DspyRegisterDriver is an external C function from the NSI library.
        // The driver_name is a valid C string and all function pointers are either
        // Some(valid_function) or None as guaranteed by the type system.
        unsafe {
            ndspy_sys::DspyRegisterDriver(
                driver_name,
                p_open,
                p_write,
                p_close,
                p_query,
            )
        }
    }
}
