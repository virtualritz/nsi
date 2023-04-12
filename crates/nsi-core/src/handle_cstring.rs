use std::ffi::{c_char, CString};

pub(crate) struct HandleString(CString);

impl From<&str> for HandleString {
    #[inline(always)]
    fn from(handle: &str) -> Self {
        Self(CString::new(handle).unwrap())
    }
}

impl HandleString {
    #[inline(always)]
    pub(crate) fn as_char_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }
}
