#![cfg_attr(feature = "nightly", doc(cfg(feature = "output")))]
//! Output driver callbacks.
//!
//! This module provides type-safe, generic callback support for streaming pixel
//! data during and after renders. Callbacks are generic over pixel type (`f32`,
//! `u16`, `u8`, etc.) with zero runtime cost via monomorphization.
//!
//! ## Callback Types
//!
//! * [`FnOpen`] -- Called once when the output driver is opened by the renderer.
//!
//! * [`FnWrite`] -- Called for each bucket of pixel data. Generic over pixel
//!   type `T: PixelType`. Receives only the bucket data, not the full image.
//!
//! * [`FnFinish`] -- Called once when the output driver is closed. Does NOT
//!   receive pixel data (matching the C ndspy API). Use [`AccumulatingCallbacks`]
//!   if you need the full accumulated image.
//!
//! ## Pixel Types
//!
//! The callbacks are generic over pixel type via the [`PixelType`] trait.
//! Use the corresponding typed driver constant:
//!
//! * [`FERRIS_F32`] -- 32-bit float pixels.
//! * [`FERRIS_U32`] / [`FERRIS_I32`] -- 32-bit integer pixels.
//! * [`FERRIS_U16`] / [`FERRIS_I16`] -- 16-bit integer pixels.
//! * [`FERRIS_U8`] / [`FERRIS_I8`] -- 8-bit integer pixels.
//!
//! ## Example: Streaming buckets
//!
//! ```ignore
//! use nsi_ffi_wrap as nsi;
//! use std::sync::{Arc, Mutex};
//!
//! let pixels = Arc::new(Mutex::new(Vec::<f32>::new()));
//! let pixels_write = Arc::clone(&pixels);
//!
//! // Write callback receives each bucket as it's rendered.
//! let write = nsi::output::WriteCallback::<f32>::new(
//!     move |_name, width, _height, x0, x1, y0, y1, fmt, bucket: &[f32]| {
//!         let mut buf = pixels_write.lock().unwrap();
//!         // Accumulate bucket into full image buffer...
//!         nsi::output::Error::None
//!     },
//! );
//!
//! ctx.set_attribute(
//!     "driver",
//!     &[
//!         nsi::string!("drivername", nsi::output::FERRIS_F32),
//!         nsi::string!("imagefilename", "render"),
//!         nsi::callback!("callback.write", write),
//!     ],
//! );
//! ```
//!
//! ## Example: Using AccumulatingCallbacks
//!
//! For the common case where you need the full accumulated image at the end,
//! use [`AccumulatingCallbacks`]:
//!
//! ```ignore
//! use nsi_ffi_wrap as nsi;
//!
//! let (write, finish) = nsi::output::AccumulatingCallbacks::<f32>::new(
//!     |name, width, height, fmt, pixels: Vec<f32>| {
//!         // Called once with the complete accumulated image.
//!         write_exr(&name, width, height, &pixels);
//!         nsi::output::Error::None
//!     },
//! );
//!
//! ctx.set_attribute(
//!     "driver",
//!     &[
//!         nsi::string!("drivername", nsi::output::FERRIS_F32),
//!         nsi::string!("imagefilename", "render"),
//!         nsi::callback!("callback.write", write),
//!         nsi::callback!("callback.finish", finish),
//!     ],
//! );
//! ```
//!
//! ## Color Profiles
//!
//! The pixel color data that the renderer generates is linear and
//! scene-referred. I.e. relative to whatever units you used to describe
//! illuminants in your scene.
//!
//! Using the
//! [`"colorprofile"` attribute](https://nsi.readthedocs.io/en/latest/nodes.html?highlight=outputlayer#the-outputlayer-node)
//! of an [`OutputLayer`](crate::OUTPUT_LAYER) you can ask the
//! renderer to apply an [Open Color IO](https://opencolorio.org/) (OCIO)
//! [profile/LUT](https://github.com/colour-science/OpenColorIO-Configs/tree/feature/aces-1.2-config/aces_1.2/luts)
//! before quantizing (see below).
//!
//! Once OCIO has a [Rust wrapper](https://crates.io/crates/opencolorio) you can easily choose to
//! do these color conversions yourself. In the meantime there is the
//! [`colorspace`](https://crates.io/crates/colorspace) crate which has some useful profiles built
//! in, e.g. [ACEScg](https://en.wikipedia.org/wiki/Academy_Color_Encoding_System#ACEScg).
//!
//! ```
//! # use nsi_ffi_wrap as nsi;
//! # let ctx = nsi::Context::new(None).unwrap();
//! ctx.create("beauty", nsi::OUTPUT_LAYER, None);
//! ctx.set_attribute(
//!     "beauty",
//!     &[
//!         // The Ci variable comes from Open Shading Language.
//!         nsi::string!("variablename", "Ci"),
//!         // We want the pixel data 'display-referred' in sRGB and quantized down to 0.0..255.0.
//!         nsi::string!("colorprofile", "/home/luts/linear_to_sRGB.spi1d"),
//!         nsi::string!("scalarformat", "uint8"),
//!     ],
//! );
//! ```
//!
//! ## Quantization
//!
//! Using the [`"scalarformat"`
//! attribute](https://nsi.readthedocs.io/en/latest/nodes.html?highlight=outputlayer#the-outputlayer-node)
//! of an [`OutputLayer`](crate::OUTPUT_LAYER) you can ask the
//! renderer to quantize data down to a suitable range. For example, setting
//! this to `"uint16"` will get you valid `u16` values from `0.0..65535.0`, but
//! stored in the `f32`s of the `pixel_data` buffer. The value of `1.0` will map
//! to `65535.0` and everything above will be clipped. You can convert
//! such a value straight via `f32 as u16`.
//!
//! Unless you asked the renderer to also apply some color profile (see above)
//! the data is linear. To look good on a screen it needs to be made
//! display-referred.
//!
//! See the `output` example on how to do this with a simple, display-referred
//! `sRGB` curve.
use crate::argument::CallbackPtr;
use std::{
    ffi::CStr,
    mem::size_of,
    os::raw::{c_char, c_int, c_void},
};

