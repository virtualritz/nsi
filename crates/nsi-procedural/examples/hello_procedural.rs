//! A procedural that puts `count` spheres in a row and says so.
//!
//! Build it with `cargo build --example hello_procedural`, then evaluate
//! it from any ɴsɪ scene:
//!
//! ```text
//! Evaluate
//!     "type" "string" 1 "dynamiclibrary"
//!     "filename" "string" 1 "target/debug/examples/libhello_procedural.so"
//!     "count" "int" 1 3
//! ```
use nsi_ffi_wrap as nsi;
use nsi_procedural::{Error, Params, Procedural, Report};

struct Row;

impl Procedural for Row {
    fn load(
        report: &Report<'_>,
        renderer_version: &str,
    ) -> Result<Self, Error> {
        report.info(&format!("hello_procedural loaded by {renderer_version}"));
        Ok(Row)
    }

    fn execute<N>(
        &self,
        nsi: &N,
        report: &Report<'_>,
        params: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
    {
        let count = params.get("count").and_then(|p| p.i32()).unwrap_or(1);
        if count < 0 {
            return Err(
                format!("count must not be negative, is {count}").into()
            );
        }
        // A particle with a width is a sphere.
        let positions = (0..count)
            .map(|index| [index as f32 * 2.0, 0.0, 0.0])
            .collect::<Vec<_>>();

        nsi.create("row", nsi::PARTICLES, None)?;
        nsi.connect("row", None, nsi::ROOT, "objects", None)?;
        nsi.set_attribute(
            "row",
            &[nsi::point3_f32_slice!("P", &positions), nsi::real_f32!("width", 1.0)],
        )?;

        report.info(&format!("hello_procedural created {count} spheres"));
        Ok(())
    }
}

nsi_procedural::declare_procedural!(Row);
