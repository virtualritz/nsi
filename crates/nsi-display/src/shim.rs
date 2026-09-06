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
/// buckets (`PkThreadQuery` is answered with `multithread = 0`). That
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

/// A display driver that accepts buckets from several threads at once.
///
/// Implement this instead of [`DisplayDriver`], then invoke
/// [`declare_concurrent_display_driver!`](crate::declare_concurrent_display_driver)
/// to export the symbols the renderer looks for. A crate invokes exactly
/// one of the two macros.
///
/// `write` takes `&self`, so the renderer is told it may deliver buckets
/// from several threads at once (`PkThreadQuery` is answered with
/// `multithread = 1`). `Sync` on the trait is what makes that sound: the
/// renderer may be inside `write` on another thread while it calls in on
/// this one. `PkThreadQuery` is a 3Delight extension -- standard ndspy
/// serialises -- so [`DisplayDriver`] is the portable default.
pub trait ConcurrentDisplayDriver: Sized + Sync + 'static {
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

    /// Called once per bucket, possibly from several threads at once.
    fn write(&self, bucket: Bucket<'_, Self::Pixel>) -> Result<()>;

    /// Called once, after the last bucket. Takes `self`, so the driver
    /// is consumed and cannot be used again.
    fn close(self) -> Result<()>;
}

/// State the shims keep behind `PtDspyImageHandle`.
///
/// Generic over the driver only, with no trait bound: the same layout
/// serves both [`DisplayDriver`] and [`ConcurrentDisplayDriver`], and
/// each shim function supplies whichever bound it needs.
struct Handle<D> {
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

        // Every path out of here must leave a defined handle. ndspy does
        // not promise the caller pre-nulled `*image`, and an author is
        // free to return `Err(Error::None)` -- which reports success --
        // so without this the renderer could be handed whatever `*image`
        // happened to contain on entry.
        // SAFETY: `image` is non-null per the check above.
        unsafe { *image = core::ptr::null_mut() };

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
/// Called by the renderer with the ndspy contract's pointers.
#[allow(clippy::too_many_arguments)]
pub unsafe fn concurrent_shim_open<D: ConcurrentDisplayDriver>(
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

        // Every path out of here must leave a defined handle. ndspy does
        // not promise the caller pre-nulled `*image`, and an author is
        // free to return `Err(Error::None)` -- which reports success --
        // so without this the renderer could be handed whatever `*image`
        // happened to contain on entry.
        // SAFETY: `image` is non-null per the check above.
        unsafe { *image = core::ptr::null_mut() };

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

        // The handle is ours: reclaimed in `concurrent_shim_close`, once.
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

/// Validates `entry_size` against `channels` and the rectangle, then
/// builds the `Bucket` the renderer's data describes.
///
/// Shared between the serialised and concurrent shims: every check here,
/// and the slice it builds, only ever reads the renderer's buffer -- it
/// is sound however the caller borrowed the handle behind it.
///
/// # Safety
/// `data` must point to at least `(x_max_plus_one - x_min) *
/// (y_max_plus_one - y_min) * channels` values of `P`, per the ndspy
/// contract.
#[allow(clippy::too_many_arguments)]
unsafe fn validate_and_build_bucket<'a, P: PixelType>(
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    entry_size: c_int,
    data: *const u8,
    channels: usize,
) -> core::result::Result<Bucket<'a, P>, Error> {
    // An inverted rectangle would sign-extend into an enormous
    // length through the `as usize` casts below.
    if x_max_plus_one < x_min || y_max_plus_one < y_min {
        return Err(Error::BadParameters);
    }

    // `entry_size` is the renderer's ground truth for the bytes it wrote
    // per pixel; `channels` is the result of parsing the layer names.
    // When the two disagree the parse is wrong, and believing it would
    // build a slice running past the end of the renderer's buffer and
    // hand it to safe author code. Fail loudly instead of reading out of
    // bounds.
    if entry_size as usize != channels * core::mem::size_of::<P>() {
        return Err(Error::BadParameters);
    }

    let width = (x_max_plus_one - x_min) as usize;
    let height = (y_max_plus_one - y_min) as usize;

    // SAFETY: the renderer's buffer holds exactly this many values, in
    // the type requested in `open`. The rest is the caller's obligation.
    let pixels = unsafe {
        core::slice::from_raw_parts(data as *const P, width * height * channels)
    };

    Ok(Bucket::new(
        x_min as usize,
        x_max_plus_one as usize,
        y_min as usize,
        y_max_plus_one as usize,
        channels,
        pixels,
    ))
}

/// # Safety
/// `image` must have come from `shim_open` and still be open.
pub unsafe fn shim_data<D: DisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    entry_size: c_int,
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

        // SAFETY: the renderer's buffer holds exactly this many bytes,
        // in the type requested in `shim_open`; checked against
        // `entry_size` inside.
        let bucket = match unsafe {
            validate_and_build_bucket::<D::Pixel>(
                x_min,
                x_max_plus_one,
                y_min,
                y_max_plus_one,
                entry_size,
                data,
                channels,
            )
        } {
            Ok(bucket) => bucket,
            Err(error) => return error,
        };

        match handle.driver.write(bucket) {
            Ok(()) => Error::None,
            Err(error) => error,
        }
    })
}

