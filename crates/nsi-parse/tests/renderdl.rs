//! The gate that matters: parse what the renderer wrote.
//!
//! `nsi-intermediate`'s writer emits one attribute per statement;
//! 3Delight groups them, wraps long values across lines, and chooses its
//! own float spellings. So a round-trip against our own output proves
//! much less than reading the renderer's, which is what this does.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, write_stream};
use nsi_parse::parse_stream;

/// Fold continuation lines and drop the banner, so the comparison is on
/// statements rather than on layout.
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

/// The gate. Needs 3Delight, which is what makes it meaningful.
#[test]
fn what_3delight_writes_parses_back_into_the_same_scene() {
    let path = std::env::temp_dir().join("nsi-parse-gate.nsi");
    let _ = std::fs::remove_file(&path);

    {
        let ctx = nsi::Context::new(Some(&[
            nsi::string!("type", "apistream"),
            nsi::string!("streamfilename", path.to_str().unwrap()),
            nsi::string!("streamformat", "nsi"),
        ]))
        .expect("could not create an apistream ɴsɪ context");

        // Several attributes in one call, which is what a renderer
        // writes and what our own writer never produces.
        ctx.create("cam", "perspectivecamera", None);
        ctx.set_attribute(
            "cam",
            &[
                nsi::f32!("fov", 45.0),
                nsi::f64!("shutter", 0.1f64),
                nsi::string!("name", "hero"),
            ],
        );

        ctx.create("m", "mesh", None);
        // Long enough that 3Delight wraps it across lines.
        let points: Vec<[f32; 3]> =
            (0..40).map(|i| [i as f32, 0.5, -1.25]).collect();
        ctx.set_attribute(
            "m",
            &[
                nsi::point_slice!("P", &points),
                nsi::i32_slice!("nvertices", &[4i32]),
                nsi::color!("c", &[0.1, 0.2, 0.3]),
            ],
        );

        ctx.create("xf", "transform", None);
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
        );
        ctx.set_attribute_at_time("xf", 0.5, &[nsi::f64!("t", 1.0)]);

        ctx.connect("xf", None, ".root", "objects", None);
        ctx.connect("m", None, "xf", "objects", None);
    } // Context::drop calls NSIEnd, which flushes.

    let written = std::fs::read(&path).expect("stream written");

    let recorder = Recorder::new();
    parse_stream(&written, &recorder).unwrap_or_else(|e| {
        panic!(
            "could not parse 3Delight's own stream: {e}\n--- stream ---\n{}",
            String::from_utf8_lossy(&written)
        )
    });

    let mut ours = Vec::new();
    write_stream(&recorder.into_scene(), &mut ours).expect("write_stream");

    // Our writer splits one call per attribute, so compare the parsed
    // scene against the *reference* only after both are canonicalised
    // through the same statement folding.
    let reference = canonicalise(&String::from_utf8_lossy(&written));
    let reparsed = canonicalise(&String::from_utf8_lossy(&ours));

    // Every value 3Delight wrote must appear, in order, in what we
    // re-emit -- grouping aside.
    let reference_values = statement_values(&reference);
    let reparsed_values = statement_values(&reparsed);
    assert_eq!(reference_values, reparsed_values);
}

/// Statements reduced to `(keyword, operands, parameter tokens)`, so the
/// comparison ignores how attributes were grouped into calls.
fn statement_values(statements: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for statement in statements {
        let mut tokens = statement.split_whitespace();
        let keyword = tokens.next().unwrap_or_default();
        match keyword {
            "SetAttribute" | "SetAttributeAtTime" => {
                // Split the parameter run into one entry per parameter,
                // so grouping stops mattering.
                let rest: Vec<&str> = tokens.collect();
                let head = if keyword == "SetAttributeAtTime" {
                    2
                } else {
                    1
                };
                let handle = rest[..head].join(" ");
                let mut current = String::new();
                for token in &rest[head..] {
                    if token.starts_with('"')
                        && token.ends_with('"')
                        && current.matches('"').count() >= 4
                    {
                        out.push(format!("{keyword} {handle} {current}"));
                        current.clear();
                    }
                    if !current.is_empty() {
                        current.push(' ');
                    }
                    current.push_str(token);
                }
                if !current.is_empty() {
                    out.push(format!("{keyword} {handle} {current}"));
                }
            }
            _ => out.push(statement.clone()),
        }
    }
    out
}
