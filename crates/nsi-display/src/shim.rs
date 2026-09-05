//! The trait an author implements, and the shim bodies the macro calls.

use crate::{Bucket, Error, Params, Result};
use core::{
    ffi::{c_char, c_int, c_void},
    panic::AssertUnwindSafe,
};
use nsi_ffi_wrap::output::{PixelFormat, PixelType};

/// A display driver.
///
/// Implement this, then invoke
/// [`declare_display_driver!`](crate::declare_display_driver) to export
/// the symbols the renderer looks for.
///
/// `write` takes `&mut self`, so the renderer is told to serialise
/// buckets ([`PkThreadQuery`] is answered with `multithread = 0`). That
/// is the standard ndspy contract.
pub trait DisplayDriver: Sized + 'static {
    /// The scalar this driver wants its pixels in. The shim rewrites the
    /// renderer's requested format to match.
    type Pixel: PixelType;

    /// Called once, before any pixels.
    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self>;

    /// Called once per bucket.
    fn write(&mut self, bucket: Bucket<'_, Self::Pixel>) -> Result<()>;

    /// Called once, after the last bucket. Takes `self`, so the driver
    /// is consumed and cannot be used again.
    fn close(self) -> Result<()>;
}

/// State the shims keep behind `PtDspyImageHandle`.
struct Handle<D: DisplayDriver> {
    driver: D,
    format: PixelFormat,
}

/// Runs `body`, converting a panic into an error code rather than
/// unwinding into C.
fn guard(body: impl FnOnce() -> Error) -> ndspy_sys::PtDspyError {
    match std::panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(error) => error.into(),
        Err(_) => Error::Undefined.into(),
    }
}

/// # Safety
/// Called by the renderer with the ndspy contract's pointers.
#[allow(clippy::too_many_arguments)]
pub unsafe fn shim_open<D: DisplayDriver>(
    image: *mut ndspy_sys::PtDspyImageHandle,
    _drivername: *const c_char,
    _filename: *const c_char,
    width: c_int,
    height: c_int,
    param_count: c_int,
    parameters: *const ndspy_sys::UserParameter,
    format_count: c_int,
    format: *mut ndspy_sys::PtDspyDevFormat,
    flags: *mut ndspy_sys::PtFlagStuff,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() || format.is_null() || format_count <= 0 {
            return Error::BadParameters;
        }

        // SAFETY: the renderer guarantees `format_count` entries.
        let format = unsafe {
            core::slice::from_raw_parts_mut(format, format_count as usize)
        };
        // This driver's scalar is what the renderer must deliver.
        format
            .iter_mut()
            .for_each(|f| f.type_ = D::Pixel::NDSPY_TYPE);

        let pixel_format = PixelFormat::from_ndspy(format);
        // SAFETY: borrowed for this call only.
        let params = unsafe { Params::from_raw(parameters, param_count) };

        let driver = match D::open(
            params,
            width as usize,
            height as usize,
            &pixel_format,
        ) {
            Ok(driver) => driver,
            Err(error) => return error,
        };

        // The handle is ours: reclaimed in `shim_close`, once.
        let handle = Box::new(Handle {
            driver,
            format: pixel_format,
        });
        // SAFETY: `image` and `flags` are valid per the ndspy contract.
        unsafe {
            *image = Box::into_raw(handle) as _;
            if !flags.is_null() {
                (*flags).flags &=
                    !(ndspy_sys::PkDspyFlagsWantsEmptyBuckets as c_int);
            }
        }
        Error::None
    })
}

/// # Safety
/// `image` must have come from `shim_open` and still be open.
pub unsafe fn shim_data<D: DisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    _entry_size: c_int,
    data: *const u8,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() {
            return Error::BadParameters;
        }
        if data.is_null() {
            return Error::None;
        }

        // SAFETY: ours, from `shim_open`. `write` needs `&mut`, which is
        // sound because we answer `multithread = 0`.
        let handle = unsafe { &mut *(image as *mut Handle<D>) };

        let channels = handle.format.channels();
        let width = (x_max_plus_one - x_min) as usize;
        let height = (y_max_plus_one - y_min) as usize;

        // SAFETY: the renderer's buffer holds exactly this many values,
        // in the type we requested in `shim_open`.
        let pixels = unsafe {
            core::slice::from_raw_parts(
                data as *const D::Pixel,
                width * height * channels,
            )
        };

        let bucket = Bucket::new(
            x_min as usize,
            x_max_plus_one as usize,
            y_min as usize,
            y_max_plus_one as usize,
            channels,
            pixels,
        );

        match handle.driver.write(bucket) {
            Ok(()) => Error::None,
            Err(error) => error,
        }
    })
}

/// # Safety
/// `image` must have come from `shim_open` and not been closed.
pub unsafe fn shim_close<D: DisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() {
            return Error::BadParameters;
        }
        // SAFETY: ours, reclaimed exactly here. `close` consumes the
        // driver, so the type system prevents a second use.
        let handle = unsafe { Box::from_raw(image as *mut Handle<D>) };
        match handle.driver.close() {
            Ok(()) => Error::None,
            Err(error) => error,
        }
    })
}

