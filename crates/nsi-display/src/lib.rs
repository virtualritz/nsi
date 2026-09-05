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
