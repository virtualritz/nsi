//! Write ɴsɪ display drivers in safe Rust.
//!
//! 3Delight's display drivers are shared libraries the renderer loads
//! and calls into through four C entry points (`DspyImageOpen`,
//! `DspyImageData`, `DspyImageClose`, `DspyImageQuery` -- the "ndspy"
//! ABI). This crate turns that into a safe trait:
//!
//! 1. Implement `DisplayDriver` for a type of your own.
//! 2. Invoke `declare_display_driver!` once, at the crate root, to
//!    export the four symbols.
//! 3. Build the crate as a `cdylib`.
//!
//! # A complete driver
//!
//! ```ignore
//! use nsi_display::{Bucket, DisplayDriver, Error, Params, PixelFormat, Result};
//!
//! struct Ppm {
//!     path: String,
//!     width: usize,
//!     height: usize,
//!     channels: usize,
//!     pixels: Vec<u8>,
//! }
//!
//! impl DisplayDriver for Ppm {
//!     type Pixel = u8;
//!
//!     fn open(
//!         params: Params<'_>,
//!         width: usize,
//!         height: usize,
//!         format: &PixelFormat,
//!     ) -> Result<Self> {
//!         let channels = format.channels();
//!         Ok(Ppm {
//!             path: params.string("imagefilename").unwrap_or("render").to_owned(),
//!             width,
//!             height,
//!             channels,
//!             pixels: vec![0u8; width * height * channels],
//!         })
//!     }
//!
//!     fn write(&mut self, bucket: Bucket<'_, u8>) -> Result<()> {
//!         for y in bucket.y_min()..bucket.y_max() {
//!             for x in bucket.x_min()..bucket.x_max() {
//!                 let src = ((y - bucket.y_min()) * bucket.width()
//!                     + (x - bucket.x_min())) * self.channels;
//!                 let dst = (y * self.width + x) * self.channels;
//!                 self.pixels[dst..dst + self.channels]
//!                     .copy_from_slice(&bucket.pixels()[src..src + self.channels]);
//!             }
//!         }
//!         Ok(())
//!     }
//!
//!     fn close(self) -> Result<()> {
//!         let file = std::fs::File::create(format!("{}.ppm", self.path))
//!             .map_err(|_| Error::NoResource)?;
//!         // ... write a PPM header and `self.pixels` ...
//!         Ok(())
//!     }
//! }
//!
//! nsi_display::declare_display_driver!(Ppm);
//! ```
//!
//! # How the renderer finds your driver
//!
//! This is the single most important practical thing about running a
//! driver, and it is documented nowhere in 3Delight's own materials --
//! not `nsi.pdf`, not `3delight.config`.
//!
//! `drivername` (the NSI attribute you set on the `outputdriver` node)
//! is **not a path**. 3Delight resolves it against a search path, the
//! same mechanism it uses for shaders, textures, archives and
//! procedurals. The default display search path is
//! `.:$DELIGHT/displays:$DELIGHT/lib`, searched in that order, with
//! `.` -- the renderer's current working directory at render time --
//! searched *first*.
//!
//! Unlike every other resource kind, there is **no way to override
//! this path**. Shaders, textures, archives, procedurals and generic
//! resources each have a `DL_<TYPE>S_PATH` environment variable
//! (`DL_SHADERS_PATH`, `DL_TEXTURES_PATH`, `DL_ARCHIVES_PATH`,
//! `DL_PROCEDURALS_PATH`, `DL_RESOURCES_PATH`); `display` has none.
//! `3delight.config` documents seven keys, none of them
//! display-related. `nsi.pdf`'s index of the `.global` node's
//! attributes has no search-path attribute of any kind. So there are
//! exactly two places a driver can go: the renderer's working
//! directory, or `$DELIGHT/displays` (often not writable by an
//! unprivileged build).
//!
//! The artefact must be named `<drivername>.dpy`. Cargo produces
//! `libfoo.so` (or `.dll`/`.dylib`), so it has to be renamed or
//! symlinked before the renderer can find it.
//!
//! **The trap:** `drivername` can collide with a format 3Delight
//! implements internally. Naming a driver `"png"` silently resolves to
//! 3Delight's own built-in PNG driver (`dspy_png` internally) instead
//! of yours -- you get a valid, correct-looking file, written by
//! entirely the wrong code path, with no error of any kind. Pick a
//! name unlikely to collide. If you need certainty, use the
//! negative-control pattern in `tests/render.rs`: render once with the
//! driver *not* staged and confirm nothing gets written, then stage it
//! and render again.
//!
//! # What the shims give you
//!
//! `declare_display_driver!` generates the four `extern "C"`
//! functions; each forwards, through a thin shim, to your
//! `DisplayDriver` implementation. The shims are what make the trait
//! safe to implement:
//!
//! - A panic in your code is caught and converted to an ndspy error
//!   code rather than unwinding into the renderer's C stack.
//! - The image handle is heap-allocated once in `open` and reclaimed
//!   exactly once in `close`; there is no way to leak it or free it
//!   twice from safe code.
//! - Every pointer the renderer hands you (parameters, pixel data) is
//!   borrowed for the duration of that one call, never stored past it.
//! - `Params` checks the ndspy type tag before reading a value, so
//!   asking for the wrong type returns `None` instead of reinterpreting
//!   bytes.
//!
//! `write` takes `&mut self`, and the shims answer `PkThreadQuery` with
//! `multithread = 0`, so the renderer serialises buckets. A driver that
//! wants concurrent bucket delivery instead implements
//! `ConcurrentDisplayDriver`; see "Choosing a trait" below.
//!
//! # Choosing a trait
//!
//! - `DisplayDriver` -- the default. The renderer serialises buckets
//!   (`multithread = 0`), `write` takes `&mut self`, and no
//!   synchronisation is needed. Right for a driver that accumulates into
//!   a frame buffer or writes a file.
//! - `ConcurrentDisplayDriver` -- opt in. The renderer may deliver
//!   buckets from several threads at once (`multithread = 1`), `write`
//!   takes `&self`, and the type must be `Sync`. Right for a driver
//!   whose state is already behind atomics or a lock, or which writes
//!   each bucket independently.
//!
//! `PkThreadQuery` is a 3Delight extension -- standard ndspy has no
//! thread negotiation and always serialises -- so `DisplayDriver` is the
//! portable default. A crate invokes exactly one of
//! `declare_display_driver!` or `declare_concurrent_display_driver!`;
//! invoking both defines the same four symbols twice and will not link.
//!
//! # Scope
//!
//! This crate does not implement `DspyImageReopen`,
//! `DspyImageActiveRegion` or `DspyImageDelayClose`. If your driver
//! needs any of those, this crate isn't (yet) for you.
//!
//! # Dependencies
//!
//! `declare_display_driver!` expands to code that refers to
//! `::ndspy_sys` directly, so any crate invoking it must itself depend
//! on `ndspy-sys`.