/// # Safety
/// `data` must point to `data_len` bytes of the struct the query names.
pub unsafe fn shim_query<D: DisplayDriver>(
    _image: ndspy_sys::PtDspyImageHandle,
    query_type: ndspy_sys::PtDspyQueryType,
    data_len: c_int,
    data: *mut c_void,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if data.is_null() || data_len <= 0 {
            return Error::BadParameters;
        }
        match query_type {
            ndspy_sys::PtDspyQueryType::Thread => {
                if (data_len as usize)
                    < core::mem::size_of::<ndspy_sys::PtDspyThreadInfo>()
                {
                    return Error::BadParameters;
                }
                // `write` is `&mut self`: the renderer must serialise.
                // SAFETY: length checked above.
                unsafe {
                    (*(data as *mut ndspy_sys::PtDspyThreadInfo)).multithread =
                        0;
                }
                Error::None
            }
            ndspy_sys::PtDspyQueryType::Progressive => {
                if (data_len as usize)
                    < core::mem::size_of::<ndspy_sys::PtDspyProgressiveInfo>()
                {
                    return Error::BadParameters;
                }
                // SAFETY: length checked above.
                unsafe {
                    (*(data as *mut ndspy_sys::PtDspyProgressiveInfo))
                        .acceptProgressive = 1;
                }
                Error::None
            }
            _ => Error::Unsupported,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct Recorder {
        buckets: Arc<AtomicUsize>,
        closed: Arc<AtomicUsize>,
    }

    // A driver's state is normally private; these statics let the test
    // observe it after the shim has taken ownership of the driver.
    static BUCKETS: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();
    static CLOSED: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();

    impl DisplayDriver for Recorder {
        type Pixel = f32;

        fn open(
            _params: Params<'_>,
            _width: usize,
            _height: usize,
            _format: &PixelFormat,
        ) -> crate::Result<Self> {
            Ok(Recorder {
                buckets: Arc::clone(BUCKETS.get().unwrap()),
                closed: Arc::clone(CLOSED.get().unwrap()),
            })
        }

        fn write(&mut self, bucket: Bucket<'_, f32>) -> crate::Result<()> {
            assert!(bucket.pixels().iter().all(|p| *p == 0.5));
            self.buckets.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(self) -> crate::Result<()> {
            self.closed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    /// The full lifecycle through the shims, with the handle reclaimed
    /// exactly once. Miri's leak check is half the assertion.
    #[test]
    fn open_write_close_round_trip() {
        BUCKETS.set(Arc::new(AtomicUsize::new(0))).ok();
        CLOSED.set(Arc::new(AtomicUsize::new(0))).ok();

        let layer = CString::new("beauty.000").unwrap();
        let mut format = [ndspy_sys::PtDspyDevFormat {
            name: layer.as_ptr(),
            type_: 0,
        }];
        let mut flags = ndspy_sys::PtFlagStuff { flags: 0 };
        let filename = CString::new("render").unwrap();
        let drivername = CString::new("recorder").unwrap();
        let mut handle: ndspy_sys::PtDspyImageHandle = core::ptr::null_mut();

        // SAFETY: every pointer below is valid for this call.
        let error = unsafe {
            shim_open::<Recorder>(
                &mut handle,
                drivername.as_ptr(),
                filename.as_ptr(),
                4,
                4,
                0,
                core::ptr::null(),
                format.len() as _,
                format.as_mut_ptr(),
                &mut flags,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert!(!handle.is_null());

        // The driver asked for f32, so the shim must have said so.
        assert_eq!(f32::NDSPY_TYPE, format[0].type_);

        let channels = PixelFormat::from_ndspy(&format).channels();
        let pixels = vec![0.5f32; 2 * 2 * channels];
        // SAFETY: `handle` came from `shim_open`; `pixels` outlives the call.
        let error = unsafe {
            shim_data::<Recorder>(
                handle,
                0,
                2,
                0,
                2,
                core::mem::size_of::<f32>() as _,
                pixels.as_ptr() as *const u8,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(1, BUCKETS.get().unwrap().load(Ordering::SeqCst));

        // SAFETY: `handle` is live and reclaimed exactly here.
        let error = unsafe { shim_close::<Recorder>(handle) };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(1, CLOSED.get().unwrap().load(Ordering::SeqCst));
    }

    /// We must not promise concurrency we do not honour: `write` takes
    /// `&mut self`, so the renderer has to serialise.
    #[test]
    fn we_do_not_advertise_concurrent_buckets() {
        let mut info = ndspy_sys::PtDspyThreadInfo { multithread: 1 };
        // SAFETY: `info` is a valid, correctly sized PtDspyThreadInfo.
        let error = unsafe {
            shim_query::<Recorder>(
                core::ptr::null_mut(),
                ndspy_sys::PtDspyQueryType::Thread,
                core::mem::size_of::<ndspy_sys::PtDspyThreadInfo>() as _,
                &mut info as *mut _ as *mut core::ffi::c_void,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(
            0, info.multithread,
            "write takes &mut self, so buckets must be serialised"
        );
    }
}