pub mod pixel_format;
pub use pixel_format::*;

pub mod pixel_type;
pub use pixel_type::*;

/// Driver name for f32 pixel type.
pub static FERRIS_F32: &str = "ferris_f32";
/// Driver name for u32 pixel type.
pub static FERRIS_U32: &str = "ferris_u32";
/// Driver name for i32 pixel type.
pub static FERRIS_I32: &str = "ferris_i32";
/// Driver name for u16 pixel type.
pub static FERRIS_U16: &str = "ferris_u16";
/// Driver name for i16 pixel type.
pub static FERRIS_I16: &str = "ferris_i16";
/// Driver name for u8 pixel type.
pub static FERRIS_U8: &str = "ferris_u8";
/// Driver name for i8 pixel type.
pub static FERRIS_I8: &str = "ferris_i8";

/// Legacy driver name - defaults to f32.
/// Deprecated: Use [`FERRIS_F32`], [`FERRIS_U16`], etc. for type-specific drivers.
#[deprecated(
    since = "0.9.0",
    note = "Use FERRIS_F32, FERRIS_U16, etc. for type-specific drivers"
)]
pub static FERRIS: &str = "ferris_f32";

/// An error type the callbacks return to communicate with the
/// renderer.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, num_enum::IntoPrimitive)]
pub enum Error {
    /// Everything is dandy.
    None = ndspy_sys::PtDspyError::None as _,
    /// We ran out of memory.
    NoMemory = ndspy_sys::PtDspyError::NoMemory as _,
    /// We do no support this request.
    Unsupported = ndspy_sys::PtDspyError::Unsupported as _,
    BadParameters = ndspy_sys::PtDspyError::BadParams as _,
    NoResource = ndspy_sys::PtDspyError::NoResource as _,
    /// Something else went wrong.
    Undefined = ndspy_sys::PtDspyError::Undefined as _,
    /// Stop the render.
    Stop = ndspy_sys::PtDspyError::Stop as _,
}

