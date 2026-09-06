//! The parser's gate: what this crate reads must equal what wrote it.
//!
//! A scene is recorded, written as a stream, parsed back into a second
//! recorder, and the two scenes compared. Both sides come from one
//! source, which is what makes the comparison mean something.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, write_stream};
use nsi_parse::parse_stream;
use nsi_trait::Nsi;

/// A scene exercising every type, both flag kinds, arrays and motion.
fn build<R>(ctx: &R) -> Result<(), R::Error>
where
    R: Nsi,
    for<'call> R: Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    ctx.create("cam", "perspectivecamera", None)?;
    ctx.set_attribute("cam", &[nsi::f32!("fov", 45.0)])?;
    ctx.set_attribute("cam", &[nsi::f64!("shutter", 0.1f64)])?;
    ctx.set_attribute("cam", &[nsi::i32!("n", 4)])?;
    ctx.set_attribute("cam", &[nsi::i64!("big", 9_007_199_254_740_993i64)])?;
    ctx.set_attribute("cam", &[nsi::string!("name", "he said \"hi\"\nbye")])?;

    ctx.create("m", "mesh", None)?;
    let points = [[0.0f32, 0.0, 0.0], [1.0, 2.0, 3.0]];
    ctx.set_attribute("m", &[nsi::point_slice!("P", &points)])?;
    ctx.set_attribute("m", &[nsi::color!("c", &[0.1, 0.2, 0.3])])?;
    let normals = [[0.0f32, 1.0, 0.0], [0.0, 1.0, 0.0]];
    ctx.set_attribute("m", &[nsi::normal_slice!("N", &normals).per_vertex()])?;
    ctx.set_attribute("m", &[nsi::f32!("w", 1.0).per_face()])?;

    // A flat scalar and a tuple parameter on one node: folding the
    // tuple must not disturb the scalar's values.
    ctx.set_attribute("m", &[nsi::f32!("after_tuple", 7.5)])?;

    let resolution = [1280i32, 720];
    ctx.set_attribute(
        "m",
        &[nsi::i32_slice!("resolution", &resolution)
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    )?;

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
    ctx.set_attribute_at_time("xf", 1.0 / 3.0, &[nsi::f32!("t", 1.0)])?;

    ctx.create("attr", "attributes", None)?;
    ctx.create("shader", "shader", None)?;
    ctx.connect("xf", None, ".root", "objects", None)?;
    ctx.connect("m", None, "xf", "objects", None)?;
    ctx.connect("shader", None, "attr", "surfaceshader", None)?;
    ctx.connect(
        "attr",
        None,
        "m",
        "geometryattributes",
        Some(&[nsi::i32!("priority", 3)]),
    )?;
    ctx.connect("shader", Some("outColor"), "attr", "inColor", None)?;
    Ok(())
}

fn stream_of(scene: &nsi_intermediate::Scene) -> Vec<u8> {
    let mut out = Vec::new();
    write_stream(scene, &mut out).expect("write_stream");
    out
}

/// Written, parsed, written again: the two streams must agree.
#[test]
fn a_written_stream_parses_back_into_the_same_scene() {
    let original = Recorder::new();
    build(&original).expect("build");
    let original = original.into_scene();
    let written = stream_of(&original);

    let reparsed = Recorder::new();
    parse_stream(&written, &reparsed).expect("parse");
    let reparsed = reparsed.into_scene();

    assert_eq!(
        String::from_utf8(stream_of(&reparsed)).unwrap(),
        String::from_utf8(written).unwrap(),
    );
}

/// The grammar is keyword-terminated, not line-based: 3Delight accepts a
/// whole scene on one line, and a line-oriented reader would be wrong on
/// valid input.
#[test]
fn a_scene_on_one_line_parses() {
    let source = br#"Create "a" "transform" Create "b" "mesh" SetAttribute "b" "fov" "float" 1 45 Connect "b" "" "a" "objects""#;

    let recorder = Recorder::new();
    parse_stream(source, &recorder).expect("parse");
    let scene = recorder.into_scene();

    assert_eq!(scene.len(), 2);
    assert_eq!(scene.node("b").unwrap().node_type, "mesh");
    assert_eq!(scene.edges().count(), 1);
}

