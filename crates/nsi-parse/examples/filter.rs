//! Filter an ɴsɪ stream: read one, drop some statements, write one.
//!
//! ```text
//! cargo run --example filter -- scene.nsi lightset > filtered.nsi
//! ```
//!
//! Reads `scene.nsi`, drops every `Connect` into the named attribute,
//! and writes the rest to standard output. The whole filter is the
//! `Nsi` implementation below: a call it forwards continues to the
//! writer, a call it does not is gone. This is 3Delight's
//! `nsicallbacks.h` -- "return true if you want the execution to
//! continue in the original context" -- with a `Result` in place of the
//! `bool`.
//!
//! The sink downstream is a [`StreamWriter`]. Point it at a
//! [`LuaWriter`](nsi_intermediate::LuaWriter) to convert the stream to
//! a script on the way, or at `nsi_ffi_wrap::Context` to render it.

use nsi_intermediate::StreamWriter;
use nsi_trait::{Action, Nsi};
use std::{env, fs, io::BufWriter, process::ExitCode};

/// Forwards everything except connections into one attribute.
struct DropConnectionsTo<N> {
    inner: N,
    attribute: String,
}

impl<N: Nsi> Nsi for DropConnectionsTo<N> {
    type Arg<'call> = N::Arg<'call>;
    type Error = N::Error;

    fn connect(
        &self,
        from: &str,
        from_attribute: Option<&str>,
        to: &str,
        to_attribute: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        if to_attribute == self.attribute {
            // Swallowed: the statement never reaches the writer.
            return Ok(());
        }
        self.inner
            .connect(from, from_attribute, to, to_attribute, args)
    }

    fn create(
        &self,
        handle: &str,
        node_type: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.inner.create(handle, node_type, args)
    }

    fn delete(
        &self,
        handle: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.inner.delete(handle, args)
    }

    fn set_attribute(
        &self,
        handle: &str,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        self.inner.set_attribute(handle, args)
    }

    fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        self.inner.set_attribute_at_time(handle, time, args)
    }

    fn delete_attribute(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<(), Self::Error> {
        self.inner.delete_attribute(handle, name)
    }

    fn disconnect(
        &self,
        from: &str,
        from_attribute: Option<&str>,
        to: &str,
        to_attribute: &str,
    ) -> Result<(), Self::Error> {
        self.inner
            .disconnect(from, from_attribute, to, to_attribute)
    }

    fn evaluate(&self, args: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        self.inner.evaluate(args)
    }

    fn render_control(
        &self,
        action: Action,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.inner.render_control(action, args)
    }
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let (Some(path), Some(attribute)) = (arguments.next(), arguments.next())
    else {
        eprintln!("usage: filter <scene.nsi> <attribute-to-drop>");
        return ExitCode::FAILURE;
    };

    let source = match fs::read(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    // The writer holds one statement at a time, so this costs the same
    // for a scene of ten nodes and one of ten million.
    // `io::stdout()` rather than its lock: the trait is `Send + Sync`,
    // and a `StdoutLock` is neither.
    let filter = DropConnectionsTo {
        inner: StreamWriter::new(BufWriter::new(std::io::stdout())),
        attribute,
    };

    if let Err(error) = nsi_parse::parse_stream(&source, &filter) {
        eprintln!("{path}: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
