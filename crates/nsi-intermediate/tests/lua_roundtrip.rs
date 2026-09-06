//! The Lua gate: a script this crate emits must rebuild the same scene.
//!
//! `renderdl -lua -cat` interprets a Lua script and writes the ɴsɪ
//! stream it produces. So the script is fed back through 3Delight and
//! the result compared against this crate's own stream for the same
//! scene. Both sides come from one recorded scene, which is what makes
//! the comparison mean something.
#![cfg(feature = "lua")]

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, write_lua, write_stream};
use nsi_trait::Nsi;
use std::process::Command;

/// A scene using only the types ɴsɪ's Lua binding can express.
///
/// No `double`, `int64` or pointer: Lua has no name for them, and
/// `write_lua` refuses rather than degrading them. See `lua_refuses_*`.
fn build<R>(ctx: &R) -> Result<(), R::Error>
where
    R: Nsi,
    for<'call> R: Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    ctx.create("cam", "perspectivecamera", None)?;
    ctx.set_attribute("cam", &[nsi::f32!("fov", 45.0)])?;
    ctx.set_attribute("cam", &[nsi::string!("name", "hero \"cam\"")])?;

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

    ctx.create("xf", "transform", None)?;
    #[rustfmt::skip]
    let matrix = [
        1.0f64, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        1.0, 2.0, 3.0, 1.0,
    ];
    ctx.set_attribute(
        "xf",
        &[nsi::matrix_f64!("transformationmatrix", &matrix)],
    )?;

    ctx.create("attr", "attributes", None)?;
    ctx.create("shader", "shader", None)?;

    // A string with escapes 3Delight round-trips.
    ctx.set_attribute("m", &[nsi::string!("tricky", "a\\b\nc\"d")])?;

    // An empty numeric slice, which Lua accepts (unlike an empty string
    // array, which aborts the renderer -- see `lua_refuses_*`).
    let nothing: [f32; 0] = [];
    ctx.set_attribute("m", &[nsi::f32_slice!("empty", &nothing)])?;

    // `array_len(1)` is a real one-element array.
    ctx.set_attribute(
        "m",
        &[nsi::f32_slice!("one_array", &[1.0f32, 2.0])
            .array_len(const { std::num::NonZeroUsize::new(1).unwrap() })],
    )?;

    // A motion sample, at a time that discriminates float printers.
    ctx.set_attribute_at_time("xf", 1.0 / 3.0, &[nsi::f32!("t", 1.0)])?;

    // `.global` is reserved: it takes attributes and is never created.
    ctx.set_attribute(".global", &[nsi::i32!("renderatlowpriority", 1)])?;

    ctx.connect("xf", None, ".root", "objects", None)?;
    ctx.connect("m", None, "xf", "objects", None)?;
    ctx.connect("shader", None, "attr", "surfaceshader", None)?;

    // A connection carrying arguments.
    ctx.connect(
        "attr",
        None,
        "m",
        "geometryattributes",
        Some(&[nsi::i32!("priority", 3)]),
    )?;
    Ok(())
}

/// Fold 3Delight's continuation lines and drop its banner, exactly as
/// the stream gate does.
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
        let starts = KEYWORDS
            .iter()
            .any(|k| trimmed.starts_with(k) && !line.starts_with(' '));
        if starts || statements.is_empty() {
            statements.push(trimmed.to_string());
        } else {
            statements.last_mut().expect("non-empty").push(' ');
            statements.last_mut().expect("non-empty").push_str(trimmed);
        }
    }

    statements
        .into_iter()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn renderdl() -> Option<std::path::PathBuf> {
    let root = std::env::var("DELIGHT").ok()?;
    let path = std::path::Path::new(&root).join("bin").join("renderdl");
    path.exists().then_some(path)
}

/// The gate. Needs 3Delight, which is what makes it meaningful.
#[test]
fn a_lua_script_rebuilds_the_recorded_scene() {
    let Some(renderdl) = renderdl() else {
        panic!("DELIGHT must point at a 3Delight install for this gate");
    };

    let recorder = Recorder::new();
    build(&recorder).expect("recorder build failed");
    let scene = recorder.into_scene();

    let script = std::env::temp_dir().join("nsi-intermediate-gate.lua");
    let mut lua = Vec::new();
    write_lua(&scene, &mut lua).expect("write_lua");
    std::fs::write(&script, &lua).expect("script written");

    let output = Command::new(&renderdl)
        .args(["-lua", "-cat"])
        .arg(&script)
        .output()
        .expect("renderdl ran");
    let replayed = String::from_utf8_lossy(&output.stdout);

    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("ERROR"),
        "3Delight rejected the script:\n{}\n--- script ---\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&lua),
    );

    let mut ours = Vec::new();
    write_stream(&scene, &mut ours).expect("write_stream");

    assert_eq!(
        canonicalise(&String::from_utf8(ours).expect("utf-8")),
        canonicalise(&replayed),
        "the Lua script rebuilt a different scene\n--- script ---\n{}",
        String::from_utf8_lossy(&lua),
    );
}

/// ɴsɪ's Lua binding has no `nsi.TypeDouble`, no `nsi.TypeInt64` and no
/// pointer type. Emitting such an attribute untyped would silently turn
/// a double into a float and a large integer into a different number,
/// so it is refused instead.
#[test]
fn lua_refuses_what_it_cannot_express() {
    use nsi_intermediate::LuaError;

    // Types: `nsi.TypeDouble` and `nsi.TypeInt64` do not exist.
    for arg in [
        nsi::f64!("a_double", 0.5f64),
        nsi::i64!("a_big_int", 9_007_199_254_740_993i64),
    ] {
        let error = refuse(arg);
        assert!(
            matches!(error, LuaError::Inexpressible { .. }),
            "got {error:?}"
        );
    }

    // Flags: a parameter table has nowhere to put them, and a
    // per-vertex normal emitted without its flag is a different surface.
    let normals = [[0.0f32, 1.0, 0.0], [0.0, 1.0, 0.0]];
    let error = refuse(nsi::normal_slice!("N", &normals).per_vertex());
    assert!(
        matches!(error, LuaError::InexpressibleFlags { .. }),
        "got {error:?}"
    );

    // An empty string array aborts the renderer outright.
    let no_strings: [&str; 0] = [];
    let error = refuse(nsi::string_slice!("names", &no_strings));
    assert!(
        matches!(error, LuaError::EmptyStringArray { .. }),
        "got {error:?}"
    );
}

/// Record one argument and expect `write_lua` to refuse it.
fn refuse(arg: nsi::Arg<'_, 'static>) -> nsi_intermediate::LuaError {
    let recorder = Recorder::new();
    recorder.create("m", "mesh", None).unwrap();
    recorder.set_attribute("m", &[arg]).unwrap();

    let mut out = Vec::new();
    write_lua(&recorder.into_scene(), &mut out).expect_err("must refuse")
}