impl From<Error> for ndspy_sys::PtDspyError {
    fn from(item: Error) -> ndspy_sys::PtDspyError {
        match item {
            Error::None => ndspy_sys::PtDspyError::None,
            Error::NoMemory => ndspy_sys::PtDspyError::NoMemory,
            Error::Unsupported => ndspy_sys::PtDspyError::Unsupported,
            Error::BadParameters => ndspy_sys::PtDspyError::BadParams,
            Error::NoResource => ndspy_sys::PtDspyError::NoResource,
            Error::Undefined => ndspy_sys::PtDspyError::Undefined,
            Error::Stop => ndspy_sys::PtDspyError::Stop,
        }
    }
}

/// A closure which is called once per
/// [`OutputDriver`](crate::OUTPUT_DRIVER) instance.
///
/// It is passed to ɴsɪ via the `"callback.open"` attribute on that node.
///
/// The closure is called once, before the renderer starts sending pixels to the
/// output driver.
///
/// # Arguments
/// The `pixel_format` parameter is an array of strings that details the
/// composition of the `f32` data that the renderer will send to the [`FnWrite`]
/// and/or [`FnFinish`] closures.
///
/// # Example
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # use nsi_ffi_wrap as nsi;
/// # use nsi::output::PixelFormat;
/// # let ctx = nsi::Context::new(None).unwrap();
/// # ctx.create("display_driver", nsi::OUTPUT_DRIVER, None);
/// let open = nsi::output::OpenCallback::new(
///     |name: &str,
///      width: usize,
///      height: usize,
///      pixel_format: &nsi::output::PixelFormat| {
///         println!(
///             "Resolution: {}×{}\nPixel Format:\n{:?}",
///             width, height, pixel_format
///         );
///         nsi::output::Error::None
///     },
/// );
/// # }
/// ```
pub trait FnOpen<'a>: FnMut(
    // Filename.
    &str,
    // Width.
    usize,
    // Height.
    usize,
    // Pixel format.
    &PixelFormat,
) -> Error
+ 'a {}

#[doc(hidden)]
impl<'a, T: FnMut(&str, usize, usize, &PixelFormat) -> Error + 'a> FnOpen<'a>
    for T
{
}

// FIXME once trait aliases are in stable.
/*
trait FnOpen<'a> = FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // Pixel format.
        &PixelFormat,
    ) -> Error
    + 'a
*/

/// A closure which is called for each bucket of pixels the
/// [`OutputDriver`](crate::OUTPUT_DRIVER) instance sends
/// during rendering.
///
/// The closure receives ONLY the bucket data, not an accumulated image.
/// Bucket dimensions are: `(x_max_plus_one - x_min) x (y_max_plus_one - y_min)`.
/// Data layout is row-major with channels interleaved per pixel.
///
/// It is passed to ɴsɪ via the `"callback.write"` attribute on that node.
///
/// # Type Parameter
///
/// `T` is the pixel scalar type (e.g., `f32`, `u16`, `u8`). It must implement
/// [`PixelType`]. The driver name must match the type (e.g., `FERRIS_F32` for `f32`).
///
/// # Example
/// ```ignore
/// let write = nsi::output::WriteCallback::<f32>::new(
///     |name: &str,
///      width: usize,
///      height: usize,
///      x_min: usize,
///      x_max_plus_one: usize,
///      y_min: usize,
///      y_max_plus_one: usize,
///      pixel_format: &nsi::output::PixelFormat,
///      bucket_data: &[f32]| {
///         // bucket_data contains ONLY this bucket, not the full image
///         // Upload bucket to GPU texture, etc.
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "driver",
///     &[
///         nsi::string!("drivername", nsi::output::FERRIS_F32),
///         nsi::callback!("callback.write", write),
///     ],
/// );
/// ```
pub trait FnWrite<'a, T: PixelType>: FnMut(
        // Filename.
        &str,
        // Full image width.
        usize,
        // Full image height.
        usize,
        // Bucket x_min.
        usize,
        // Bucket x_max_plus_one.
        usize,
        // Bucket y_min.
        usize,
        // Bucket y_max_plus_one.
        usize,
        // Pixel format.
        &PixelFormat,
        // Bucket pixel data (NOT the full image!).
        &[T],
    ) -> Error
    + 'a {}