/// Whitespace runs and `#` comments are skipped.
#[test]
fn comments_and_spacing_are_ignored() {
    let source = b"# a comment\n\nCreate   \"a\"   \"transform\"  # trailing\nCreate \"b\" \"mesh\"\n";

    let recorder = Recorder::new();
    parse_stream(source, &recorder).expect("parse");
    assert_eq!(recorder.into_scene().len(), 2);
}

/// A malformed stream names where it gave up, and the sink keeps what
/// came before.
#[test]
fn a_malformed_stream_reports_an_offset() {
    let source = b"Create \"a\" \"transform\"\nCreate \"b\"\n";

    let recorder = Recorder::new();
    let error = parse_stream(source, &recorder).expect_err("must fail");
    assert!(
        matches!(error, nsi_parse::Error::Syntax { .. }),
        "got {error:?}"
    );
    assert_eq!(recorder.into_scene().len(), 1, "the first statement stuck");
}

/// The sink's own refusal is carried, not swallowed.
#[test]
fn a_sink_refusal_is_reported() {
    // `b` is never created, and the recorder refuses a connection to an
    // unknown handle.
    let source = br#"Create "a" "transform" Connect "a" "" "b" "objects""#;

    let recorder = Recorder::new();
    let error = parse_stream(source, &recorder).expect_err("must fail");
    assert!(matches!(error, nsi_parse::Error::Sink(_)), "got {error:?}");
}

/// 3Delight groups a node's attributes into one `SetAttribute`, and
/// `nsi-intermediate`'s writer does not -- so a round-trip against our
/// own output never exercises a multi-parameter statement, and the
/// scratch buffers that back them are only shared within one.
///
/// Mixing a flat type with a tuple type in one statement is the case
/// that catches a fold which moves more than its own run.
#[test]
fn several_parameters_of_mixed_types_in_one_statement() {
    let source = br#"
Create "m" "mesh"
SetAttribute "m"
  "before" "float" 1 1.5
  "P" "point" 2 [ 0 0 0 1 2 3 ]
  "after" "float" 1 2.5
  "c" "color" 1 [ 0.25 0.5 0.75 ]
  "last" "float" 2 [ 8 9 ]
"#;

    let recorder = Recorder::new();
    parse_stream(source, &recorder).expect("parse");
    let scene = recorder.into_scene();
    let attrs = &scene.node("m").expect("created").attrs;

    use nsi_intermediate::OwnedData;
    assert_eq!(attrs["before"].data, OwnedData::F32(vec![1.5]));
    assert_eq!(
        attrs["P"].data,
        OwnedData::F32(vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0])
    );
    assert_eq!(
        attrs["after"].data,
        OwnedData::F32(vec![2.5]),
        "a tuple parameter must not disturb the scalars around it"
    );
    assert_eq!(attrs["c"].data, OwnedData::F32(vec![0.25, 0.5, 0.75]));
    assert_eq!(attrs["last"].data, OwnedData::F32(vec![8.0, 9.0]));
}

/// A bare word that is not a statement keyword is an error, not a
/// silently skipped line.
#[test]
fn an_unknown_statement_is_rejected() {
    let source = b"Create \"a\" \"transform\"\nNonsense \"a\"\n";

    let recorder = Recorder::new();
    let error = parse_stream(source, &recorder).expect_err("must fail");
    assert!(
        matches!(error, nsi_parse::Error::Syntax { expected, .. }
            if expected.contains("statement")),
        "got {error:?}"
    );
}

