//! Arbitrary bytes into `parse_stream`: it may refuse them, but it may
//! not panic.
//!
//! The sink keeps nothing and accepts everything, so what is being
//! explored is the grammar and the lexer rather than a recorder. The
//! bytes are handed over unchanged -- building a `String` from them
//! first would constrain the fuzzer to valid UTF-8 and quietly explore
//! a fraction of the input space.

#![no_main]

use libfuzzer_sys::fuzz_target;
use nsi_parse::parse_stream;
use nsi_trait::{Action, Nsi};

/// Accepts every call, keeps nothing.
struct Sink;

impl Nsi for Sink {
    type Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>;
    type Error = core::convert::Infallible;

    fn create(
        &self,
        _: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete(
        &self,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_attribute(
        &self,
        _: &str,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_attribute_at_time(
        &self,
        _: &str,
        _: f64,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete_attribute(&self, _: &str, _: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn connect(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn disconnect(
        &self,
        _: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn evaluate(&self, _: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn render_control(
        &self,
        _: Action,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    // The only contract: it returns. An error is a fine answer to
    // nonsense; a panic is not, because a consumer embedding the parser
    // would take the process down with it.
    let _ = parse_stream(data, &Sink);
});
