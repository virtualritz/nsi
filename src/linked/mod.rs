use crate::Api;
use nsi_sys::*;

pub type ApiImpl = LinkedApi;

#[derive(Debug)]
pub struct LinkedApi {}

impl LinkedApi {
    #[inline]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(LinkedApi {})
    }
}

impl Api for LinkedApi {
    #[inline]
    fn NSIBegin(&self, nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t {
        unsafe { NSIBegin(nparams, params) }
    }
    #[inline]
    fn NSIEnd(&self, ctx: NSIContext_t) {
        unsafe { NSIEnd(ctx) };
    }
    #[inline]
    fn NSICreate(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSICreate(ctx, handle, type_, nparams, params) };
    }
    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSIDelete(ctx, handle, nparams, params) };
    }
    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSISetAttribute(ctx, object, nparams, params) };
    }
    #[inline]
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSISetAttributeAtTime(ctx, object, time, nparams, params) };
    }
    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        name: *const ::std::os::raw::c_char,
    ) {
        unsafe { NSIDeleteAttribute(ctx, object, name) };
    }
    #[inline]
    fn NSIConnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params) };
    }
    #[inline]
    fn NSIDisconnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
    ) {
        unsafe { NSIDisconnect(ctx, from, from_attr, to, to_attr) };
    }
    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSIEvaluate(ctx, nparams, params) };
    }
    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        unsafe { NSIRenderControl(ctx, nparams, params) };
    }
}
