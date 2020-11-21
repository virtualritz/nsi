#![doc(cfg(feature = "output"))]
//! # Output Driver Callbacks
//! This module allows passing closures to the renderer as callbacks.
//!
//! There are three types of closure:
//! * [`FnOpen`] is called once when the
//!   [`OutputDriver`](crate::context::NodeType::OutputDriver) is
//!   *opened* by the renderer.
//! * [`FnWrite`] is called for each bucket of pixel data the renderer
//!   sends to the
//!   [`OutputDriver`](crate::context::NodeType::OutputDriver).
//! * [`FnFinish`] is called once when the
//!   [`OutputDriver`](crate::context::NodeType::OutputDriver) is
//!   *closed* by the renderer.
//!
//! As a user you can choose how to use this API.
//! * To get one final big blob of pixel data, when rendering is
//!   finished, it suffices to only implement an [`FnFinish`] closure.
//!   The [`Vec<f32>`] buffer that was used to accumulate the data is
//!   passed back to this closure.
//! * To update a buffer while the renderer is working implement an
//!   [`FnWrite`] closure.
//!
//!
use core::ptr;
use ndspy_sys;
use num_enum::IntoPrimitive;
use std::{
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
};

use crate::argument::CallbackPtr;

// Juypiter notebook support ------------------------------------------
#[cfg(feature = "juypiter")]
mod juypiter;

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, IntoPrimitive)]
pub enum Error {
    None = ndspy_sys::PtDspyError_PkDspyErrorNone as _,
    NoMemory = ndspy_sys::PtDspyError_PkDspyErrorNoMemory as _,
    Unsupported = ndspy_sys::PtDspyError_PkDspyErrorUnsupported as _,
    BadParameters = ndspy_sys::PtDspyError_PkDspyErrorBadParams as _,
    NoResource = ndspy_sys::PtDspyError_PkDspyErrorNoResource as _,
    Undefined = ndspy_sys::PtDspyError_PkDspyErrorUndefined as _,
    Stop = ndspy_sys::PtDspyError_PkDspyErrorStop as _,
}

pub type FnOpen<'a> = dyn FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // Pixel format.
        &mut Vec<&str>,
    ) -> Error
    + 'a;
//Result<(Vec<Format>, Flags), Error>;

pub type FnWrite<'a> = dyn FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // x_min.
        usize,
        // x_max_plus_one
        usize,
        // y_min.
        usize,
        // y_max_plus_one,
        usize,
        // Pixel format.
        &[&str],
        // Pixel data.
        &mut [f32],
    ) -> Error
    + 'a;

/// A closure which is called once per
/// [`OutputDriver`](crate::context::NodeType::OutputDriver)
/// instance. It is passed to NSI via the `"callback.finish"` attribute
/// on that node.
/// # Example
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # let ctx = nsi::Context::new(&[]).unwrap();
/// # ctx.create("display_driver", nsi::NodeType::OutputDriver, &[]);
/// let finish = nsi::output::FinishCallback::new(
///     |name: &str,
///      width: usize,
///      height: usize,
///      pixel_format: Vec<&str>,
///      pixel_data: Vec<f32>| {
///         println!(
///             "The 1st channel of the 1st pixel has the value {}.",
///             pixel_data[0]
///         );
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "oxidized_output_driver",
///     &[
///         nsi::string!("drivername", "ferris"),
///         // When done, send all pixels to the finish closure.
///         nsi::callback!("callback.finish", finish),
///     ],
/// );
/// # }
/// ```
pub type FnFinish<'a> = dyn FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // Pixel format.
        Vec<&str>,
        // Pixel data.
        Vec<f32>,
    ) -> Error
    + 'a;

enum Query {}

type FnQuery<'a> = dyn FnMut(
    Query
) -> Error + 'a;

pub struct OpenCallback<'a>(Box<Box<Box<FnOpen<'a>>>>);

impl<'a> OpenCallback<'a> {
    pub fn new<F>(fn_open: F) -> Self
    where
        F: FnMut(&str, usize, usize, &mut Vec<&str>) -> Error + 'a,
    {
        OpenCallback(Box::new(Box::new(Box::new(fn_open))))
    }
}

