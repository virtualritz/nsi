use std::ffi::c_char;
use ustr::Ustr;

pub(crate) struct HandleString(Ustr);

impl From<&str> for HandleString {
    #[inline(always)]
    fn from(handle: &str) -> Self {
        Self(Ustr::from(handle))
    }
}

impl HandleString {
    #[inline(always)]
    pub(crate) fn as_char_ptr(&self) -> *const c_char {
        self.0.as_char_ptr()
    }
}
