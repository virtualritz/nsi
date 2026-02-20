use crate::{FfiApi, *};
use dlopen2::wrapper::{Container, WrapperApi};
use std::{env, error::Error, ffi::c_char, os::raw::c_int, path::Path};

pub type ApiImpl = DynamicApi;

#[derive(WrapperApi)]
struct NsiCApi {
    NSIBegin:
        extern "C" fn(nparams: c_int, params: *const NSIParam) -> NSIContext,
    NSIEnd: extern "C" fn(ctx: NSIContext),
    NSICreate: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDelete: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSISetAttribute: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSISetAttributeAtTime: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDeleteAttribute:
        extern "C" fn(ctx: NSIContext, object: NSIHandle, name: *const c_char),
    #[allow(clippy::too_many_arguments)]
    NSIConnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDisconnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attr: *const c_char,
        to: NSIHandle,
        to_attr: *const c_char,
    ),
    NSIEvaluate:
        extern "C" fn(ctx: NSIContext, nparams: c_int, params: *const NSIParam),
    NSIRenderControl:
        extern "C" fn(ctx: NSIContext, nparams: c_int, params: *const NSIParam),
    #[cfg(feature = "output")]
    DspyRegisterDriver: extern "C" fn(
        driver_name: *const c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError,
}

pub struct DynamicApi {
    api: Container<NsiCApi>,
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
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // SAFETY: Container::load calls dlopen which is safe to call with any string.
        // The paths are hardcoded constants and the library handles invalid paths gracefully.
        match unsafe { Container::load(DELIGHT_APP_PATH) }
            .or_else(|_| unsafe { Container::load(DELIGHT_LIB) })
            .or_else(|_| match env::var("DELIGHT") {
                Err(e) => Err(Box::new(e) as _),
                Ok(delight) => {
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    let path =
                        Path::new(&delight).join("lib").join(DELIGHT_LIB);
                    #[cfg(target_os = "windows")]
                    let path =
                        Path::new(&delight).join("bin").join(DELIGHT_LIB);

                    // SAFETY: Container::load is safe to call with any path.
                    // The library handles invalid paths gracefully.
                    unsafe { Container::load(path) }
                        .map_err(|e| Box::new(e) as _)
                }
            }) {
            Err(e) => Err(e),
            Ok(api) => {
                let api = DynamicApi { api };

                #[cfg(feature = "output")]
                super::register_output_drivers(&api);

                Ok(api)
            }
        }
    }
}

impl TryFrom<&Path> for DynamicApi {
    type Error = dlopen2::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        // SAFETY: Container::load is safe to call with any path.
        // The library handles invalid paths gracefully.
        match unsafe { Container::load(path) } {
            Err(e) => Err(e),
            Ok(api) => {
                let api = DynamicApi { api };

                #[cfg(feature = "output")]
                super::register_output_drivers(&api);

                Ok(api)
            }
        }
    }
}

impl FfiApi for DynamicApi {
    #[inline]
    fn NSIBegin(&self, nparams: c_int, params: *const NSIParam) -> NSIContext {
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
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSICreate(ctx, handle, type_, nparams, params);
    }

    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIDelete(ctx, handle, nparams, params);
    }

    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
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
        nparams: c_int,
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
        name: *const c_char,
    ) {
        self.api.NSIDeleteAttribute(ctx, object, name);
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
        self.api
            .NSIConnect(ctx, from, from_attr, to, to_attr, nparams, params);
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
        self.api.NSIDisconnect(ctx, from, from_attr, to, to_attr);
    }

    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIEvaluate(ctx, nparams, params);
    }

    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIRenderControl(ctx, nparams, params);
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
        self.api.DspyRegisterDriver(
            driver_name,
            p_open,
            p_write,
            p_close,
            p_query,
        )
    }
}