#[doc(hidden)]
impl<'a, T: PixelType, F> FnWrite<'a, T> for F where
    F: FnMut(
            &str,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            &PixelFormat,
            &[T],
        ) -> Error
        + 'a
{
}

/// A closure which is called once per
/// [`OutputDriver`](crate::OUTPUT_DRIVER) instance when rendering completes.
///
/// It is passed to ɴsɪ via the `"callback.finish"` attribute on that node.
///
/// **Note:** This closure does NOT receive pixel data. This matches the C ndspy
/// API behavior where the display driver is responsible for accumulating pixels
/// if needed. Use [`AccumulatingCallbacks`] if you need the complete image.
///
/// # Example
/// ```ignore
/// let finish = nsi::output::FinishCallback::new(
///     |name: String,
///      width: usize,
///      height: usize,
///      pixel_format: nsi::output::PixelFormat| {
///         println!("Rendering complete: {}x{}", width, height);
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "driver",
///     &[
///         nsi::string!("drivername", nsi::output::FERRIS_F32),
///         nsi::callback!("callback.finish", finish),
///     ],
/// );
/// ```
pub trait FnFinish<'a>: FnMut(
    // Filename.
    String,
    // Width.
    usize,
    // Height.
    usize,
    // Pixel format.
    PixelFormat,
) -> Error
+ 'a {}

#[doc(hidden)]
impl<'a, F> FnFinish<'a> for F where
    F: FnMut(String, usize, usize, PixelFormat) -> Error + 'a
{
}

enum Query {}

trait FnQuery<'a>: FnMut(Query) -> Error + 'a {}
impl<'a, T: FnMut(Query) -> Error + 'a> FnQuery<'a> for T {}

// FIXME once trait aliases are in stable.
/*
pub trait FnQuery<'a> = dyn FnMut(Query) -> Error + 'a;
*/

/// Wrapper to pass an [`FnOpen`] closure to an
/// [`OutputDriver`](crate::OUTPUT_DRIVER) node.
pub struct OpenCallback<'a>(Box<Box<Box<dyn FnOpen<'a>>>>);

// Why do we need a triple Box here? Why does a Box<Box<T>> not suffice?
// This is a known pattern for passing closures through FFI boundaries.
// The issue is related to fat pointers (trait objects):
// - Box<dyn Trait> is a fat pointer (16 bytes: data ptr + vtable ptr)
// - Box<Box<dyn Trait>> is a thin pointer (8 bytes) to the fat pointer
// - Box<Box<Box<dyn Trait>>> is a thin pointer to a thin pointer
//
// When casting through *const c_void, type information is lost.
// With double Box, reconstructing the fat pointer fails (segfault).
// Triple Box ensures we always deal with thin pointers at the FFI boundary.
impl<'a> OpenCallback<'a> {
    pub fn new<F>(fn_open: F) -> Self
    where
        F: FnOpen<'a>,
    {
        OpenCallback(Box::new(Box::new(Box::new(fn_open))))
    }
}

impl CallbackPtr for OpenCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}
/// Wrapper to pass an [`FnWrite`] closure to an
/// [`OutputDriver`](crate::OUTPUT_DRIVER) node.
///
/// # Type Parameter
///
/// `T` is the pixel scalar type. Use the matching driver name:
/// - `WriteCallback::<f32>` with `FERRIS_F32`
/// - `WriteCallback::<u16>` with `FERRIS_U16`
/// - etc.
pub struct WriteCallback<'a, T: PixelType>(Box<Box<Box<dyn FnWrite<'a, T>>>>);

impl<'a, T: PixelType> WriteCallback<'a, T> {
    pub fn new<F>(fn_write: F) -> Self
    where
        F: FnWrite<'a, T>,
    {
        WriteCallback(Box::new(Box::new(Box::new(fn_write))))
    }
}

impl<T: PixelType> CallbackPtr for WriteCallback<'_, T> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