impl CallbackPtr for OpenCallback<'_> {
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

pub struct WriteCallback<'a>(Box<Box<Box<FnWrite<'a>>>>);

impl<'a> WriteCallback<'a> {
    pub fn new<F>(fn_write: F) -> Self
    where
        F: FnMut(&str, usize, usize, usize, usize, usize, usize, &[&str], &mut [f32]) -> Error + 'a,
    {
        WriteCallback(Box::new(Box::new(Box::new(fn_write))))
    }
}

impl CallbackPtr for WriteCallback<'_> {
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

pub struct FinishCallback<'a>(Box<Box<Box<FnFinish<'a>>>>);

impl<'a> FinishCallback<'a> {
    pub fn new<F>(fn_finish: F) -> Self
    where
        F: FnMut(&str, usize, usize, Vec<&str>, Vec<f32>) -> Error + 'a,
    {
        FinishCallback(Box::new(Box::new(Box::new(fn_finish))))
    }
}

impl CallbackPtr for FinishCallback<'_> {
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

struct DisplayData<'a> {
    name: &'a str,
    width: usize,
    height: usize,
    pixel_format: Vec<&'a str>,
    pixel_data: Vec<f32>,
    fn_open: Option<Box<Box<Box<FnOpen<'a>>>>>,
    fn_write: Option<Box<Box<Box<FnWrite<'a>>>>>,
    // Unused atm.
    fn_query: Option<Box<Box<Box<FnQuery<'a>>>>>,
    fn_finish: Option<Box<Box<Box<FnFinish<'a>>>>>,
}

impl<'a> DisplayData<'a> {
    fn box_into_tuple(display_data: Box<Self>) -> (&'a str, usize, usize, Vec<&'a str>, Vec<f32>, Option<Box<Box<Box<FnFinish<'a>>>>>) {
        // These boxes somehow get deallocated twice if we don't suppress this here.
        // No idea why.
        if let Some(fn_open) = display_data.fn_open {
            Box::into_raw(fn_open);
        }
        if let Some(fn_write) = display_data.fn_write {
            Box::into_raw(fn_write);
        }
        if let Some(fn_query) = display_data.fn_query {
            Box::into_raw(fn_query);
        }

        (
            display_data.name,
            display_data.width,
            display_data.height,
            display_data.pixel_format,
            display_data.pixel_data,
            display_data.fn_finish
        )
    }
}

fn get_parameter_triple_box<T: ?Sized>(
    name: &str,
    type_: u8,
    len: usize,
    parameters: &mut [ndspy_sys::UserParameter],
) -> Option<Box<Box<Box<T>>>> {
    for p in parameters.iter() {
        let p_name = unsafe { CStr::from_ptr(p.name) }.to_str().unwrap();

        if name == p_name && type_ == p.valueType as _ && len == p.valueCount as _ {
            if p.value != ptr::null_mut() {
                return Some(unsafe { Box::from_raw(p.value as *mut Box<Box<T>>) });
            } else {
                // Parameter exists but value is missing –
                // exit quietly.
                break;
            }
        }
    }
    None
}

#[no_mangle]
pub(crate) extern "C" fn image_open(
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
    // FIXME: check that driver_name is "ferris".
    if (image_handle_ptr == ptr::null_mut()) || (output_filename == ptr::null_mut()) {
        return Error::BadParameters.into();
    }

    let parameters = unsafe {
        std::slice::from_raw_parts_mut(std::mem::transmute(parameters), parameters_count as _)
    };

    let format = unsafe { std::slice::from_raw_parts_mut(format, format_count as _) };

    // Collect format names as &str and force format to f32.
    let pixel_format = format
        .iter_mut()
        .enumerate()
        .map(|format| {
            // Ensure all channels are sent to us as 32bit float.
            format.1.type_ = ndspy_sys::PkDspyFloat32;
            unsafe { CStr::from_ptr(format.1.name).to_str().unwrap() }
        })
        .collect::<Vec<_>>();

    let mut display_data = Box::new(DisplayData {
        name: unsafe { CStr::from_ptr(output_filename).to_str().unwrap() },
        width: width as _,
        height: height as _,
        pixel_format,
        pixel_data: vec![0.0f32; width as usize * height as usize * format.len()],
        fn_open: get_parameter_triple_box::<FnOpen>("callback.open", b'p', 1, parameters),
        fn_write: get_parameter_triple_box::<FnWrite>("callback.write", b'p', 1, parameters),
        fn_query: None, // get_parameter_triple_box::<FnQuery>("callback.query", b'p', 1, parameters),
        fn_finish: get_parameter_triple_box::<FnFinish>("callback.finish", b'p', 1, parameters),
    });

    let error = if let Some(ref mut fn_open) = display_data.fn_open {
        fn_open(
            display_data.name,
            width as _,
            height as _,
            &mut display_data.pixel_format
        )
    } else {
        Error::None
    };

    unsafe {
        *image_handle_ptr = Box::into_raw(display_data) as _;
    }

    // We want to be called for all buckets.
    unsafe {
        (*flag_stuff).flags = ndspy_sys::PkDspyFlagsWantsEmptyBuckets as _;
    }

    error.into()
}

