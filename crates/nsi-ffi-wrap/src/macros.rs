//! Macros for defining NSI renderer implementations.
//!
//! This module provides the [`define_nsi_renderer!`] macro which generates
//! all the boilerplate code needed to wrap an NSI C API implementation.

/// Generates a complete NSI renderer implementation.
///
/// This macro generates:
/// - A `DynamicApi` struct for runtime library loading
/// - A `LinkedApi` struct for compile-time linking
/// - Feature-gated selection between them
/// - A global `NSI_API` static for the initialized API
/// - Re-exports of common types
///
/// # Example
///
/// ```ignore
/// nsi_ffi_wrap::define_nsi_renderer! {
///     name: Delight3,
///     dynamic: {
///         linux: "lib3delight.so",
///         macos: "lib3delight.dylib",
///         windows: "3Delight.dll",
///     },
///     env_var: "DELIGHT",
///     link_feature: "link_lib3delight",
/// }
/// ```
///
/// This generates a renderer that:
/// - Loads `lib3delight.so` on Linux, `lib3delight.dylib` on macOS, `3Delight.dll` on Windows
/// - Falls back to searching the `$DELIGHT/lib` (or `$DELIGHT/bin` on Windows) directory
/// - Uses static linking when the `link_lib3delight` feature is enabled
#[macro_export]
macro_rules! define_nsi_renderer {
    (
        name: $name:ident,
        dynamic: {
            linux: $linux_lib:literal,
            macos: $macos_lib:literal,
            windows: $windows_lib:literal,
        },
        env_var: $env_var:literal,
        link_feature: $link_feature:literal $(,)?
    ) => {
        // Re-export common types from nsi-ffi-wrap
        pub use $crate::{
            FfiApi,
            argument::*,
            context::*,
            node::*,
            nsi_trait::{Action, NodeType, Nsi, NsiExt},
        };

        #[cfg(feature = "output")]
        pub use $crate::output;

        // Platform-specific library paths
        #[cfg(target_os = "linux")]
        static LIB_NAME: &str = $linux_lib;

        #[cfg(target_os = "macos")]
        static LIB_NAME: &str = $macos_lib;

        #[cfg(target_os = "windows")]
        static LIB_NAME: &str = $windows_lib;

        // Default installation paths
        #[cfg(target_os = "linux")]
        static DEFAULT_LIB_PATH: &str =
            concat!("/usr/local/3delight/lib/", $linux_lib);

        #[cfg(target_os = "macos")]
        static DEFAULT_LIB_PATH: &str =
            concat!("/Applications/3Delight/lib/", $macos_lib);

        #[cfg(target_os = "windows")]
        static DEFAULT_LIB_PATH: &str =
            concat!("C:/%ProgramFiles%/3Delight/bin/", $windows_lib);

        // Environment variable for custom installation path
        static ENV_VAR: &str = $env_var;

        /// Dynamic API implementation using dlopen2.
        #[cfg(not(feature = $link_feature))]
        pub mod dynamic {
            use super::*;
            use nsi_sys::*;
            use std::{env, path::Path};
            use $crate::dlopen2::wrapper::{Container, WrapperApi};

            #[derive(WrapperApi)]
            struct FfiApiWrapper {
                NSIBegin: extern "C" fn(
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) -> NSIContext,
                NSIEnd: extern "C" fn(ctx: NSIContext),
                NSICreate: extern "C" fn(
                    ctx: NSIContext,
                    handle: NSIHandle,
                    type_: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSIDelete: extern "C" fn(
                    ctx: NSIContext,
                    handle: NSIHandle,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSISetAttribute: extern "C" fn(
                    ctx: NSIContext,
                    object: NSIHandle,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSISetAttributeAtTime: extern "C" fn(
                    ctx: NSIContext,
                    object: NSIHandle,
                    time: f64,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSIDeleteAttribute: extern "C" fn(
                    ctx: NSIContext,
                    object: NSIHandle,
                    name: *const std::ffi::c_char,
                ),
                #[allow(clippy::too_many_arguments)]
                NSIConnect: extern "C" fn(
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSIDisconnect: extern "C" fn(
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                ),
                NSIEvaluate: extern "C" fn(
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                NSIRenderControl: extern "C" fn(
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ),
                #[cfg(feature = "output")]
                DspyRegisterDriver: extern "C" fn(
                    driver_name: *const std::ffi::c_char,
                    p_open: ndspy_sys::PtDspyOpenFuncPtr,
                    p_write: ndspy_sys::PtDspyWriteFuncPtr,
                    p_close: ndspy_sys::PtDspyCloseFuncPtr,
                    p_query: ndspy_sys::PtDspyQueryFuncPtr,
                )
                    -> ndspy_sys::PtDspyError,
            }

            pub struct DynamicApi {
                api: Container<FfiApiWrapper>,
            }

            impl DynamicApi {
                pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
                    // Try loading in order:
                    // 1. Default installation path
                    // 2. Library name only (system paths)
                    // 3. Environment variable path
                    match unsafe { Container::load(DEFAULT_LIB_PATH) }
                        .or_else(|_| unsafe { Container::load(LIB_NAME) })
                        .or_else(|_| match env::var(ENV_VAR) {
                            Err(e) => Err(Box::new(e) as _),
                            Ok(base_path) => {
                                #[cfg(any(
                                    target_os = "linux",
                                    target_os = "macos"
                                ))]
                                let path = Path::new(&base_path)
                                    .join("lib")
                                    .join(LIB_NAME);
                                #[cfg(target_os = "windows")]
                                let path = Path::new(&base_path)
                                    .join("bin")
                                    .join(LIB_NAME);

                                unsafe { Container::load(path) }
                                    .map_err(|e| Box::new(e) as _)
                            }
                        }) {
                        Err(e) => Err(e),
                        Ok(api) => {
                            let api = DynamicApi { api };

                            #[cfg(feature = "output")]
                            {
                                $crate::register_output_drivers(&api);
                            }

                            Ok(api)
                        }
                    }
                }
            }

            impl $crate::FfiApi for DynamicApi {
                #[inline]
                fn NSIBegin(
                    &self,
                    nparams: std::os::raw::c_int,
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
                    type_: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSICreate(ctx, handle, type_, nparams, params);
                }

                #[inline]
                fn NSIDelete(
                    &self,
                    ctx: NSIContext,
                    handle: NSIHandle,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSIDelete(ctx, handle, nparams, params);
                }

                #[inline]
                fn NSISetAttribute(
                    &self,
                    ctx: NSIContext,
                    object: NSIHandle,
                    nparams: std::os::raw::c_int,
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
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSISetAttributeAtTime(
                        ctx, object, time, nparams, params,
                    );
                }

                #[inline]
                fn NSIDeleteAttribute(
                    &self,
                    ctx: NSIContext,
                    object: NSIHandle,
                    name: *const std::ffi::c_char,
                ) {
                    self.api.NSIDeleteAttribute(ctx, object, name);
                }

                #[inline]
                fn NSIConnect(
                    &self,
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSIConnect(
                        ctx, from, from_attr, to, to_attr, nparams, params,
                    );
                }

                #[inline]
                fn NSIDisconnect(
                    &self,
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                ) {
                    self.api.NSIDisconnect(ctx, from, from_attr, to, to_attr);
                }

                #[inline]
                fn NSIEvaluate(
                    &self,
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSIEvaluate(ctx, nparams, params);
                }

                #[inline]
                fn NSIRenderControl(
                    &self,
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    self.api.NSIRenderControl(ctx, nparams, params);
                }

                #[cfg(feature = "output")]
                #[inline]
                fn DspyRegisterDriver(
                    &self,
                    driver_name: *const std::ffi::c_char,
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

            pub type ApiImpl = DynamicApi;
        }

        /// Linked API implementation using compile-time linking.
        #[cfg(feature = $link_feature)]
        pub mod linked {
            use super::*;
            use nsi_sys::*;

            #[derive(Debug)]
            pub struct LinkedApi {}

            impl LinkedApi {
                pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
                    let api = LinkedApi {};

                    #[cfg(feature = "output")]
                    {
                        $crate::register_output_drivers(&api);
                    }

                    Ok(api)
                }
            }

            impl $crate::FfiApi for LinkedApi {
                #[inline]
                fn NSIBegin(
                    &self,
                    nparams: std::os::raw::c_int,
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
                    type_: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe { NSICreate(ctx, handle, type_, nparams, params) };
                }

                #[inline]
                fn NSIDelete(
                    &self,
                    ctx: NSIContext,
                    handle: NSIHandle,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe { NSIDelete(ctx, handle, nparams, params) };
                }

                #[inline]
                fn NSISetAttribute(
                    &self,
                    ctx: NSIContext,
                    object: NSIHandle,
                    nparams: std::os::raw::c_int,
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
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe {
                        NSISetAttributeAtTime(
                            ctx, object, time, nparams, params,
                        )
                    };
                }

                #[inline]
                fn NSIDeleteAttribute(
                    &self,
                    ctx: NSIContext,
                    object: NSIHandle,
                    name: *const std::ffi::c_char,
                ) {
                    unsafe { NSIDeleteAttribute(ctx, object, name) };
                }

                #[inline]
                fn NSIConnect(
                    &self,
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe {
                        NSIConnect(
                            ctx, from, from_attr, to, to_attr, nparams, params,
                        )
                    };
                }

                #[inline]
                fn NSIDisconnect(
                    &self,
                    ctx: NSIContext,
                    from: NSIHandle,
                    from_attr: *const std::ffi::c_char,
                    to: NSIHandle,
                    to_attr: *const std::ffi::c_char,
                ) {
                    unsafe { NSIDisconnect(ctx, from, from_attr, to, to_attr) };
                }

                #[inline]
                fn NSIEvaluate(
                    &self,
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe { NSIEvaluate(ctx, nparams, params) };
                }

                #[inline]
                fn NSIRenderControl(
                    &self,
                    ctx: NSIContext,
                    nparams: std::os::raw::c_int,
                    params: *const NSIParam,
                ) {
                    unsafe { NSIRenderControl(ctx, nparams, params) };
                }

                #[cfg(feature = "output")]
                #[inline]
                fn DspyRegisterDriver(
                    &self,
                    driver_name: *const std::ffi::c_char,
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

            pub type ApiImpl = LinkedApi;
        }

        // Select the appropriate API module
        #[cfg(not(feature = $link_feature))]
        use dynamic as api_impl;
        #[cfg(feature = $link_feature)]
        use linked as api_impl;

        /// The API implementation type for this renderer.
        pub type ApiImpl = api_impl::ApiImpl;

        // Global API instance
        $crate::lazy_static::lazy_static! {
            /// Global NSI API instance.
            pub static ref NSI_API: ApiImpl =
                ApiImpl::new().expect(concat!("Could not load ", $linux_lib));
        }
    };
}