/// # Safety
/// `image` must have come from `concurrent_shim_open` and still be open.
/// The renderer may call this concurrently from several threads on the
/// same `image`, per the `multithread = 1` answered in
/// `concurrent_shim_query`.
pub unsafe fn concurrent_shim_data<D: ConcurrentDisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    entry_size: c_int,
    data: *const u8,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() {
            return Error::BadParameters;
        }
        if data.is_null() {
            return Error::None;
        }

        // SAFETY: ours, from `concurrent_shim_open`. Shared, not
        // exclusive: we answer `multithread = 1`, so the renderer may be
        // inside this call on another thread right now, on the same
        // handle. `&mut` here would be two exclusive references to one
        // `Handle<D>`; every field read below is just that -- a read.
        let handle = unsafe { &*(image as *const Handle<D>) };

        let channels = handle.format.channels();

        // SAFETY: the renderer's buffer holds exactly this many bytes,
        // in the type requested in `concurrent_shim_open`; checked
        // against `entry_size` inside.
        let bucket = match unsafe {
            validate_and_build_bucket::<D::Pixel>(
                x_min,
                x_max_plus_one,
                y_min,
                y_max_plus_one,
                entry_size,
                data,
                channels,
            )
        } {
            Ok(bucket) => bucket,
            Err(error) => return error,
        };

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
/// `image` must have come from `concurrent_shim_open` and not been
/// closed. The renderer calls `close` once, after every `write` has
/// returned, so no concurrent access remains by the time this runs.
pub unsafe fn concurrent_shim_close<D: ConcurrentDisplayDriver>(
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

/// # Safety
/// `data` must point to `data_len` bytes of the struct the query names.
pub unsafe fn concurrent_shim_query<D: ConcurrentDisplayDriver>(
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
                // `write` is `&self`: the renderer may deliver buckets
                // from several threads at once.
                // SAFETY: length checked above.
                unsafe {
                    (*(data as *mut ndspy_sys::PtDspyThreadInfo)).multithread =
                        1;
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

    /// A driver whose `write` must never be reached.
    struct NeverWrites;

    impl DisplayDriver for NeverWrites {
        type Pixel = f32;

        fn open(
            _params: Params<'_>,
            _width: usize,
            _height: usize,
            _format: &PixelFormat,
        ) -> crate::Result<Self> {
            Ok(NeverWrites)
        }

        fn write(&mut self, _bucket: Bucket<'_, f32>) -> crate::Result<()> {
            panic!("the shim must reject the bucket before calling `write`")
        }

        fn close(self) -> crate::Result<()> {
            Ok(())
        }
    }

    /// The round-trip test sizes its buffer with the same `channels()`
    /// the shim uses, so it cannot see a divergence. These buckets are
    /// sized by `entry_size` -- the renderer's ground truth -- and
    /// disagree with the parsed channel count, which is exactly the
    /// case that used to build a slice past the end of the renderer's
    /// buffer. A panic from `write` would surface as `Undefined`, so
    /// `BadParams` also proves `write` was never called.
    #[test]
    fn a_bucket_the_shim_cannot_trust_is_rejected() {
        let layer = CString::new("beauty.000").unwrap();
        let mut format = [ndspy_sys::PtDspyDevFormat {
            name: layer.as_ptr(),
            type_: 0,
        }];
        let mut flags = ndspy_sys::PtFlagStuff { flags: 0 };
        let name = CString::new("never").unwrap();
        let mut handle: ndspy_sys::PtDspyImageHandle = core::ptr::null_mut();

        // SAFETY: every pointer below is valid for this call.
        let error = unsafe {
            shim_open::<NeverWrites>(
                &mut handle,
                name.as_ptr(),
                name.as_ptr(),
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

        // `beauty.000` parses to one channel; the renderer here claims
        // two floats per pixel. The buffer holds what the renderer
        // says, so trusting the parse would read only half of it -- and
        // the reverse mismatch would read off the end.
        let pixels = [0.5f32; 2 * 2 * 2];
        // SAFETY: `handle` came from `shim_open`; `pixels` outlives the call.
        let error = unsafe {
            shim_data::<NeverWrites>(
                handle,
                0,
                2,
                0,
                2,
                (2 * core::mem::size_of::<f32>()) as _,
                pixels.as_ptr() as *const u8,
            )
        };
        assert_eq!(
            ndspy_sys::PtDspyError::BadParams as u32,
            error as u32,
            "entry_size disagreeing with channels() must be refused"
        );

        // An inverted rectangle sign-extends into an enormous length.
        // SAFETY: as above.
        let error = unsafe {
            shim_data::<NeverWrites>(
                handle,
                2,
                0,
                0,
                2,
                core::mem::size_of::<f32>() as _,
                pixels.as_ptr() as *const u8,
            )
        };
        assert_eq!(
            ndspy_sys::PtDspyError::BadParams as u32,
            error as u32,
            "an inverted rectangle must be refused"
        );

        // SAFETY: `handle` is live and reclaimed exactly here.
        let error = unsafe { shim_close::<NeverWrites>(handle) };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
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

    struct ConcurrentRecorder {
        buckets: Arc<AtomicUsize>,
    }

    // A driver's state is normally private; this static lets the test
    // observe it after the shim has taken ownership of the driver.
    static CONCURRENT_BUCKETS: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();

    impl ConcurrentDisplayDriver for ConcurrentRecorder {
        type Pixel = f32;

        fn open(
            _params: Params<'_>,
            _width: usize,
            _height: usize,
            _format: &PixelFormat,
        ) -> crate::Result<Self> {
            Ok(ConcurrentRecorder {
                buckets: Arc::clone(CONCURRENT_BUCKETS.get().unwrap()),
            })
        }

        fn write(&self, bucket: Bucket<'_, f32>) -> crate::Result<()> {
            assert!(bucket.pixels().iter().all(|p| *p == 0.5));
            self.buckets.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(self) -> crate::Result<()> {
            Ok(())
        }
    }

    /// We answer `PkThreadQuery` with `multithread = 1` for a driver
    /// that opts into concurrency, mirroring
    /// `we_do_not_advertise_concurrent_buckets` above.
    #[test]
    fn we_advertise_concurrent_buckets_when_asked() {
        let mut info = ndspy_sys::PtDspyThreadInfo { multithread: 0 };
        // SAFETY: `info` is a valid, correctly sized PtDspyThreadInfo.
        let error = unsafe {
            concurrent_shim_query::<ConcurrentRecorder>(
                core::ptr::null_mut(),
                ndspy_sys::PtDspyQueryType::Thread,
                core::mem::size_of::<ndspy_sys::PtDspyThreadInfo>() as _,
                &mut info as *mut _ as *mut core::ffi::c_void,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(
            1, info.multithread,
            "write takes &self, so the renderer may deliver buckets \
             concurrently"
        );
    }

    /// We answer `PkThreadQuery` with `multithread = 1`, so prove we
    /// honour it: drive `concurrent_shim_data` from two threads at once
    /// on one handle, as the renderer is now told it may.
    ///
    /// Run under Miri, whose data-race detector is the actual assertion:
    ///
    /// ```text
    /// cargo +nightly miri test -p nsi-display --lib -- concurrent_buckets
    /// ```
    #[test]
    fn concurrent_buckets_are_honoured_not_just_advertised() {
        CONCURRENT_BUCKETS.set(Arc::new(AtomicUsize::new(0))).ok();

        let layer = CString::new("beauty.000").unwrap();
        let mut format = [ndspy_sys::PtDspyDevFormat {
            name: layer.as_ptr(),
            type_: 0,
        }];
        let mut flags = ndspy_sys::PtFlagStuff { flags: 0 };
        let filename = CString::new("render").unwrap();
        let drivername = CString::new("concurrent_recorder").unwrap();
        let mut handle: ndspy_sys::PtDspyImageHandle = core::ptr::null_mut();

        // SAFETY: every pointer below is valid for this call.
        let error = unsafe {
            concurrent_shim_open::<ConcurrentRecorder>(
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

        let channels = PixelFormat::from_ndspy(&format).channels();
        let bucket = vec![0.5f32; 2 * 2 * channels];

        // The renderer hands different regions to different threads; the
        // handle is shared. Carried in a newtype rather than as a
        // `usize`, which would launder away the pointer's provenance and
        // blind Miri to exactly what we are asking it to check.
        struct Shared(ndspy_sys::PtDspyImageHandle);
        // SAFETY: this is the renderer's contract under `multithread = 1`
        // -- the handle is used from several threads at once.
        unsafe impl Sync for Shared {}
        let shared = Shared(handle);

        std::thread::scope(|scope| {
            for y in 0..2 {
                let bucket = &bucket;
                let shared = &shared;
                scope.spawn(move || {
                    // SAFETY: `shared.0` came from `concurrent_shim_open`
                    // and is still open; `bucket` outlives the call.
                    let error = unsafe {
                        concurrent_shim_data::<ConcurrentRecorder>(
                            shared.0,
                            0,
                            2,
                            y * 2,
                            y * 2 + 2,
                            core::mem::size_of::<f32>() as _,
                            bucket.as_ptr() as *const u8,
                        )
                    };
                    assert_eq!(
                        ndspy_sys::PtDspyError::None as u32,
                        error as u32
                    );
                });
            }
        });

        assert_eq!(
            2,
            CONCURRENT_BUCKETS.get().unwrap().load(Ordering::SeqCst),
            "both concurrent buckets must reach the driver"
        );

        // SAFETY: `handle` is live and reclaimed exactly here.
        let error =
            unsafe { concurrent_shim_close::<ConcurrentRecorder>(handle) };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
    }
}