/// Wrapper to pass an [`FnFinish`] closure to an
/// [`OutputDriver`](crate::OUTPUT_DRIVER) node.
///
/// **Note:** `FnFinish` does not receive pixel data. If you need the complete
/// accumulated image, use [`AccumulatingCallbacks`] instead.
pub struct FinishCallback<'a>(Box<Box<Box<dyn FnFinish<'a>>>>);

impl<'a> FinishCallback<'a> {
    pub fn new<F>(fn_finish: F) -> Self
    where
        F: FnFinish<'a>,
    {
        FinishCallback(Box::new(Box::new(Box::new(fn_finish))))
    }
}

impl CallbackPtr for FinishCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

/// Internal data structure passed through FFI as the image handle.
/// Generic over pixel type T for zero-cost type handling.
struct DisplayData<'a, T: PixelType> {
    name: String,
    width: usize,
    height: usize,
    pixel_format: PixelFormat,
    // NO pixel_data buffer - we pass buckets directly to callbacks
    fn_write: Option<Box<Box<Box<dyn FnWrite<'a, T>>>>>,
    fn_finish: Option<Box<Box<Box<dyn FnFinish<'a>>>>>,
    // FIXME: unused atm.
    fn_query: Option<Box<Box<Box<dyn FnQuery<'a>>>>>,
    // PhantomData to ensure T is used
    _phantom: std::marker::PhantomData<T>,
}

fn extract_callback<T: ?Sized>(
    name: &str,
    type_: u8,
    len: usize,
    parameters: &[ndspy_sys::UserParameter],
) -> Option<Box<Box<Box<T>>>> {
    for p in parameters.iter() {
        // SAFETY: Parameter names come from NSI API and should be valid C strings
        if p.name.is_null() {
            continue;
        }
        let p_name = match unsafe { CStr::from_ptr(p.name) }.to_str() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if name == p_name
            && type_ == p.valueType as _
            && len == p.valueCount as _
        {
            if !p.value.is_null() {
                // SAFETY: p.value was created by Box::into_raw in the callback's to_ptr method
                // The type cast is valid because we verified the parameter type matches
                return Some(unsafe {
                    Box::from_raw(p.value as *mut Box<Box<T>>)
                });
            } else {
                // Parameter exists but value is missing - exit quietly.
                break;
            }
        }
    }
    None
}

// Generic trampoline function for the FnOpen callback.
// Each instantiation (image_open::<f32>, image_open::<u16>, etc.) handles a specific pixel type.
pub(crate) extern "C" fn image_open<T: PixelType>(
    image_handle_ptr: *mut ndspy_sys::PtDspyImageHandle,
    _driver_name: *const c_char,
    output_filename: *const c_char,
    width: c_int,
    height: c_int,
    parameters_count: c_int,
    parameters: *const ndspy_sys::UserParameter,
    format_count: c_int,
    format: *mut ndspy_sys::PtDspyDevFormat,
    flag_stuff: *mut ndspy_sys::PtFlagStuff,
) -> ndspy_sys::PtDspyError {
    // Catch any panics to prevent unwinding into C code.
    match std::panic::catch_unwind(|| {
        if (image_handle_ptr.is_null())
            || (output_filename.is_null())
            || format.is_null()
            || (format_count <= 0)
            || ((parameters_count > 0) && parameters.is_null())
        {
            return Error::BadParameters.into();
        }

        // SAFETY: We only read from the parameters slice, never modify it.
        let parameters = unsafe {
            std::slice::from_raw_parts(parameters, parameters_count as _)
        };

        let mut display_data = Box::new(DisplayData::<T> {
            name: {
                // SAFETY: output_filename is checked for null above and comes from NSI C API
                let c_str = unsafe { CStr::from_ptr(output_filename) };
                c_str.to_string_lossy().into_owned()
            },
            width: width as _,
            height: height as _,
            pixel_format: PixelFormat::default(),
            // NO pixel_data allocation - we pass buckets directly
            fn_write: extract_callback::<dyn FnWrite<T>>(
                "callback.write",
                b'p',
                1,
                parameters,
            ),
            fn_finish: extract_callback::<dyn FnFinish>(
                "callback.finish",
                b'p',
                1,
                parameters,
            ),
            fn_query: None,
            _phantom: std::marker::PhantomData,
        });

        // SAFETY: format is a valid pointer to format_count elements from NSI C API.
        let format = unsafe {
            std::slice::from_raw_parts_mut(format, format_count as _)
        };

        // Set format to the requested pixel type T
        format.iter_mut().for_each(|f| f.type_ = T::NDSPY_TYPE);

        display_data.pixel_format = PixelFormat::new(format);

        let error = if let Some(mut fn_open) =
            extract_callback::<dyn FnOpen>("callback.open", b'p', 1, parameters)
        {
            let error = fn_open(
                &display_data.name,
                width as _,
                height as _,
                &display_data.pixel_format,
            );
            // wtf?
            Box::leak(fn_open);

            error
        } else {
            Error::None
        };

        // SAFETY: image_handle_ptr and flag_stuff are valid pointers from NSI C API
        unsafe {
            *image_handle_ptr = Box::into_raw(display_data) as _;
            // Preserve renderer-provided flags, but clear the empty-bucket request.
            (*flag_stuff).flags &=
                !(ndspy_sys::PkDspyFlagsWantsEmptyBuckets as i32);
        }

        error.into()
    }) {
        Ok(result) => result,
        Err(_) => {
            // If we panicked, return an error to the renderer
            Error::Undefined.into()
        }
    }
}

