//! C API function generation for [`Nsi`] trait implementations.
//!
//! This module provides the infrastructure for generating C-compatible
//! `extern "C"` functions from an [`Nsi`] implementation.
//!
//! # Usage
//!
//! A renderer implementing the [`Nsi`] trait can use this module to expose
//! a C API. The typical pattern is:
//!
//! 1. Implement [`Nsi`] for your renderer type
//! 2. Create a [`FfiApiAdapter`] wrapping your renderer
//! 3. Store the adapter in a `static` or use the provided macros
//! 4. Implement the `extern "C"` functions delegating to the adapter
//!
//! # Example
//!
//! ```ignore
//! use nsi_ffi_wrap::{FfiApiAdapter, Nsi, nsi_trait::Action};
//! use std::sync::OnceLock;
//!
//! // Your renderer implementation
//! struct MyRenderer { /* ... */ }
//! impl Nsi for MyRenderer { /* ... */ }
//!
//! // Global adapter instance
//! static ADAPTER: OnceLock<FfiApiAdapter<MyRenderer>> = OnceLock::new();
//!
//! fn init_adapter() -> &'static FfiApiAdapter<MyRenderer> {
//!     ADAPTER.get_or_init(|| FfiApiAdapter::new(MyRenderer::new()))
//! }
//!
//! // C API functions
//! #[unsafe(no_mangle)]
//! pub extern "C" fn NSIBegin(
//!     nparams: std::ffi::c_int,
//!     params: *const nsi_sys::NSIParam,
//! ) -> std::ffi::c_int {
//!     let args = unsafe { marshal_params(nparams, params) };
//!     init_adapter().begin(args.as_deref())
//! }
//! ```

use crate::{
    Arg, ArgData, Double, Float, Integer, String as NsiString,
    nsi_trait::{Action, NodeType},
};
use nsi_sys::NSIParam;
use std::ffi::{CStr, c_char, c_int};

/// Convert C API parameters to Rust [`Arg`] slice.
///
/// # Safety
///
/// The caller must ensure:
/// - `params` is valid for `nparams` elements (or null if nparams is 0)
/// - The parameter data pointers are valid for the duration of the call
/// - String data is valid UTF-8 or at least valid C strings
pub unsafe fn marshal_params_to_args<'a>(
    nparams: c_int,
    params: *const NSIParam,
) -> Option<Vec<Arg<'a, 'a>>> {
    if nparams <= 0 || params.is_null() {
        return None;
    }

    // SAFETY: Caller guarantees params is valid for nparams elements
    let params_slice =
        unsafe { std::slice::from_raw_parts(params, nparams as usize) };
    let mut args = Vec::with_capacity(nparams as usize);

    for param in params_slice {
        // SAFETY: Each param in the slice is valid per caller's guarantee
        if let Some(arg) = unsafe { marshal_single_param(param) } {
            args.push(arg);
        }
    }

    if args.is_empty() { None } else { Some(args) }
}

/// Convert a single C API parameter to a Rust [`Arg`].
///
/// # Safety
///
/// Same requirements as [`marshal_params_to_args`].
unsafe fn marshal_single_param<'a>(param: &NSIParam) -> Option<Arg<'a, 'a>> {
    if param.name.is_null() || param.data.is_null() {
        return None;
    }

    // SAFETY: Caller guarantees param.name is a valid C string
    let name = unsafe { CStr::from_ptr(param.name) }.to_str().ok()?;

    // Convert based on type
    // This is a simplified version - full implementation would handle all types
    let arg_data = match param.type_ {
        t if t == nsi_sys::NSIType::Float as i32 => {
            // SAFETY: Caller guarantees param.data points to valid f32
            let value = unsafe { *(param.data as *const f32) };
            ArgData::from(Float::new(value))
        }
        t if t == nsi_sys::NSIType::Double as i32 => {
            // SAFETY: Caller guarantees param.data points to valid f64
            let value = unsafe { *(param.data as *const f64) };
            ArgData::from(Double::new(value))
        }
        t if t == nsi_sys::NSIType::Integer as i32 => {
            // SAFETY: Caller guarantees param.data points to valid i32
            let value = unsafe { *(param.data as *const i32) };
            ArgData::from(Integer::new(value))
        }
        t if t == nsi_sys::NSIType::String as i32 => {
            // SAFETY: Caller guarantees param.data points to valid string pointer
            let ptr = unsafe { *(param.data as *const *const c_char) };
            if ptr.is_null() {
                return None;
            }
            // SAFETY: Caller guarantees the string pointer is valid
            let s = unsafe { CStr::from_ptr(ptr) }.to_str().ok()?;
            ArgData::from(NsiString::new(s))
        }
        // Add other types as needed...
        _ => return None,
    };

    Some(Arg::new(name, arg_data))
}

