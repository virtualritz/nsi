use crate::Api;
extern crate dlopen;
use dlopen::wrapper::{Container, WrapperApi};
use std::{env, path::Path};

pub type ApiImpl = DynamicApi;

use nsi_sys::*;

#[derive(WrapperApi)]
struct CApi {
    NSIBegin:
        extern "C" fn(nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t,
    NSIEnd: extern "C" fn(ctx: NSIContext_t),
    NSICreate: extern "C" fn(
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ),
    NSIDelete: extern "C" fn(
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ),
    NSISetAttribute: extern "C" fn(
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ),
    NSISetAttributeAtTime: extern "C" fn(
        ctx: NSIContext_t,
        object: NSIHandle_t,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ),
    NSIDeleteAttribute:
        extern "C" fn(ctx: NSIContext_t, object: NSIHandle_t, name: *const ::std::os::raw::c_char),
    NSIConnect: extern "C" fn(
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ),
    NSIDisconnect: extern "C" fn(
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
    ),
    NSIEvaluate:
        extern "C" fn(ctx: NSIContext_t, nparams: ::std::os::raw::c_int, params: *const NSIParam_t),
    NSIRenderControl:
        extern "C" fn(ctx: NSIContext_t, nparams: ::std::os::raw::c_int, params: *const NSIParam_t),
}

pub struct DynamicApi {
    api: Container<CApi>,
}

impl DynamicApi {
    // macOS implementation
    #[inline]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        match unsafe { Container::load("/Applications/3Delight/lib/lib3delight.dylib") }
            .or_else(|_| unsafe { Container::load("lib3delight.dylib") })
            .or_else(|_| match env::var("DELIGHT") {
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
                Ok(delight) => unsafe {
                    Container::load(Path::new(&delight).join("lib").join("lib3delight.dylib"))
                }
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            }) {
            Err(e) => Err(e),
            Ok(api) => Ok(DynamicApi { api }),
        }
    }
}

impl Api for DynamicApi {
    #[inline]
    fn NSIBegin(&self, nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t {
        self.api.NSIBegin(nparams, params)
    }
    #[inline]
    fn NSIEnd(&self, ctx: NSIContext_t) {
        self.api.NSIEnd(ctx);
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
        self.api.NSICreate(ctx, handle, type_, nparams, params);
    }
    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        self.api.NSIDelete(ctx, handle, nparams, params);
    }
    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        self.api.NSISetAttribute(ctx, object, nparams, params);
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
        self.api
            .NSISetAttributeAtTime(ctx, object, time, nparams, params);
    }
    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        name: *const ::std::os::raw::c_char,
    ) {
        self.api.NSIDeleteAttribute(ctx, object, name);
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
        self.api
            .NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params);
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
        self.api.NSIDisconnect(ctx, from, from_attr, to, to_attr);
    }
    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        self.api.NSIEvaluate(ctx, nparams, params);
    }
    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    ) {
        self.api.NSIRenderControl(ctx, nparams, params);
    }
}