// FIXME: this will be used for a FnProgress callback later.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn image_query(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    query_type: ndspy_sys::PtDspyQueryType,
    data_len: c_int,
    data: *mut c_void,
) -> ndspy_sys::PtDspyError {
    // Catch any panics to prevent unwinding into C code.
    match std::panic::catch_unwind(|| {
        match query_type {
        ndspy_sys::PtDspyQueryType::RenderProgress => {
            if (data_len as usize)
                < core::mem::size_of::<ndspy_sys::PtDspyRenderProgressFuncPtr>()
                || data.is_null()
            {
                Error::BadParameters
            } else {
                // SAFETY: data is a valid pointer to PtDspyRenderProgressFuncPtr
                // as specified by the query type and validated by data_len check.
                unsafe {
                    let func_ptr = data as *mut ndspy_sys::PtDspyRenderProgressFuncPtr;
                    *func_ptr = Some(image_progress);
                }
                Error::None
            }
        }
        ndspy_sys::PtDspyQueryType::Progressive => {
            if (data_len as usize) < size_of::<ndspy_sys::PtDspyProgressiveInfo>()
                || data.is_null()
            {
                Error::BadParameters
            } else {
                // SAFETY: data points to PtDspyProgressiveInfo with validated length.
                unsafe {
                    let info = data as *mut ndspy_sys::PtDspyProgressiveInfo;
                    (*info).acceptProgressive = 1;
                }
                Error::None
            }
        }
        ndspy_sys::PtDspyQueryType::Thread => {
            if (data_len as usize) < size_of::<ndspy_sys::PtDspyThreadInfo>()
                || data.is_null()
            {
                Error::BadParameters
            } else {
                // SAFETY: data points to PtDspyThreadInfo with validated length.
                unsafe {
                    let info = data as *mut ndspy_sys::PtDspyThreadInfo;
                    // Allow multithreaded buckets.
                    (*info).multithread = 1;
                }
                Error::None
            }
        }
        ndspy_sys::PtDspyQueryType::Cooked => {
            if (data_len as usize) < size_of::<ndspy_sys::PtDspyCookedInfo>()
                || data.is_null()
            {
                Error::BadParameters
            } else {
                // SAFETY: data points to PtDspyCookedInfo with validated length.
                unsafe {
                    let info = data as *mut ndspy_sys::PtDspyCookedInfo;
                    // Accept filtered pixel data (cooked=1 = PkDspyCQDefault).
                    (*info).cooked = 1;
                }
                Error::None
            }
        }
        // StopQuery asks "should rendering stop?" - return None to continue.
        ndspy_sys::PtDspyQueryType::Stop => Error::None,
        ndspy_sys::PtDspyQueryType::Overwrite => {
            if (data_len as usize) < size_of::<ndspy_sys::PtDspyOverwriteInfo>()
                || data.is_null()
            {
                Error::BadParameters
            } else {
                // SAFETY: data points to PtDspyOverwriteInfo with validated length.
                unsafe {
                    let info = data as *mut ndspy_sys::PtDspyOverwriteInfo;
                    // Allow the renderer to overwrite existing files.
                    (*info).overwrite = 1;
                }
                Error::None
            }
        }
        _ => Error::Unsupported,
    }
    .into()
    }) {
        Ok(result) => result,
        Err(_) => {
            // If we panicked, return an error to the renderer
            Error::Undefined.into()
        }
    }
}

