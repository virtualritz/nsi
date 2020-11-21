use crate::Api;
extern crate dlopen;
use dlopen::wrapper::{Container, WrapperApi};
use std::{env, path::Path};

pub type ApiImpl = DynamicApi;

use crate::*;

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

    #[cfg(feature = "output")]
    DspyRegisterDriver: extern "C" fn(
        driver_name: *const ::std::os::raw::c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError,
}

pub struct DynamicApi {
    api: Container<CApi>,
}

#[cfg(target_os = "linux")]
static DELIGHT_APP_PATH: &str = "/usr/local/3delight/lib/lib3delight.so";

#[cfg(target_os = "macos")]
static DELIGHT_APP_PATH: &str = "/Applications/3Delight/lib/lib3delight.dylib";

#[cfg(target_os = "windows")]
static DELIGHT_APP_PATH: &str = "C:/%ProgramFiles%/3Delight/lib/lib3delight.dll";

#[cfg(target_os = "linux")]
static DELIGHT_LIB: &str = "lib3delight.so";

#[cfg(target_os = "macos")]
static DELIGHT_LIB: &str = "lib3delight.dylib";

#[cfg(target_os = "windows")]
static DELIGHT_LIB: &str = "lib3delight.dll";

impl DynamicApi {
    #[inline]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        match unsafe { Container::load(DELIGHT_APP_PATH) }
            .or_else(|_| unsafe { Container::load(DELIGHT_LIB) })
            .or_else(|_| match env::var("DELIGHT") {
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
                Ok(delight) => {
                    unsafe { Container::load(Path::new(&delight).join("lib").join(DELIGHT_LIB)) }
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
                }
            }) {
            Err(e) => Err(e),
            Ok(api) => {
                let api = DynamicApi { api };

                #[cfg(feature = "output")]
                api.DspyRegisterDriver(
                    b"ferris\0" as *const u8 as _,
                    Some(output::image_open),
                    Some(output::image_write),
                    Some(output::image_close),
                    Some(output::image_query),
                );

                Ok(api)
            }
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
        self.api
            .DspyRegisterDriver(driver_name, p_open, p_write, p_close, p_query)
    }
}
