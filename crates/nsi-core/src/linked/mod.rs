use crate::Api;
use nsi_sys::*;

pub type ApiImpl = LinkedApi;

#[derive(Debug)]
pub struct LinkedApi {}

impl LinkedApi {
    #[inline]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let api = LinkedApi {};

        #[cfg(feature = "output")]
        api.DspyRegisterDriver(
            b"ferris\0" as *const u8 as _,
            Some(crate::output::image_open),
            Some(crate::output::image_write),
            Some(crate::output::image_close),
            Some(crate::output::image_query),
        );

        Ok(api)
    }
}

impl Api for LinkedApi {
    #[inline]
    fn NSIBegin(
        &self,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) -> NSIContext {
        unsafe { NSIBegin(nparams, params) }
    }

    #[inline]
    fn NSIEnd(&self, ctx: NSIContext) {
        unsafe { NSIEnd(ctx) };
    }

    #[inline]
    fn NSICreate(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSICreate(ctx, handle, type_, nparams, params) };
    }

    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSIDelete(ctx, handle, nparams, params) };
    }

    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSISetAttribute(ctx, object, nparams, params) };
    }

    #[inline]
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSISetAttributeAtTime(ctx, object, time, nparams, params) };
    }

    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const ::std::os::raw::c_char,
    ) {
        unsafe { NSIDeleteAttribute(ctx, object, name) };
    }

    #[inline]
    fn NSIConnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe {
            NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params)
        };
    }

    #[inline]
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
    ) {
        unsafe { NSIDisconnect(ctx, from, from_attr, to, to_attr) };
    }

    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSIEvaluate(ctx, nparams, params) };
    }

    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        unsafe { NSIRenderControl(ctx, nparams, params) };
    }

    #[cfg(feature = "output")]
    #[inline]
    fn DspyRegisterDriver(
        &self,
        driver_name: *const ::std::os::raw::c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError {
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