// Generic trampoline function for the FnWrite callback.
// Passes bucket data directly to callback - NO accumulation, NO memcpy to full buffer.
pub(crate) extern "C" fn image_write<T: PixelType>(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    _entry_size: c_int,
    pixel_data: *const u8,
) -> ndspy_sys::PtDspyError {
    // Catch any panics to prevent unwinding into C code
    match std::panic::catch_unwind(|| {
        // SAFETY: image_handle_ptr should be valid as it was created by image_open
        if image_handle_ptr.is_null() {
            return Error::BadParameters.into();
        }
        let display_data =
            unsafe { &mut *(image_handle_ptr as *mut DisplayData<T>) };

        let channels = display_data.pixel_format.channels();
        let bucket_width = (x_max_plus_one - x_min) as usize;
        let bucket_height = (y_max_plus_one - y_min) as usize;
        let bucket_pixel_count = bucket_width * bucket_height * channels;

        // SAFETY: pixel_data comes from the renderer and should be valid
        if pixel_data.is_null() {
            return Error::None.into();
        }

        // Zero-cost slice creation - just pointer reinterpretation, no copy!
        let bucket_data = unsafe {
            std::slice::from_raw_parts(
                pixel_data as *const T,
                bucket_pixel_count,
            )
        };

        // Pass bucket directly to callback - NO memcpy, NO accumulation!
        if let Some(ref mut fn_write) = display_data.fn_write {
            fn_write(
                &display_data.name,
                display_data.width,
                display_data.height,
                x_min as _,
                x_max_plus_one as _,
                y_min as _,
                y_max_plus_one as _,
                &display_data.pixel_format,
                bucket_data, // Just the bucket!
            )
        } else {
            Error::None
        }
        .into()
    }) {
        Ok(result) => result,
        Err(_) => {
            // If we panicked, return an error to the renderer
            Error::Undefined.into()
        }
    }
}

