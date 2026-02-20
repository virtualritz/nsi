//! Node handle type with configurable backing storage.
//!
//! The `Handle` type wraps node handle strings with different backing
//! implementations based on feature flags:
//!
//! - Default (`CString`): For C FFI, owned strings
//! - `ustr_handles`: Uses `Ustr` for zero-cost `*const c_char` + string interning

use std::ffi::c_char;

// ─── Ustr backing (interned strings) ────────────────────────────────────────

#[cfg(feature = "ustr_handles")]
mod inner {
    use super::*;
    use ustr::{Ustr, ustr};

    /// A node handle string backed by an interned `Ustr`.
    ///
    /// This provides O(1) equality checks and zero-cost conversion to C strings.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Handle(Ustr);

    impl Handle {
        /// Create a new handle from a string slice.
        #[inline(always)]
        pub fn new(s: &str) -> Self {
            Self(ustr(s))
        }

        /// Get the handle as a string slice.
        #[inline(always)]
        pub fn as_str(&self) -> &str {
            self.0.as_str()
        }

        /// Get the handle as a C string pointer.
        ///
        /// The returned pointer is valid for the lifetime of the `Ustr` cache
        /// (effectively 'static for interned strings).
        #[inline(always)]
        pub fn as_char_ptr(&self) -> *const c_char {
            self.0.as_char_ptr()
        }
    }

    impl From<&str> for Handle {
        #[inline(always)]
        fn from(s: &str) -> Self {
            Self::new(s)
        }
    }
}

// ─── CString backing (owned C strings, default) ─────────────────────────────

#[cfg(not(feature = "ustr_handles"))]
mod inner {
    use super::*;
    use std::ffi::CString;

    /// A node handle string backed by an owned `CString`.
    ///
    /// This provides zero-cost conversion to C strings without interning overhead.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Handle(CString);

    impl Handle {
        /// Create a new handle from a string slice.
        ///
        /// # Panics
        /// Panics if the string contains interior NUL bytes.
        #[inline(always)]
        pub fn new(s: &str) -> Self {
            Self(CString::new(s).expect("Handle string contains NUL byte"))
        }

        /// Get the handle as a string slice.
        #[inline(always)]
        pub fn as_str(&self) -> &str {
            self.0.to_str().expect("Handle contains invalid UTF-8")
        }

        /// Get the handle as a C string pointer.
        ///
        /// The returned pointer is valid for the lifetime of this `Handle`.
        #[inline(always)]
        pub fn as_char_ptr(&self) -> *const c_char {
            self.0.as_ptr()
        }
    }

    impl From<&str> for Handle {
        #[inline(always)]
        fn from(s: &str) -> Self {
            Self::new(s)
        }
    }
}

pub use inner::Handle;

// ─── Internal alias for backward compatibility ──────────────────────────────

pub(crate) type HandleString = Handle;
