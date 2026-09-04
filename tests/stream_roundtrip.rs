//! The Phase 2 gate: a recorded scene must replay as the same ɴsɪ
//! stream 3Delight writes for the same calls.
//!
//! `build` is generic over [`Nsi`], so the identical code drives the
//! recorder and a real 3Delight `apistream` context. That is the whole
//! point — the two streams come from one source of truth.

use nsi_ffi_wrap as nsi;
use nsi_record::{Recorder, write_stream};
use nsi_trait::Nsi;

/// One attribute per call, deliberately.
///
/// The recorder holds scene state, not a call log, so it cannot know
/// that three attributes arrived in one call rather than three. Setting
/// them one at a time is what makes the two streams literally
/// comparable; see `nsi_record::stream`.
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
    ctx.set_attribute_at_time("xf", 0.5, &[nsi::f64!("t", 1.0)])?;
    ctx.connect("xf", None, ".root", "objects", None)?;
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
    let path = std::env::temp_dir().join("nsi-record-gate.nsi");
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

    let reference = canonicalise(&std::fs::read_to_string(&path).expect("stream written"));

    let recorder = Recorder::new();
    build(&recorder).expect("recorder build failed");
    let mut ours = Vec::new();
    write_stream(&recorder.scene(), &mut ours).expect("write_stream");
    let ours = canonicalise(&String::from_utf8(ours).expect("utf-8"));

    assert_eq!(reference, ours, "recorded stream diverged from 3Delight");
}