/// Parse a node type string to [`NodeType`].
///
/// # Safety
///
/// `type_str` must be a valid, null-terminated C string.
pub unsafe fn parse_node_type(type_str: *const c_char) -> Option<NodeType> {
    if type_str.is_null() {
        return None;
    }
    // SAFETY: Caller guarantees type_str is a valid C string
    let s = unsafe { CStr::from_ptr(type_str) }.to_str().ok()?;
    NodeType::from_name(s)
}

/// Parse an action string to [`Action`].
///
/// # Safety
///
/// `action_str` must be a valid, null-terminated C string.
pub unsafe fn parse_action(action_str: *const c_char) -> Option<Action> {
    if action_str.is_null() {
        return None;
    }
    // SAFETY: Caller guarantees action_str is a valid C string
    let s = unsafe { CStr::from_ptr(action_str) }.to_str().ok()?;
    Action::from_name(s)
}

/// Extract the "action" parameter from a parameter list.
///
/// This is used by `NSIRenderControl` which takes the action as a parameter.
///
/// # Safety
///
/// Same requirements as [`marshal_params_to_args`].
pub unsafe fn extract_action_from_params(
    nparams: c_int,
    params: *const NSIParam,
) -> Option<Action> {
    if nparams <= 0 || params.is_null() {
        return None;
    }

    // SAFETY: Caller guarantees params is valid for nparams elements
    let params_slice =
        unsafe { std::slice::from_raw_parts(params, nparams as usize) };

    for param in params_slice {
        if param.name.is_null() {
            continue;
        }

        // SAFETY: We checked param.name is not null
        let name = match unsafe { CStr::from_ptr(param.name) }.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        if name == "action" && param.type_ == nsi_sys::NSIType::String as i32 {
            // SAFETY: Caller guarantees param.data is valid for String type
            let ptr = unsafe { *(param.data as *const *const c_char) };
            if !ptr.is_null() {
                // SAFETY: We checked ptr is not null
                if let Ok(s) = unsafe { CStr::from_ptr(ptr) }.to_str() {
                    return Action::from_name(s);
                }
            }
        }
    }

    None
}

/// Convert a C string handle to a Rust string slice.
///
/// # Safety
///
/// `handle` must be a valid, null-terminated C string.
pub unsafe fn handle_to_str<'a>(handle: *const c_char) -> Option<&'a str> {
    if handle.is_null() {
        return None;
    }
    // SAFETY: Caller guarantees handle is a valid C string
    unsafe { CStr::from_ptr(handle) }.to_str().ok()
}

