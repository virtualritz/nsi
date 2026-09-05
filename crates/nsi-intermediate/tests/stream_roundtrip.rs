//! The Phase 2 gate: a recorded scene must replay as the same ɴsɪ
//! stream 3Delight writes for the same calls.
//!
//! `build` is generic over [`Nsi`], so the identical code drives the
//! recorder and a real 3Delight `apistream` context. That is the whole
//! point — the two streams come from one source of truth.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, write_stream};
use nsi_trait::Nsi;

/// One attribute per call, deliberately.
///
/// The recorder holds scene state, not a call log, so it cannot know
/// that three attributes arrived in one call rather than three. Setting
/// them one at a time is what makes the two streams literally
/// comparable; see `nsi_intermediate::stream`.
fn build<'ctx, R>(ctx: &R) -> Result<(), R::Error>
where
    R: Nsi + 'ctx,
    for<'call> R: Nsi<Arg<'call> = nsi::Arg<'call, 'ctx>>,
{
    ctx.create("cam", "perspectivecamera", None)?;
    ctx.set_attribute("cam", &[nsi::f32!("fov", 45.0)])?;
    ctx.set_attribute("cam", &[nsi::string!("name", "hello")])?;

    ctx.create("m", "mesh", None)?;
    let points = [[0.0f32, 0.0, 0.0], [1.0, 2.0, 3.0]];
    ctx.set_attribute("m", &[nsi::point_slice!("P", &points)])?;
    ctx.set_attribute("m", &[nsi::i32_slice!("nvertices", &[4i32])])?;
    let resolution = [1280i32, 720];
    ctx.set_attribute(
        "m",
        &[nsi::i32_slice!("resolution", &resolution)
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    )?;
    ctx.set_attribute("m", &[nsi::color!("c", &[0.1, 0.2, 0.3])])?;
    ctx.set_attribute("m", &[nsi::i64!("big", 7i64)])?;
    ctx.set_attribute("m", &[nsi::f64!("d", 0.5f64)])?;

    ctx.create("xf", "transform", None)?;

    // Matrices. `transformationmatrix` is `doublematrix`; the `f32` form
    // is `matrix`, and the two emit different type names for the same
    // sixteen numbers.
    #[rustfmt::skip]
    let m64 = [
        1.0f64, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        1.0, 2.0, 3.0, 1.0,
    ];
    ctx.set_attribute("xf", &[nsi::matrix_f64!("transformationmatrix", &m64)])?;
    #[rustfmt::skip]
    let m32 = [
        2.0f32, 0.0, 0.0, 0.0,
        0.0, 2.0, 0.0, 0.0,
        0.0, 0.0, 2.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    ctx.set_attribute("xf", &[nsi::matrix_f32!("othermatrix", &m32)])?;

    // After the static attributes, deliberately: `write_stream` emits a
    // node's `attrs` before its `time_attrs`, so a fixture that
    // interleaved them would diverge on ordering alone. See the
    // "What this is not" note in `nsi_intermediate::stream`.
    ctx.set_attribute_at_time("xf", 0.5, &[nsi::f64!("t", 1.0)])?;

    // Every non-shader connection class, so the classifier and the
    // emitter are held to being inverse over all of them rather than
    // over `objects` alone.
    ctx.create("mesh", "mesh", None)?;
    ctx.create("attr", "attributes", None)?;
    ctx.create("shader", "shader", None)?;
    ctx.create("inst", "instances", None)?;
    ctx.create("scr", "screen", None)?;
    ctx.create("layer", "outputlayer", None)?;
    ctx.create("drv", "outputdriver", None)?;

    ctx.connect("xf", None, ".root", "objects", None)?;
    ctx.connect("attr", None, "mesh", "geometryattributes", None)?;
    ctx.connect("shader", None, "attr", "surfaceshader", None)?;
    ctx.connect("mesh", None, "inst", "sourcemodels", None)?;
    ctx.connect("scr", None, "cam", "screens", None)?;
    ctx.connect("layer", None, "scr", "outputlayers", None)?;
    ctx.connect("drv", None, "layer", "outputdrivers", None)?;

    // ɴsɪ documents `Some("")` as equivalent to `None`. 3Delight writes
    // the same empty source port for both, so a recorder that read it as
    // a port name would diverge here.
    ctx.connect("mesh", Some(""), "xf", "objects", None)?;

    ctx.connect("s1", Some("outColor"), "s2", "inColor", None)?;
    Ok(())
}

/// Canonicalise a stream for comparison.
///
/// Drops 3Delight's banner and timestamp, folds its continuation lines
/// back onto the statement they belong to (it wraps long values at an
/// arbitrary width), and collapses whitespace runs. What survives is the
/// sequence of calls and their values.
fn canonicalise(stream: &str) -> Vec<String> {
    const KEYWORDS: [&str; 9] = [
        "Create",
        "SetAttribute",
        "SetAttributeAtTime",
        "Delete",
        "DeleteAttribute",
        "Connect",
        "Disconnect",
        "Evaluate",
        "RenderControl",
    ];

    let mut statements: Vec<String> = Vec::new();
    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let starts_statement = KEYWORDS
            .iter()
            .any(|k| trimmed.starts_with(k) && !line.starts_with(' '));
        if starts_statement || statements.is_empty() {
            statements.push(trimmed.to_string());
        } else {
            let last = statements.last_mut().expect("non-empty");
            last.push(' ');
            last.push_str(trimmed);
        }
    }

    statements
        .into_iter()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// The gate. Needs 3Delight, which is what makes it meaningful.
#[test]
fn recorder_replays_what_3delight_writes() {
    let path = std::env::temp_dir().join("nsi-intermediate-gate.nsi");
    let _ = std::fs::remove_file(&path);

    {
        let ctx = nsi::Context::new(Some(&[
            nsi::string!("type", "apistream"),
            nsi::string!("streamfilename", path.to_str().unwrap()),
            nsi::string!("streamformat", "nsi"),
        ]))
        .expect("could not create an apistream ɴsɪ context");
        build(&ctx).expect("3Delight build failed");
    } // Context::drop calls NSIEnd, which flushes.

    let reference =
        canonicalise(&std::fs::read_to_string(&path).expect("stream written"));

    let recorder = Recorder::new();
    build(&recorder).expect("recorder build failed");
    let mut ours = Vec::new();
    write_stream(&recorder.scene(), &mut ours).expect("write_stream");
    let ours = canonicalise(&String::from_utf8(ours).expect("utf-8"));

    assert_eq!(reference, ours, "recorded stream diverged from 3Delight");
}