#[no_mangle]
pub(crate) extern "C" fn image_query(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    query_type: ndspy_sys::PtDspyQueryType,
    data_len: c_int,
    data: *mut c_void,
) -> ndspy_sys::PtDspyError {
    println!("Query: {:?}", query_type);

    match query_type {
        ndspy_sys::PtDspyQueryType_PkRenderProgress => {
            if (data_len as usize) < core::mem::size_of::<ndspy_sys::PtDspyRenderProgressFuncPtr>()
            {
                Error::BadParameters
            } else {
                *unsafe {
                    &mut std::mem::transmute::<_, ndspy_sys::PtDspyRenderProgressFuncPtr>(data)
                } = Some(image_progress);
                Error::None
            }
        }
        _ => Error::Unsupported,
    }
    .into()
}

#[no_mangle]
pub(crate) extern "C" fn image_write(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    _entry_size: c_int,
    pixel_data: *const u8,
) -> ndspy_sys::PtDspyError {
    let display_data = unsafe { &mut *(image_handle_ptr as *mut DisplayData) };

    // _entry_size is pixel_length in u8s, we need pixel length in f32s.
    let pixel_length = display_data.pixel_format.len();

    let pixel_data = unsafe {
        std::slice::from_raw_parts(
            pixel_data as *const f32,
            pixel_length * ((x_max_plus_one - x_min) * (y_max_plus_one - y_min)) as usize,
        )
    };

    let bucket_width = pixel_length * (x_max_plus_one - x_min) as usize;

    let mut source_index = 0;
    for y in y_min as usize..y_max_plus_one as _ {
        let dest_index = (y * display_data.width + x_min as usize) * pixel_length;

        // We memcpy() each scanline.
        (&mut display_data.pixel_data[dest_index..dest_index + bucket_width])
            .copy_from_slice(&(pixel_data[source_index..source_index + bucket_width]));

        source_index += bucket_width;
    }

    // Call the closure.
    let error = if let Some(ref mut fn_write) = display_data.fn_write {
        fn_write(
            display_data.name,
            display_data.width,
            display_data.height,
            x_min as _,
            x_max_plus_one as _,
            y_min as _,
            y_max_plus_one as _,
            &display_data.pixel_format,
            display_data.pixel_data.as_mut_slice(),
        )
    } else {
        Error::None
    };

    error.into()
}

#[no_mangle]
pub(crate) extern "C" fn image_close(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
) -> ndspy_sys::PtDspyError {
    println!("Closing!");

    let display_data = unsafe { Box::from_raw(image_handle_ptr as *mut DisplayData) };

    let (name, width, height, pixel_format, pixel_data, fn_finish) =
    DisplayData::box_into_tuple(display_data);

    let error = if let Some(mut fn_finish) = fn_finish {
        let error = fn_finish(name, width, height, pixel_format, pixel_data);
        Box::into_raw(fn_finish);
        error
    } else {
        Error::None
    };

    //if let Some(ref mut fn_finish


    error.into()
}

#[no_mangle]
extern "C" fn image_progress(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    progress: f32,
) -> ndspy_sys::PtDspyError {
    println!("\rProgress: {}", progress * 100.0);
    Error::None.into()
}