mod params;
pub use params::{Params, Value};

mod bucket;
pub use bucket::Bucket;

mod shim;
pub use shim::{ConcurrentDisplayDriver, DisplayDriver};
#[doc(hidden)]
pub use shim::{
    concurrent_shim_close, concurrent_shim_data, concurrent_shim_open,
    concurrent_shim_query, shim_close, shim_data, shim_open, shim_query,
};

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
///
/// A crate invokes exactly one of `declare_display_driver!` or
/// [`declare_concurrent_display_driver!`]; invoking both defines the
/// same four symbols twice and will not link.
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

/// Exports `$driver` as an ɴsɪ display driver that accepts buckets from
/// several threads at once.
///
/// Emits the same four symbols as [`declare_display_driver!`], with
/// identical `extern "C"` signatures -- the renderer resolves the same
/// names either way. Invoke once, at the crate root of a `cdylib`:
///
/// ```ignore
/// nsi_display::declare_concurrent_display_driver!(MyDriver);
/// ```
///
/// Build it with `crate-type = ["cdylib"]` and give the artefact the
/// name the renderer expects — 3Delight looks for `<drivername>.dpy`, so
/// `libmy_driver.so` has to be renamed or symlinked to `my_driver.dpy`.
///
/// The macro body refers to `::ndspy_sys`, so any crate invoking it must
/// itself depend on `ndspy-sys`.
///
/// A crate invokes exactly one of [`declare_display_driver!`] or
/// `declare_concurrent_display_driver!`; invoking both defines the same
/// four symbols twice and will not link.
#[macro_export]
macro_rules! declare_concurrent_display_driver {
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
                $crate::concurrent_shim_open::<$driver>(
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
                $crate::concurrent_shim_data::<$driver>(
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
            unsafe { $crate::concurrent_shim_close::<$driver>(image) }
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
                $crate::concurrent_shim_query::<$driver>(
                    image, query_type, data_len, data,
                )
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
