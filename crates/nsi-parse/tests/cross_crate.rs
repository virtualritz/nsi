//! Write with `nsi-intermediate`, read with `nsi-parse`, write again.
//!
//! This is the pairing's whole claim, and until now nothing exercised it
//! end to end: the writer's tests compared against 3Delight, and the
//! parser's compared against the writer for a much smaller scene. Any
//! value one emits and the other rejects shows up here.
//!
//! # What this cannot catch
//!
//! Agreement is not correctness. Two classes of defect round-trip
//! perfectly and are caught elsewhere, on purpose:
//!
//! - **A shared mistake.** Both crates spell a connection through
//!   `EdgeKind::to_attr` and `classify`, so a wrong entry there is
//!   written and read back consistently. `classifier::
//!   every_connection_the_specification_declares_is_classified` pins
//!   that table against the ɴsɪ specification instead.
//! - **A stream that is self-consistent but not ɴsɪ.** An unescaped
//!   control byte survives a write-parse-write cycle unchanged while
//!   being wrong in the file. `nsi-intermediate`'s `stream_roundtrip`
//!   compares against what 3Delight itself writes, which is the only
//!   thing that can see it.
//!
//! Both were confirmed by breaking them and watching this test stay
//! green.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, Scene, write_stream};
use nsi_parse::parse_stream;
use nsi_trait::Nsi;

fn stream_of(scene: &Scene) -> String {
    let mut out = Vec::new();
    write_stream(scene, &mut out).expect("write_stream");
    String::from_utf8(out).expect("utf-8")
}

