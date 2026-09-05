//! Write ɴsɪ display drivers in safe Rust.

mod params;
pub use params::Params;

mod bucket;
pub use bucket::Bucket;

mod shim;
pub use shim::DisplayDriver;
#[doc(hidden)]
pub use shim::{shim_close, shim_data, shim_open, shim_query};

pub use nsi_ffi_wrap::output::{Error, PixelFormat, PixelType};

/// The result an author's driver methods return.
pub type Result<T> = core::result::Result<T, Error>;

/// Exports `$driver` as an ɴsɪ display driver.
///
/// Emits the four symbols the renderer resolves by name. Invoke once,
/// at the crate root of a `cdylib`:
///
/// ```ignore
/// nsi_display::declare_display_driver!(MyDriver);
/// ```
///
/// Build it with `crate-type = ["cdylib"]` and give the artefact the
/// name the renderer expects — 3Delight looks for `<drivername>.dpy`, so
/// `libmy_driver.so` has to be renamed or symlinked to `my_driver.dpy`.
///
/// The macro body refers to `::ndspy_sys`, so any crate invoking it must
/// itself depend on `ndspy-sys`.
#[macro_export]
macro_rules! declare_display_driver {
    ($driver:ty) => {
        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageOpen(
            image: *mut ::ndspy_sys::PtDspyImageHandle,
            drivername: *const ::core::ffi::c_char,
            filename: *const ::core::ffi::c_char,
            width: ::core::ffi::c_int,
            height: ::core::ffi::c_int,
            param_count: ::core::ffi::c_int,
            parameters: *const ::ndspy_sys::UserParameter,
            format_count: ::core::ffi::c_int,
            format: *mut ::ndspy_sys::PtDspyDevFormat,
            flags: *mut ::ndspy_sys::PtFlagStuff,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_open::<$driver>(
                    image,
                    drivername,
                    filename,
                    width,
                    height,
                    param_count,
                    parameters,
                    format_count,
                    format,
                    flags,
                )
            }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageData(
            image: ::ndspy_sys::PtDspyImageHandle,
            x_min: ::core::ffi::c_int,
            x_max_plus_one: ::core::ffi::c_int,
            y_min: ::core::ffi::c_int,
            y_max_plus_one: ::core::ffi::c_int,
            entry_size: ::core::ffi::c_int,
            data: *const u8,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_data::<$driver>(
                    image,
                    x_min,
                    x_max_plus_one,
                    y_min,
                    y_max_plus_one,
                    entry_size,
                    data,
                )
            }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageClose(
            image: ::ndspy_sys::PtDspyImageHandle,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe { $crate::shim_close::<$driver>(image) }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageQuery(
            image: ::ndspy_sys::PtDspyImageHandle,
            query_type: ::ndspy_sys::PtDspyQueryType,
            data_len: ::core::ffi::c_int,
            data: *mut ::core::ffi::c_void,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_query::<$driver>(image, query_type, data_len, data)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The error type must round-trip to the code ndspy expects, since
    /// that is what every generated shim returns.
    #[test]
    fn errors_map_to_ndspy_codes() {
        assert_eq!(ndspy_sys::PtDspyError::None as u32, u32::from(Error::None));
        assert_eq!(
            ndspy_sys::PtDspyError::BadParams as u32,
            u32::from(Error::BadParameters)
        );
    }
}