/// The escapes 3Delight actually writes.
///
/// Measured from the renderer: `\"`, `\\`, `\t` and `\n` by name, and
/// every other byte below `0x20` as three-digit octal. There is no
/// `\xHH` -- decoding that instead rejected `\001` outright, so a
/// stream carrying a tab or a carriage return in a string could not be
/// read at all.
#[test]
fn octal_escapes_are_decoded() {
    let source = b"Create \"m\" \"mesh\"\nSetAttribute \"m\"\n  \"s\" \"string\" 1 \"a\\001b\\015c\\td\\ne\"\n";

    let recorder = Recorder::new();
    parse_stream(source, &recorder).expect("parse");
    let scene = recorder.into_scene();

    use nsi_intermediate::OwnedData;
    assert_eq!(
        scene.node("m").unwrap().attrs["s"].data,
        OwnedData::String(vec![b"a\x01b\rc\td\ne".to_vec()])
    );
}

/// The declared element count is authoritative, not decorative. Given
/// `"P" "point" 1 [ 0 0 0 1 2 3 ]` the renderer warns and keeps one
/// point; ignoring the count here would silently yield two.
#[test]
fn a_count_that_disagrees_with_its_values_is_an_error() {
    let source = b"Create \"m\" \"mesh\"\nSetAttribute \"m\"\n  \"P\" \"point\" 1 [ 0 0 0 1 2 3 ]\n";

    let recorder = Recorder::new();
    assert!(matches!(
        parse_stream(source, &recorder),
        Err(nsi_parse::Error::Syntax { .. })
    ));
}