/// Everything both crates claim to handle, in one scene.
fn build<R>(ctx: &R) -> Result<(), R::Error>
where
    R: Nsi,
    for<'call> R: Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    // Every scalar and tuple type.
    ctx.create("types", "mesh", None)?;
    ctx.set_attribute("types", &[nsi::f32!("a_float", 0.1)])?;
    ctx.set_attribute("types", &[nsi::f64!("a_double", 1.0 / 3.0)])?;
    ctx.set_attribute("types", &[nsi::i32!("an_int", -7)])?;
    ctx.set_attribute("types", &[nsi::i64!("a_long", i64::MIN)])?;
    ctx.set_attribute("types", &[nsi::string!("a_string", "plain")])?;
    ctx.set_attribute("types", &[nsi::color!("a_color", &[0.1, 0.2, 0.3])])?;
    let points = [[0.0f32, 1.0, 2.0], [3.0, 4.0, 5.0]];
    ctx.set_attribute("types", &[nsi::point_slice!("points", &points)])?;
    let vectors = [[1.0f32, 0.0, 0.0]];
    ctx.set_attribute("types", &[nsi::vector_slice!("vectors", &vectors)])?;
    let normals = [[0.0f32, 1.0, 0.0]];
    ctx.set_attribute("types", &[nsi::normal_slice!("normals", &normals)])?;
    #[rustfmt::skip]
    let m32 = [
        2.0f32, 0.0, 0.0, 0.0,
        0.0, 2.0, 0.0, 0.0,
        0.0, 0.0, 2.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    ctx.set_attribute("types", &[nsi::matrix_f32!("m32", &m32)])?;
    #[rustfmt::skip]
    let m64 = [
        1.0f64, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        1.0, 2.0, 3.0, 1.0,
    ];
    ctx.set_attribute("types", &[nsi::matrix_f64!("m64", &m64)])?;

    // Floats that discriminate the two printers.
    ctx.set_attribute("types", &[nsi::f32!("f_exp", 100_000.0f32)])?;
    ctx.set_attribute("types", &[nsi::f32!("f_tiny", 1e-7f32)])?;
    ctx.set_attribute("types", &[nsi::f64!("d_tenth", 0.1f64)])?;
    ctx.set_attribute("types", &[nsi::f64!("d_huge", 1e20f64)])?;
    ctx.set_attribute("types", &[nsi::f64!("d_neg_zero", -0.0f64)])?;

    // All three flags, together and apart.
    ctx.create("flags", "mesh", None)?;
    ctx.set_attribute(
        "flags",
        &[nsi::point_slice!("pv", &points).per_vertex()],
    )?;
    ctx.set_attribute("flags", &[nsi::f32!("pf", 1.0).per_face()])?;
    ctx.set_attribute("flags", &[nsi::f32!("li", 1.0).linear_interpolation()])?;
    ctx.set_attribute(
        "flags",
        &[nsi::normal_slice!("both", &normals)
            .per_vertex()
            .linear_interpolation()],
    )?;

    // Arrays, including the one-element case ɴsɪ marks by flag.
    ctx.create("arrays", "mesh", None)?;
    ctx.set_attribute(
        "arrays",
        &[nsi::i32_slice!("two", &[1280i32, 720])
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    )?;
    ctx.set_attribute(
        "arrays",
        &[nsi::f32_slice!("one", &[1.0f32, 2.0])
            .array_len(const { std::num::NonZeroUsize::new(1).unwrap() })],
    )?;
    let nothing: [f32; 0] = [];
    ctx.set_attribute("arrays", &[nsi::f32_slice!("empty", &nothing)])?;

    // Strings the stream has to escape, octal included.
    ctx.create("strings", "mesh", None)?;
    ctx.set_attribute("strings", &[nsi::string!("quoted", "he said \"hi\"")])?;
    ctx.set_attribute(
        "strings",
        &[nsi::string!("control", "a\u{1}b\rc\td\ne")],
    )?;
    ctx.set_attribute("strings", &[nsi::string!("backslash", "a\\b")])?;
    let many = ["one", "two"];
    ctx.set_attribute("strings", &[nsi::string_slice!("several", &many)])?;

    // `.global` is reserved: attributes but never declared.
    ctx.set_attribute(".global", &[nsi::i32!("renderatlowpriority", 1)])?;

    // Motion samples.
    ctx.create("moving", "transform", None)?;
    ctx.set_attribute_at_time("moving", 0.0, &[nsi::f64!("t", 0.0)])?;
    ctx.set_attribute_at_time("moving", 1.0 / 3.0, &[nsi::f64!("t", 1.0)])?;

    // Every connection class the specification declares, plus a
    // shader-network edge and one carrying arguments.
    for handle in [
        "attr",
        "surf",
        "disp",
        "vol",
        "lens",
        "cam",
        "scr",
        "layer",
        "layer2",
        "drv",
        "inst",
        "set",
        "shaderattr",
        "geo",
    ] {
        ctx.create(handle, "shader", None)?;
    }
    ctx.connect("moving", None, ".root", "objects", None)?;
    ctx.connect("surf", None, "attr", "surfaceshader", None)?;
    ctx.connect("disp", None, "attr", "displacementshader", None)?;
    ctx.connect("vol", None, "attr", "volumeshader", None)?;
    ctx.connect("lens", None, "cam", "lensshader", None)?;
    ctx.connect("geo", None, "inst", "sourcemodels", None)?;
    ctx.connect("geo", None, "set", "members", None)?;
    ctx.connect("set", None, "layer", "lightset", None)?;
    ctx.connect("shaderattr", None, "geo", "shaderattributes", None)?;
    ctx.connect("attr", None, "geo", "geometryattributes", None)?;
    ctx.connect("scr", None, "cam", "screens", None)?;
    ctx.connect("layer", None, "scr", "outputlayers", None)?;
    ctx.connect("drv", None, "layer", "outputdrivers", None)?;
    ctx.connect("layer2", None, "layer", "backgroundlayer", None)?;
    ctx.connect("geo", None, "attr", "bounds", None)?;
    ctx.connect("set", None, "attr", "visibility.set.subsurface", None)?;
    ctx.connect("set", None, ".global", "exclusiveshading", None)?;
    ctx.connect("surf", Some("outColor"), "disp", "inColor", None)?;
    ctx.connect(
        "attr",
        None,
        "types",
        "geometryattributes",
        Some(&[nsi::i32!("priority", 3), nsi::i32!("strength", 1)]),
    )?;
    Ok(())
}

/// Write, parse, write: the two streams must be identical.
#[test]
fn what_one_crate_writes_the_other_reads() {
    let original = Recorder::new();
    build(&original).expect("build");
    let written = stream_of(&original.into_scene());

    let reparsed = Recorder::new();
    parse_stream(written.as_bytes(), &reparsed).unwrap_or_else(|error| {
        panic!(
            "the writer emitted something the parser rejects: {error}\n\
                --- stream ---\n{written}"
        )
    });

    assert_eq!(stream_of(&reparsed.into_scene()), written);
}

/// And again, so a fixed point is a fixed point rather than a
/// coincidence of the first pass.
#[test]
fn the_round_trip_is_idempotent() {
    let original = Recorder::new();
    build(&original).expect("build");
    let once = stream_of(&original.into_scene());

    let second = Recorder::new();
    parse_stream(once.as_bytes(), &second).expect("parse");
    let twice = stream_of(&second.into_scene());

    let third = Recorder::new();
    parse_stream(twice.as_bytes(), &third).expect("parse");
    assert_eq!(stream_of(&third.into_scene()), twice);
}

/// Call order survives the round trip.
///
/// A scene's samples resolve by the order they were *set* in, and
/// `Node::time_attrs` is sorted by time -- so a writer that walked it
/// would hand the reader a scene that resolves differently from the one
/// it wrote, silently. The `t=1` call comes first here and the `t=0`
/// call last, which is the order 3Delight answers by and the opposite
/// of the timeline.
#[test]
fn the_order_the_samples_were_set_in_survives_the_round_trip() {
    let original = Recorder::new();
    original.create("a", "attributes", None).expect("create");
    original
        .set_attribute_at_time("a", 1.0, &[nsi::i32!("visibility", 0)])
        .expect("set");
    original
        .set_attribute_at_time("a", 0.0, &[nsi::i32!("visibility", 1)])
        .expect("set");
    let written = stream_of(&original.into_scene());

    let reparsed = Recorder::new();
    parse_stream(written.as_bytes(), &reparsed).expect("parse");
    let scene = reparsed.into_scene();

    assert_eq!(
        scene.node("a").expect("node").samples["visibility"]
            .iter()
            .map(|(time, _)| *time)
            .collect::<Vec<_>>(),
        vec![1.0, 0.0],
        "the t=0 call was last on both sides",
    );
    assert_eq!(
        scene
            .node("a")
            .expect("node")
            .effective("visibility")
            .expect("set at a time")
            .as_i32(),
        Some(1),
        "which is the value 3Delight renders",
    );
}
