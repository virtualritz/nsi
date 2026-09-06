//! Reading a Lua scene: it must rebuild what wrote it, and it must read
//! a scene a script *computed*, which no pattern-matcher could.
#![cfg(feature = "lua")]

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, write_lua, write_stream};
use nsi_parse::run_lua;
use nsi_trait::Nsi;

fn stream_of(scene: &nsi_intermediate::Scene) -> String {
    let mut out = Vec::new();
    write_stream(scene, &mut out).expect("write_stream");
    String::from_utf8(out).expect("utf-8")
}

/// Emitted as Lua, run back in: the two scenes must agree.
#[test]
fn a_lua_script_this_workspace_wrote_reads_back_the_same() {
    let original = Recorder::new();
    original.create("cam", "perspectivecamera", None).unwrap();
    original
        .set_attribute("cam", &[nsi::f32!("fov", 45.0)])
        .unwrap();
    original
        .set_attribute("cam", &[nsi::string!("name", "he said \"hi\"")])
        .unwrap();
    original.create("m", "mesh", None).unwrap();
    let points = [[0.0f32, 0.0, 0.0], [1.0, 2.0, 3.0]];
    original
        .set_attribute("m", &[nsi::point_slice!("P", &points)])
        .unwrap();
    original
        .set_attribute("m", &[nsi::color!("c", &[0.1, 0.2, 0.3])])
        .unwrap();
    original
        .set_attribute(
            "m",
            &[nsi::i32_slice!("res", &[1280i32, 720])
                .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
        )
        .unwrap();
    original.create("xf", "transform", None).unwrap();
    #[rustfmt::skip]
    let matrix = [
        1.0f64, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        1.0, 2.0, 3.0, 1.0,
    ];
    original
        .set_attribute(
            "xf",
            &[nsi::matrix_f64!("transformationmatrix", &matrix)],
        )
        .unwrap();
    original
        .set_attribute_at_time("xf", 0.5, &[nsi::f32!("t", 1.0)])
        .unwrap();
    original
        .connect("xf", None, ".root", "objects", None)
        .unwrap();
    original.connect("m", None, "xf", "objects", None).unwrap();
    let original = original.into_scene();

    let mut script = Vec::new();
    write_lua(&original, &mut script).expect("write_lua");
    let script = String::from_utf8(script).expect("utf-8");

    let rebuilt = Recorder::new();
    run_lua(script.as_bytes(), &rebuilt)
        .unwrap_or_else(|e| panic!("{e}\n--- script ---\n{script}"));

    assert_eq!(stream_of(&rebuilt.into_scene()), stream_of(&original));
}

/// A script that *computes* its scene. Reading Lua means running it;
/// this is the case a pattern-matcher over the source could never do.
#[test]
fn a_computed_scene_is_read() {
    let source = r#"
        nsi.Create("group", "transform")
        nsi.Connect("group", "", ".root", "objects")
        for i = 1, 5 do
            local handle = "mesh" .. i
            nsi.Create(handle, "mesh")
            nsi.SetAttribute(handle, {name="index", data=i})
            nsi.Connect(handle, "", "group", "objects")
        end
    "#;

    let recorder = Recorder::new();
    run_lua(source.as_bytes(), &recorder).expect("run");
    let scene = recorder.into_scene();

    assert_eq!(scene.len(), 6, "one group and five meshes");
    for i in 1..=5 {
        let node = scene.node(&format!("mesh{i}")).expect("created");
        assert_eq!(
            node.attrs["index"].data,
            nsi_intermediate::OwnedData::I32(vec![i]),
            "an untyped Lua integer is an ɴsɪ int"
        );
    }
}

/// ɴsɪ accepts parameters variadically or as one table of them.
#[test]
fn both_parameter_shapes_are_accepted() {
    let source = r#"
        nsi.Create("m", "mesh")
        nsi.SetAttribute("m", {name="a", data=1.5}, {name="b", data=2})
        nsi.SetAttribute("m", { {name="c", data="x"}, {name="d", data=4} })
    "#;

    let recorder = Recorder::new();
    run_lua(source.as_bytes(), &recorder).expect("run");
    let scene = recorder.into_scene();
    let attrs = &scene.node("m").expect("created").attrs;

    assert!(["a", "b", "c", "d"].iter().all(|k| attrs.contains_key(*k)));
}

/// A script error surfaces as an error, not a panic.
#[test]
fn a_broken_script_is_an_error() {
    let recorder = Recorder::new();
    let error = run_lua("this is not lua".as_bytes(), &recorder)
        .expect_err("must fail");
    assert!(matches!(error, nsi_parse::Error::Lua(_)), "got {error:?}");
}

/// A sink refusal is carried out of the script rather than stringified.
#[test]
fn a_sink_refusal_escapes_the_script() {
    let recorder = Recorder::new();
    let error = run_lua(
        r#"nsi.Connect("ghost", "", ".root", "objects")"#.as_bytes(),
        &recorder,
    )
    .expect_err("must fail");
    assert!(matches!(error, nsi_parse::Error::Sink(_)), "got {error:?}");
}

/// A sink refusal must stop the script. Recording the error and letting
/// it run on contradicted what `Error::Sink` promises, and the sink kept
/// receiving calls after the refusal.
#[test]
fn a_refusal_stops_the_script() {
    let source = r#"
        nsi.Create("a", "transform")
        nsi.Connect("ghost", "", ".root", "objects")
        nsi.Create("after", "transform")
    "#;

    let recorder = Recorder::new();
    let error = run_lua(source.as_bytes(), &recorder).expect_err("must fail");
    assert!(matches!(error, nsi_parse::Error::Sink(_)), "got {error:?}");
    assert!(
        recorder.into_scene().node("after").is_none(),
        "the script must not run past the refusal"
    );
}

/// `nsi.RenderControl` is in the renderer's own table, and a real scene
/// ends with one.
#[test]
fn render_control_is_bound() {
    let recorder = Recorder::new();
    run_lua(
        r#"nsi.RenderControl({name="action", data="start"})"#.as_bytes(),
        &recorder,
    )
    .expect("run");
    assert_eq!(
        recorder.render_state(),
        nsi_intermediate::RenderState::Running
    );
}

/// ɴsɪ's `recursive` rides on `nsi.Delete`'s parameter list; dropping it
/// turned a recursive delete into a plain one.
#[test]
fn delete_carries_its_parameters() {
    let source = r#"
        nsi.Create("attr", "attributes")
        nsi.Create("shader", "shader")
        nsi.Connect("shader", "", "attr", "surfaceshader")
        nsi.Delete("attr", {name="recursive", data=1})
    "#;

    let recorder = Recorder::new();
    run_lua(source.as_bytes(), &recorder).expect("run");
    let scene = recorder.into_scene();

    assert!(scene.node("attr").is_none());
    assert!(
        scene.node("shader").is_none(),
        "the recursive flag must survive"
    );
}

/// Short tuple data is a malformed parameter, not something to truncate:
/// `as_chunks` would turn a two-value point into an empty one.
#[test]
fn short_tuple_data_is_refused() {
    let recorder = Recorder::new();
    let error = run_lua(
        r#"nsi.Create("m","mesh") nsi.SetAttribute("m",{name="P",data={0,0},type=nsi.TypePoint})"#.as_bytes(),
        &recorder,
    )
    .expect_err("must fail");
    assert!(matches!(error, nsi_parse::Error::Lua(_)), "got {error:?}");
}

/// The Lua path runs untrusted code, so it must not be the unguarded
/// twin of the stream reader. Both of these previously got through: a
/// NUL panicked at the ɴsɪ boundary, and a large integer became a
/// different number -- the exact corruption `write_lua` refuses to emit.
#[test]
fn the_lua_reader_refuses_what_the_stream_reader_refuses() {
    for (source, what) in [
        (
            r#"nsi.Create("m","mesh") nsi.SetAttribute("m",{name="s",data={"a\0b"},type=nsi.TypeString})"#,
            "an interior NUL",
        ),
        (
            r#"nsi.Create("m","mesh") nsi.SetAttribute("m",{name="i",data={1099511627776},type=nsi.TypeInteger})"#,
            "an integer that does not fit 32 bits",
        ),
        (
            r#"nsi.Create("m","mesh") nsi.SetAttribute("m",{name="i",arraylength=2,data={1,2,3},type=nsi.TypeInteger})"#,
            "data that does not divide by arraylength",
        ),
    ] {
        let recorder = Recorder::new();
        assert!(
            run_lua(source.as_bytes(), &recorder).is_err(),
            "must refuse {what}"
        );
    }
}

/// The Lua front end must keep a non-UTF-8 byte, exactly as the stream
/// reader does.
///
/// 3Delight's Lua accepts `"\xE9"` in a string literal and hands the
/// raw byte to ɴsɪ. This reader used `as_string_lossy`, so it recorded
/// U+FFFD instead -- the same "render writes to a file the scene did
/// not name" failure the stream reader had, reachable through the
/// other front end of the same crate.
#[test]
fn a_non_utf8_byte_survives_a_lua_script() {
    let source = br#"nsi.Create("d","outputdriver")
nsi.SetAttribute("d",{name="imagefilename", data="caf\xE9.exr"})"#;

    let recorder = Recorder::new();
    run_lua(source, &recorder).expect("run");

    use nsi_intermediate::OwnedData;
    assert_eq!(
        recorder.into_scene().node("d").unwrap().attrs["imagefilename"].data,
        OwnedData::String(vec![b"caf\xE9.exr".to_vec()]),
        "the byte survives; U+FFFD would be `ef bf bd`",
    );
}

/// And a raw byte in the chunk itself, which is why `run_lua` takes
/// bytes: `write_lua` emits such a file, so this crate could not read
/// back what it had just written.
#[test]
fn a_raw_byte_in_the_chunk_survives() {
    let mut source = Vec::new();
    source.extend_from_slice(b"nsi.Create(\"d\",\"outputdriver\")\n");
    source.extend_from_slice(b"nsi.SetAttribute(\"d\",{name=\"f\", data=\"caf");
    source.push(0xE9);
    source.extend_from_slice(b".exr\"})");

    let recorder = Recorder::new();
    run_lua(&source, &recorder).expect("run");

    use nsi_intermediate::OwnedData;
    assert_eq!(
        recorder.into_scene().node("d").unwrap().attrs["f"].data,
        OwnedData::String(vec![b"caf\xE9.exr".to_vec()]),
    );
}
