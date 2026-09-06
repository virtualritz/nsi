//! A borrowed view over the parameters ndspy hands a driver.

use core::{
    ffi::{CStr, c_char, c_int},
    slice,
};

/// The parameters the renderer passes to `DspyImageOpen`.
///
/// Borrowed, never owned: the array belongs to the renderer and is valid
/// only for the duration of the call. Copy anything you need to keep.
#[derive(Copy, Clone)]
pub struct Params<'a> {
    raw: &'a [ndspy_sys::UserParameter],
}

impl<'a> Params<'a> {
    /// # Safety
    /// `raw` must point to `count` valid `UserParameter`s that outlive
    /// `'a`, as ndspy guarantees for the duration of the call. The data
    /// referenced by each parameter's `name` and `value` pointers —
    /// including, for string parameters, the `char*` those point to —
    /// must also remain valid for `'a`.
    #[inline]
    pub unsafe fn from_raw(
        raw: *const ndspy_sys::UserParameter,
        count: c_int,
    ) -> Self {
        let raw = if raw.is_null() || count <= 0 {
            &[][..]
        } else {
            // SAFETY: the caller guarantees `count` valid entries.
            unsafe { slice::from_raw_parts(raw, count as usize) }
        };
        Self { raw }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    fn find(
        &self,
        name: &str,
        type_: u8,
    ) -> Option<&'a ndspy_sys::UserParameter> {
        self.raw.iter().find(|p| {
            if p.name.is_null()
                || p.value.is_null()
                || p.valueType as u8 != type_
            {
                return false;
            }
            // SAFETY: ndspy names are NUL-terminated C strings.
            unsafe { CStr::from_ptr(p.name) }.to_str() == Ok(name)
        })
    }

    /// A string parameter. ndspy stores these as a pointer to a
    /// `char*`, so this reads through two levels, exactly as the value
    /// is laid out.
    pub fn string(&self, name: &str) -> Option<&'a str> {
        let param = self.find(name, b's')?;
        // SAFETY: `value` addresses one `char*`, per the ndspy layout.
        let ptr = unsafe { *(param.value as *const *const c_char) };
        if ptr.is_null() {
            return None;
        }
        // SAFETY: the renderer passes NUL-terminated strings.
        unsafe { CStr::from_ptr(ptr) }.to_str().ok()
    }

    /// An integer parameter.
    pub fn i32(&self, name: &str) -> Option<i32> {
        let param = self.find(name, b'i')?;
        // SAFETY: `value` addresses one `int`.
        Some(unsafe { *(param.value as *const i32) })
    }

    /// Every string parameter, as `(name, value)`.
    ///
    /// `string`, `i32` and `f32` look a name up; this enumerates. A
    /// driver needs it when the *names* are the data -- ɴsɪ's
    /// `header.<name>` metadata passthrough, say, where `<name>` is
    /// whatever the scene chose to write into the file.
    ///
    /// Parameters that are not strings, or whose name or value is not
    /// readable, are skipped.
    pub fn strings(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.raw.iter().filter_map(|p| {
            if p.name.is_null() || p.value.is_null() || p.valueType as u8 != b's'
            {
                return None;
            }
            // SAFETY: ndspy names are NUL-terminated C strings.
            let name = unsafe { CStr::from_ptr(p.name) }.to_str().ok()?;
            // SAFETY: `value` addresses one `char*`, per the ndspy layout.
            let ptr = unsafe { *(p.value as *const *const c_char) };
            if ptr.is_null() {
                return None;
            }
            // SAFETY: the renderer passes NUL-terminated strings.
            let value = unsafe { CStr::from_ptr(ptr) }.to_str().ok()?;
            Some((name, value))
        })
    }

    /// A float parameter.
    pub fn f32(&self, name: &str) -> Option<f32> {
        let param = self.find(name, b'f')?;
        // SAFETY: `value` addresses one `float`.
        Some(unsafe { *(param.value as *const f32) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use std::ffi::CString;

    /// A `Params` view borrows the renderer's array; it must read the
    /// values back without taking ownership of anything.
    #[test]
    fn reads_string_and_integer_parameters() {
        let name = CString::new("filename").unwrap();
        let value = CString::new("render.exr").unwrap();
        let value_ptr = value.as_ptr();
        let quality_name = CString::new("quality").unwrap();
        let quality = 42i32;

        let raw = [
            ndspy_sys::UserParameter {
                name: name.as_ptr(),
                valueType: b's' as _,
                valueCount: 1,
                value: &value_ptr as *const _ as *const c_void,
                nbytes: core::mem::size_of::<*const c_char>() as _,
            },
            ndspy_sys::UserParameter {
                name: quality_name.as_ptr(),
                valueType: b'i' as _,
                valueCount: 1,
                value: &quality as *const _ as *const c_void,
                nbytes: core::mem::size_of::<i32>() as _,
            },
        ];

        // SAFETY: `raw` outlives the view.
        let params = unsafe { Params::from_raw(raw.as_ptr(), raw.len() as _) };

        assert_eq!(2, params.len());
        assert_eq!(Some("render.exr"), params.string("filename"));
        assert_eq!(Some(42), params.i32("quality"));
        assert_eq!(None, params.i32("absent"));
    }
}
