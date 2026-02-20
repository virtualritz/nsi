//! Token type for attribute and type name strings.
//!
//! The `Token` type wraps attribute/type name strings with different backing
//! implementations based on feature flags:
//!
//! - Default (`Ustr`): Interned strings with zero-cost C pointer conversion
//! - `cstring_tokens`: Uses `CString` for non-interned C strings
//!
//! Note: Unlike handles which change frequently, tokens (attribute names,
//! type names) are typically repeated many times, making interning beneficial
//! by default.

use std::ffi::c_char;

// ─── Ustr backing (interned strings, default) ───────────────────────────────

#[cfg(not(feature = "cstring_tokens"))]
mod inner {
    use super::*;
    use ustr::{Ustr, ustr};

    /// A token string backed by an interned `Ustr`.
    ///
    /// This provides O(1) equality checks and zero-cost conversion to C strings.
    /// This is the default because tokens (attribute names, type names) are
    /// typically repeated many times in a scene, making interning beneficial.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Token(Ustr);

    impl Token {
        /// Create a new token from a string slice.
        #[inline(always)]
        pub fn new(s: &str) -> Self {
            Self(ustr(s))
        }

        /// Get the token as a string slice.
        #[inline(always)]
        pub fn as_str(&self) -> &str {
            self.0.as_str()
        }

        /// Get the token as a C string pointer.
        ///
        /// The returned pointer is valid for the lifetime of the `Ustr` cache
        /// (effectively 'static for interned strings).
        #[inline(always)]
        pub fn as_char_ptr(&self) -> *const c_char {
            self.0.as_char_ptr()
        }
    }

    impl From<&str> for Token {
        #[inline(always)]
        fn from(s: &str) -> Self {
            Self::new(s)
        }
    }

    impl PartialEq<Ustr> for Token {
        fn eq(&self, other: &Ustr) -> bool {
            self.0 == *other
        }
    }
}

// ─── CString backing (owned C strings) ──────────────────────────────────────

#[cfg(feature = "cstring_tokens")]
mod inner {
    use super::*;
    use std::ffi::CString;

    /// A token string backed by an owned `CString`.
    ///
    /// This provides conversion to C strings without interning.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Token(CString);

    impl Token {
        /// Create a new token from a string slice.
        ///
        /// # Panics
        /// Panics if the string contains interior NUL bytes.
        #[inline(always)]
        pub fn new(s: &str) -> Self {
            Self(CString::new(s).expect("Token string contains NUL byte"))
        }

        /// Get the token as a string slice.
        #[inline(always)]
        pub fn as_str(&self) -> &str {
            self.0.to_str().expect("Token contains invalid UTF-8")
        }

        /// Get the token as a C string pointer.
        ///
        /// The returned pointer is valid for the lifetime of this `Token`.
        #[inline(always)]
        pub fn as_char_ptr(&self) -> *const c_char {
            self.0.as_ptr()
        }
    }

    impl From<&str> for Token {
        #[inline(always)]
        fn from(s: &str) -> Self {
            Self::new(s)
        }
    }
}

pub use inner::Token;
