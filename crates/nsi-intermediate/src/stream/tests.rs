//! Tests for [`super`].
//!
//! Separate file per the workspace rule: source files do not grow
//! inline `#[cfg(test)]` modules.

use super::{format_f64, quoted};
use crate::{OwnedArg, OwnedData, Scene, write_stream};
use nsi_trait::Type;

/// The values 3Delight writes, captured from its own `apistream`
/// output. Rust's `Display` disagrees with every one of the first
/// four, so this is what stops the emitter drifting back.
#[test]
fn doubles_format_the_way_3delight_writes_them() {
    for (value, expected) in [
        (0.1f64, "0.10000000000000001"),
        (1.0 / 3.0, "0.33333333333333331"),
        (1e-7, "9.9999999999999995e-08"),
        (1e20, "1e+20"),
        (0.5, "0.5"),
        (1.0, "1"),
        (-0.0, "-0"),
        (0.0, "0"),
    ] {
        assert_eq!(format_f64(value), expected, "for {value}");
    }
}

/// ɴsɪ's `.global` "doesn't need to be created using NSICreate",
/// and 3Delight declares neither reserved handle. Emitting one
/// produces a `Create ".global" ""` that no renderer wrote -- and
/// every real scene sets `.global`.
#[test]
fn the_reserved_handles_are_never_declared() {
    let mut scene = Scene::default();
    scene
        .set_attribute(
            crate::GLOBAL,
            vec![OwnedArg {
                name: "renderatlowpriority".to_string(),
                type_tag: Type::I32,
                array_length: 1,
                flags: 0,
                data: OwnedData::I32(vec![1]),
            }],
        )
        .unwrap();

    let mut out = Vec::new();
    write_stream(&scene, &mut out).expect("write");
    let text = String::from_utf8(out).expect("utf-8");

    assert!(
        !text.contains("Create"),
        "no Create for a reserved handle:\n{text}"
    );
    assert!(text.contains("SetAttribute \".global\""));
}

/// 3Delight writes an empty slice as `[ ]`, not as nothing. The rule
/// is "exactly one scalar is bare", not "more than one is
/// bracketed".
#[test]
fn an_empty_slice_still_brackets() {
    let mut scene = Scene::default();
    scene.create("m", "mesh").unwrap();
    scene
        .set_attribute(
            "m",
            vec![OwnedArg {
                name: "empty".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(Vec::new()),
            }],
        )
        .unwrap();

    let mut out = Vec::new();
    write_stream(&scene, &mut out).expect("write");
    let text = String::from_utf8(out).expect("utf-8");

    assert!(text.contains("[ ]"), "empty slice brackets:\n{text}");
}

/// A quote closes the literal and a newline ends the statement, so
/// an unescaped value turns into parseable ɴsɪ. The stream is a
/// persisted, cross-language format; this is data corruption, not
/// cosmetics.
#[test]
fn a_string_cannot_inject_a_statement() {
    let injected = "say \"hi\"\nCreate \"evil\" \"mesh\"";
    let escaped = quoted(injected);

    assert!(!escaped.contains('\n'), "no raw newline: {escaped}");
    assert_eq!(
        escaped.matches('"').count() - escaped.matches("\\\"").count(),
        2,
        "only the delimiters are unescaped quotes"
    );
}

/// The same, end to end: a handle and a value both carrying a quote
/// and a newline must leave the stream one statement per line.
#[test]
fn a_recorded_scene_with_hostile_strings_stays_one_statement_a_line() {
    let mut scene = Scene::default();
    scene.create("me\"ss\ny", "mesh").expect("new handle");
    scene
        .set_attribute(
            "me\"ss\ny",
            vec![OwnedArg {
                name: "na\"me".to_string(),
                type_tag: Type::String,
                array_length: 1,
                flags: 0,
                data: OwnedData::String(vec![
                    "Create \"evil\" \"mesh\"".to_string(),
                ]),
            }],
        )
        .unwrap();

    let mut out = Vec::new();
    write_stream(&scene, &mut out).expect("write");
    let text = String::from_utf8(out).expect("utf-8");

    assert_eq!(
        text.lines().filter(|l| l.starts_with("Create ")).count(),
        1,
        "exactly one Create statement:\n{text}"
    );

    // And every quote on every line is either a delimiter or
    // escaped, so a reader tokenises the line we meant to write.
    for line in text.lines() {
        let bare = line.replace("\\\\", "").matches('"').count()
            - line.replace("\\\\", "").matches("\\\"").count();
        assert_eq!(bare % 2, 0, "unbalanced quotes in {line:?}");
        assert!(
            !line.trim_start().starts_with("evil"),
            "a value escaped its literal: {line:?}"
        );
    }
}
