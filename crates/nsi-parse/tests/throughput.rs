//! A number, not an adjective.
//!
//! "Fast" is not something a test can assert without a figure measured
//! the same way each time, so this generates a corpus, parses it into a
//! sink that does nothing, and prints the rate. Run it with
//! `--release -- --nocapture`; a debug figure is not comparable.

use nsi_parse::parse_stream;
use nsi_trait::{Action, Nsi};
use std::time::Instant;

/// A sink that accepts everything and keeps nothing, so the figure
/// measures parsing rather than recording.
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

/// A scene of `meshes` nodes, each with a points buffer, a name and a
/// connection -- the shape of a real export.
fn corpus(meshes: usize) -> Vec<u8> {
    let mut out = String::with_capacity(meshes * 512);
    out.push_str("Create \"grp\" \"transform\"\nConnect \"grp\" \"\" \".root\" \"objects\"\n");
    for mesh in 0..meshes {
        out.push_str(&format!("Create \"mesh{mesh}\" \"mesh\"\n"));
        out.push_str(&format!("SetAttribute \"mesh{mesh}\"\n  \"name\" \"string\" 1 \"object_{mesh}\"\n"));
        out.push_str("  \"P\" \"point\" 8 [ ");
        for value in 0..24 {
            out.push_str(&format!("{}.5 ", value));
        }
        out.push_str("]\n  \"nvertices\" \"int\" 1 4\n");
        out.push_str(&format!(
            "Connect \"mesh{mesh}\" \"\" \"grp\" \"objects\"\n"
        ));
    }
    out.into_bytes()
}

/// Ignored by default: it is only meaningful in a release build, and
/// this workspace does not build release without being asked. Run it
/// with `--release -- --ignored --nocapture`.
#[test]
#[ignore = "measure with --release; a debug figure is not comparable"]
fn throughput() {
    let bytes = corpus(20_000);
    let megabytes = bytes.len() as f64 / (1024.0 * 1024.0);

    // Warm the caches so the figure is steady-state.
    parse_stream(&bytes, &Sink).expect("parse");

    let start = Instant::now();
    parse_stream(&bytes, &Sink).expect("parse");
    let elapsed = start.elapsed();

    println!(
        "parsed {megabytes:.1} MiB in {elapsed:?} -- {:.0} MiB/s",
        megabytes / elapsed.as_secs_f64()
    );
}
