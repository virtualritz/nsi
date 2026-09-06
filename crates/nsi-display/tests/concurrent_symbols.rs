//! The concurrent macro must export the same four symbols, with
//! identical C signatures, as `declare_display_driver!`.
use nsi_display::{
    Bucket, ConcurrentDisplayDriver, Params, PixelFormat, Result,
};

struct Noop;

impl ConcurrentDisplayDriver for Noop {
    type Pixel = f32;

    fn open(
        _: Params<'_>,
        _: usize,
        _: usize,
        _: &PixelFormat,
    ) -> Result<Self> {
        Ok(Noop)
    }

    fn write(&self, _: Bucket<'_, f32>) -> Result<()> {
        Ok(())
    }

    fn close(self) -> Result<()> {
        Ok(())
    }
}

nsi_display::declare_concurrent_display_driver!(Noop);

#[test]
fn the_declared_symbols_are_callable_through_c_signatures() {
    // Taking the symbols at their C types is the assertion: if the macro
    // emitted the wrong signature, this does not compile.
    let open: unsafe extern "C" fn(
        *mut ndspy_sys::PtDspyImageHandle,
        *const core::ffi::c_char,
        *const core::ffi::c_char,
        core::ffi::c_int,
        core::ffi::c_int,
        core::ffi::c_int,
        *const ndspy_sys::UserParameter,
        core::ffi::c_int,
        *mut ndspy_sys::PtDspyDevFormat,
        *mut ndspy_sys::PtFlagStuff,
    ) -> ndspy_sys::PtDspyError = DspyImageOpen;

    let close: unsafe extern "C" fn(
        ndspy_sys::PtDspyImageHandle,
    ) -> ndspy_sys::PtDspyError = DspyImageClose;

    // Types below are transcribed independently from the authoritative
    // C header (ndspy.h), not derived from the macro under test —
    // otherwise a shared mistake between macro and test would be
    // invisible.
    let data: unsafe extern "C" fn(
        ndspy_sys::PtDspyImageHandle,
        core::ffi::c_int,
        core::ffi::c_int,
        core::ffi::c_int,
        core::ffi::c_int,
        core::ffi::c_int,
        *const u8,
    ) -> ndspy_sys::PtDspyError = DspyImageData;

    let query: unsafe extern "C" fn(
        ndspy_sys::PtDspyImageHandle,
        ndspy_sys::PtDspyQueryType,
        core::ffi::c_int,
        *mut core::ffi::c_void,
    ) -> ndspy_sys::PtDspyError = DspyImageQuery;

    assert!(
        !(open as usize == 0
            || close as usize == 0
            || data as usize == 0
            || query as usize == 0)
    );
}