// Generic trampoline function for the FnFinish callback.
// FnFinish does NOT receive pixel data - user accumulates if needed.
pub(crate) extern "C" fn image_close<T: PixelType>(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
) -> ndspy_sys::PtDspyError {
    // Catch any panics to prevent unwinding into C code
    match std::panic::catch_unwind(|| {
        // SAFETY: image_handle_ptr should be valid as it was created by image_open
        if image_handle_ptr.is_null() {
            return Error::BadParameters.into();
        }
        let mut display_data =
            unsafe { Box::from_raw(image_handle_ptr as *mut DisplayData<T>) };

        // FnFinish receives no pixel data - user accumulates if needed
        let error = if let Some(ref mut fn_finish) = display_data.fn_finish {
            fn_finish(
                std::mem::take(&mut display_data.name),
                display_data.width,
                display_data.height,
                std::mem::take(&mut display_data.pixel_format),
            )
        } else {
            Error::None
        };

        // SAFETY: The callbacks were passed to us via FFI from Box::into_raw.
        // They should be dropped when DisplayData is dropped, but this causes
        // a double-free. This suggests the callbacks are being freed elsewhere,
        // possibly by the renderer. For now, we leak them to prevent crashes.
        // TODO: Investigate why double-free occurs and fix properly.
        if let Some(fn_write) = display_data.fn_write.take() {
            Box::leak(fn_write);
        }
        if let Some(fn_query) = display_data.fn_query.take() {
            Box::leak(fn_query);
        }
        if let Some(fn_finish) = display_data.fn_finish.take() {
            Box::leak(fn_finish);
        }

        error.into()
    }) {
        Ok(result) => result,
        Err(_) => {
            // If we panicked, return an error to the renderer
            Error::Undefined.into()
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn image_progress(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    _progress: f32,
) -> ndspy_sys::PtDspyError {
    // Progress logging disabled to reduce spam
    Error::None.into()
}

/// Helper for users who want the complete accumulated image at finish time.
///
/// The core API passes buckets directly to callbacks without accumulation.
/// This helper provides the common use case of receiving the complete image
/// when rendering finishes.
///
/// # Example
///
/// ```ignore
/// use std::sync::{Arc, Mutex};
///
/// // Create accumulating callbacks that deliver the full image at finish
/// let (write, finish) = nsi::output::AccumulatingCallbacks::<f32>::new(
///     |name, width, height, format, pixels| {
///         // Called once at end with complete accumulated image
///         save_image(&name, width, height, &pixels);
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "driver",
///     &[
///         nsi::string!("drivername", nsi::output::FERRIS_F32),
///         nsi::callback!("callback.write", write),
///         nsi::callback!("callback.finish", finish),
///     ],
/// );
/// ```
pub struct AccumulatingCallbacks<T: PixelType> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: PixelType> AccumulatingCallbacks<T> {
    /// Create a pair of callbacks that accumulate pixels and deliver the
    /// complete image to the finish callback.
    ///
    /// Returns `(WriteCallback, FinishCallback)` where:
    /// - The write callback accumulates buckets into an internal buffer
    /// - The finish callback delivers the complete buffer to your closure
    pub fn new<'a, F>(
        mut on_finish: F,
    ) -> (WriteCallback<'a, T>, FinishCallback<'a>)
    where
        F: FnMut(String, usize, usize, PixelFormat, Vec<T>) -> Error + 'a,
    {
        use std::sync::{Arc, Mutex};

        // Shared state between write and finish callbacks
        struct AccumState<T: PixelType> {
            buffer: Vec<T>,
            width: usize,
            height: usize,
            channels: usize,
            initialized: bool,
        }

        let state = Arc::new(Mutex::new(AccumState::<T> {
            buffer: Vec::new(),
            width: 0,
            height: 0,
            channels: 0,
            initialized: false,
        }));

        let write_state = state.clone();
        let finish_state = state;

        let write = WriteCallback::new(
            move |_name: &str,
                  width: usize,
                  height: usize,
                  x_min: usize,
                  x_max_plus_one: usize,
                  y_min: usize,
                  y_max_plus_one: usize,
                  format: &PixelFormat,
                  bucket_data: &[T]| {
                let mut state = write_state.lock().unwrap();

                // Initialize on first bucket
                if !state.initialized {
                    state.width = width;
                    state.height = height;
                    state.channels = format.channels();
                    state.buffer =
                        vec![T::default(); width * height * state.channels];
                    state.initialized = true;
                }

                // Copy bucket into the full buffer
                let bucket_width = x_max_plus_one - x_min;
                let channels = state.channels;

                for y in y_min..y_max_plus_one {
                    let src_start = (y - y_min) * bucket_width * channels;
                    let dst_start = (y * state.width + x_min) * channels;
                    let row_len = bucket_width * channels;

                    state.buffer[dst_start..dst_start + row_len]
                        .copy_from_slice(
                            &bucket_data[src_start..src_start + row_len],
                        );
                }

                Error::None
            },
        );

        let finish = FinishCallback::new(
            move |name: String,
                  width: usize,
                  height: usize,
                  format: PixelFormat| {
                let mut state = finish_state.lock().unwrap();
                let buffer = std::mem::take(&mut state.buffer);
                drop(state); // Release lock before calling user callback

                on_finish(name, width, height, format, buffer)
            },
        );

        (write, finish)
    }
}
