use crate::Api;
use dlopen2::wrapper::{Container, WrapperApi};
use std::{env, path::Path};

pub type ApiImpl = DynamicApi;

use crate::*;

#[derive(WrapperApi)]
struct CApi {
    NSIBegin: extern "C" fn(
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) -> NSIContext,
    NSIEnd: extern "C" fn(ctx: NSIContext),
    NSICreate: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSIDelete: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSISetAttribute: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSISetAttributeAtTime: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSIDeleteAttribute: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        name: *const ::std::os::raw::c_char,
    ),
    NSIConnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSIDisconnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle,
        to_attr: *const ::std::os::raw::c_char,
    ),
    NSIEvaluate: extern "C" fn(
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
    NSIRenderControl: extern "C" fn(
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ),
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
static DELIGHT_APP_PATH: &str = "C:/%ProgramFiles%/3Delight/bin/3Delight.dll";

#[cfg(target_os = "linux")]
static DELIGHT_LIB: &str = "lib3delight.so";

#[cfg(target_os = "macos")]
static DELIGHT_LIB: &str = "lib3delight.dylib";

#[cfg(target_os = "windows")]
static DELIGHT_LIB: &str = "3Delight.dll";

impl DynamicApi {
    #[inline]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        match unsafe { Container::load(DELIGHT_APP_PATH) }
            .or_else(|_| unsafe { Container::load(DELIGHT_LIB) })
            .or_else(|_| match env::var("DELIGHT") {
                Err(e) => Err(Box::new(e) as _),
                Ok(delight) => unsafe {
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    let path =
                        Path::new(&delight).join("lib").join(DELIGHT_LIB);
                    #[cfg(target_os = "windows")]
                    let path =
                        Path::new(&delight).join("bin").join(DELIGHT_LIB);

                    Container::load(path)
                }
                .map_err(|e| Box::new(e) as _),
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
    fn NSIBegin(
        &self,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) -> NSIContext {
        self.api.NSIBegin(nparams, params)
    }

    #[inline]
    fn NSIEnd(&self, ctx: NSIContext) {
        self.api.NSIEnd(ctx);
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
        self.api.NSICreate(ctx, handle, type_, nparams, params);
    }

    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIDelete(ctx, handle, nparams, params);
    }

    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        self.api.NSISetAttribute(ctx, object, nparams, params);
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
        self.api
            .NSISetAttributeAtTime(ctx, object, time, nparams, params);
    }

    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const ::std::os::raw::c_char,
    ) {
        self.api.NSIDeleteAttribute(ctx, object, name);
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
        self.api
            .NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params);
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
        self.api.NSIDisconnect(ctx, from, from_attr, to, to_attr);
    }

    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIEvaluate(ctx, nparams, params);
    }

    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam,
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
        self.api.DspyRegisterDriver(
            driver_name,
            p_open,
            p_write,
            p_close,
            p_query,
        )
    }
}