/// Macro for defining the complete C API for an [`Nsi`] implementation.
///
/// This macro generates all the `extern "C"` functions required for a
/// complete NSI C API implementation.
///
/// # Usage
///
/// ```ignore
/// use nsi_ffi_wrap::{define_nsi_c_api, FfiApiAdapter, Nsi};
///
/// struct MyRenderer { /* ... */ }
/// impl Nsi for MyRenderer { /* ... */ }
///
/// define_nsi_c_api!(MyRenderer, || MyRenderer::new());
/// ```
#[macro_export]
macro_rules! define_nsi_c_api {
    ($renderer_type:ty, $init:expr) => {
        use std::sync::OnceLock;

        static __NSI_ADAPTER: OnceLock<$crate::FfiApiAdapter<$renderer_type>> =
            OnceLock::new();

        fn __nsi_adapter() -> &'static $crate::FfiApiAdapter<$renderer_type> {
            __NSI_ADAPTER.get_or_init(|| $crate::FfiApiAdapter::new($init))
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIBegin(
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) -> ::std::ffi::c_int {
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };
            __nsi_adapter().begin(args.as_deref())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIEnd(ctx: ::std::ffi::c_int) {
            __nsi_adapter().end(ctx);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSICreate(
            ctx: ::std::ffi::c_int,
            handle: *const ::std::ffi::c_char,
            type_: *const ::std::ffi::c_char,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let handle_str = unsafe { $crate::c_api::handle_to_str(handle) };
            let node_type = unsafe { $crate::c_api::parse_node_type(type_) };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let (Some(h), Some(t)) = (handle_str, node_type) {
                __nsi_adapter().create(ctx, h, t, args.as_deref());
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIDelete(
            ctx: ::std::ffi::c_int,
            handle: *const ::std::ffi::c_char,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let handle_str = unsafe { $crate::c_api::handle_to_str(handle) };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let Some(h) = handle_str {
                __nsi_adapter().delete(ctx, h, args.as_deref());
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSISetAttribute(
            ctx: ::std::ffi::c_int,
            object: *const ::std::ffi::c_char,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let handle_str = unsafe { $crate::c_api::handle_to_str(object) };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let (Some(h), Some(a)) = (handle_str, args.as_deref()) {
                __nsi_adapter().set_attribute(ctx, h, a);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSISetAttributeAtTime(
            ctx: ::std::ffi::c_int,
            object: *const ::std::ffi::c_char,
            time: f64,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let handle_str = unsafe { $crate::c_api::handle_to_str(object) };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let (Some(h), Some(a)) = (handle_str, args.as_deref()) {
                __nsi_adapter().set_attribute_at_time(ctx, h, time, a);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIDeleteAttribute(
            ctx: ::std::ffi::c_int,
            object: *const ::std::ffi::c_char,
            name: *const ::std::ffi::c_char,
        ) {
            let handle_str = unsafe { $crate::c_api::handle_to_str(object) };
            let name_str = unsafe { $crate::c_api::handle_to_str(name) };

            if let (Some(h), Some(n)) = (handle_str, name_str) {
                __nsi_adapter().delete_attribute(ctx, h, n);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIConnect(
            ctx: ::std::ffi::c_int,
            from: *const ::std::ffi::c_char,
            from_attr: *const ::std::ffi::c_char,
            to: *const ::std::ffi::c_char,
            to_attr: *const ::std::ffi::c_char,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let from_str = unsafe { $crate::c_api::handle_to_str(from) };
            let from_attr_str =
                unsafe { $crate::c_api::handle_to_str(from_attr) };
            let to_str = unsafe { $crate::c_api::handle_to_str(to) };
            let to_attr_str = unsafe { $crate::c_api::handle_to_str(to_attr) };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let (Some(f), Some(t), Some(ta)) =
                (from_str, to_str, to_attr_str)
            {
                __nsi_adapter().connect(
                    ctx,
                    f,
                    from_attr_str,
                    t,
                    ta,
                    args.as_deref(),
                );
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIDisconnect(
            ctx: ::std::ffi::c_int,
            from: *const ::std::ffi::c_char,
            from_attr: *const ::std::ffi::c_char,
            to: *const ::std::ffi::c_char,
            to_attr: *const ::std::ffi::c_char,
        ) {
            let from_str = unsafe { $crate::c_api::handle_to_str(from) };
            let from_attr_str =
                unsafe { $crate::c_api::handle_to_str(from_attr) };
            let to_str = unsafe { $crate::c_api::handle_to_str(to) };
            let to_attr_str = unsafe { $crate::c_api::handle_to_str(to_attr) };

            if let (Some(f), Some(t), Some(ta)) =
                (from_str, to_str, to_attr_str)
            {
                __nsi_adapter().disconnect(ctx, f, from_attr_str, t, ta);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIEvaluate(
            ctx: ::std::ffi::c_int,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };
            __nsi_adapter().evaluate(ctx, args.as_deref());
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn NSIRenderControl(
            ctx: ::std::ffi::c_int,
            nparams: ::std::ffi::c_int,
            params: *const ::nsi_sys::NSIParam,
        ) {
            let action = unsafe {
                $crate::c_api::extract_action_from_params(nparams, params)
            };
            let args = unsafe {
                $crate::c_api::marshal_params_to_args(nparams, params)
            };

            if let Some(a) = action {
                __nsi_adapter().render_control(ctx, a, args.as_deref());
            }
        }
    };
}