/// An error's offset must name the offending token, not the whitespace
/// or comment before it.
#[test]
fn an_error_offset_points_at_the_token() {
    let source = b"Create \"a\" \"t\"\n   Nonsense";
    let at = source
        .windows(8)
        .position(|w| w == b"Nonsense")
        .expect("present");

    let recorder = Recorder::new();
    match parse_stream(source, &recorder) {
        Err(nsi_parse::Error::Syntax { offset, .. }) => {
            assert_eq!(offset, at, "offset must be the token's own start")
        }
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

/// An operand's error must name the operand, not the token before it.
/// The keyword position was already covered; these were not, and every
/// one of them reported the previous token.
#[test]
fn an_operand_error_offset_points_at_the_operand() {
    for (source, needle) in [
        (&b"Create \"a\" 5"[..], &b"5"[..]),
        (b"Create \"a\" \"m\"\nSetAttribute \"a\"\n  \"x\" \"flaot\" 1 1\n", b"\"flaot\""),
        (b"Create \"a\" \"m\"\nSetAttribute \"a\"\n  \"x\" \"float\" 2 [ 1 oops ]\n", b"oops"),
    ] {
        let at = source
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("present");

        let recorder = Recorder::new();
        match parse_stream(source, &recorder) {
            Err(nsi_parse::Error::Syntax { offset, .. }) => assert_eq!(
                offset,
                at,
                "for {:?}",
                String::from_utf8_lossy(source)
            ),
            other => panic!("expected a syntax error, got {other:?}"),
        }
    }
}

/// 3Delight decodes one to three octal digits, C-style -- it always
/// *writes* three, but reads `\1b` too, so demanding three rejected a
/// legal stream.
#[test]
fn a_short_octal_escape_is_decoded() {
    let source = b"Create \"m\" \"mesh\"\nSetAttribute \"m\"\n  \"s\" \"string\" 1 \"a\\1b\\17c\"\n";

    let recorder = Recorder::new();
    parse_stream(source, &recorder).expect("parse");

    use nsi_intermediate::OwnedData;
    assert_eq!(
        recorder.into_scene().node("m").unwrap().attrs["s"].data,
        OwnedData::String(vec![b"a\x01b\x0fc".to_vec()])
    );
}

/// A zero-count `action` must not let the next parameter's value be read
/// as the render action.
#[test]
fn a_zero_count_action_does_not_steal_the_next_value() {
    let source = b"RenderControl\n  \"action\" \"string\" 0 [ ]\n  \"x\" \"string\" 1 \"start\"\n";

    let recorder = Recorder::new();
    assert!(
        parse_stream(source, &recorder).is_err(),
        "no usable action, so the statement is malformed"
    );
}

/// A byte at or above `0x7f` in a string *value* survives parsing.
///
/// 3Delight writes such a byte raw -- `renderdl -cat` echoes a Latin-1
/// `café.exr` back unchanged -- so this is a stream the renderer
/// produces. The parser rejected it outright with `NotUtf8`, which made
/// the crate unable to read its own renderer's output, and a file name
/// on Linux is not required to be UTF-8 in the first place.
#[test]
fn a_non_utf8_string_value_survives_parsing() {
    let mut source = Vec::new();
    source.extend_from_slice(b"Create \"d\" \"outputdriver\"\n");
    source.extend_from_slice(
        b"SetAttribute \"d\" \"imagefilename\" \"string\" 1 \"caf",
    );
    source.push(0xE9);
    source.extend_from_slice(b".exr\"\n");

    let recorder = Recorder::new();
    parse_stream(&source, &recorder).expect("3Delight writes this");

    use nsi_intermediate::OwnedData;
    let scene = recorder.into_scene();
    assert_eq!(
        scene.node("d").unwrap().attrs["imagefilename"].data,
        OwnedData::String(vec![b"caf\xE9.exr".to_vec()]),
        "the byte is preserved, not replaced with U+FFFD",
    );
}

/// The same byte in an *escaped* value, so the unescape path keeps it
/// too rather than validating at the end.
#[test]
fn a_non_utf8_byte_survives_an_escaped_value() {
    let mut source = Vec::new();
    source.extend_from_slice(b"Create \"d\" \"outputdriver\"\n");
    source.extend_from_slice(b"SetAttribute \"d\" \"s\" \"string\" 1 \"a\\tb");
    source.push(0xE9);
    source.extend_from_slice(b"\"\n");

    let recorder = Recorder::new();
    parse_stream(&source, &recorder).expect("parse");

    use nsi_intermediate::OwnedData;
    assert_eq!(
        recorder.into_scene().node("d").unwrap().attrs["s"].data,
        OwnedData::String(vec![b"a\tb\xE9".to_vec()]),
    );
}

/// An identifier is refused, in every position that is one.
///
/// Not because ɴsɪ requires it -- 3Delight accepts and echoes a Latin-1
/// handle and parameter name. The constraint is this crate's:
/// [`nsi_trait::Nsi`] takes `&str` for handles and names, so the parser
/// has nothing to carry them in and refusing beats inventing a
/// different name. Values are bytes; names are text.
///
/// The offset must name the token that failed. It pointed at the
/// *previous* one, because `Lexer::offset` reports the start of the
/// token last returned and it was read before `next_token`.
#[test]
fn a_non_utf8_identifier_is_refused_in_every_position() {
    // (source, byte offset of the opening quote of the bad token)
    let cases: [(&[u8], usize); 4] = [
        // Handle.
        (b"Create \"me\xE9sh\" \"mesh\"\n", 7),
        // Node type.
        (b"Create \"mesh\" \"me\xE9sh\"\n", 14),
        // Parameter name.
        (b"Create \"m\" \"mesh\"\nSetAttribute \"m\" \"n\xE9me\" \"int\" 1 1\n", 35),
        // Type spelling.
        (b"Create \"m\" \"mesh\"\nSetAttribute \"m\" \"n\" \"i\xE9t\" 1 1\n", 39),
    ];

    for (source, offset) in cases {
        let recorder = Recorder::new();
        let error = parse_stream(source, &recorder)
            .expect_err("an identifier must be text");
        let nsi_parse::Error::NotUtf8 { offset: reported } = error else {
            panic!("expected NotUtf8, got {error:?}");
        };
        assert_eq!(
            reported, offset,
            "the offset must name the failing token, not the one before it",
        );
    }
}
