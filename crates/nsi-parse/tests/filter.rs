//! Filtering a stream: read with this crate, write with
//! `nsi-intermediate`, and put something in between.
//!
//! The gate is that an identity filter changes nothing, and that the
//! two properties a filter has and a `Scene` does not -- repeated
//! calls and their order -- survive the trip.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{LuaWriter, Recorder, Scene, StreamWriter};
use nsi_parse::parse_stream;
use nsi_trait::{Action, Nsi};

/// A small scene: two nodes, an edge, and one attribute set twice.
fn build<R>(sink: &R) -> Result<(), R::Error>
where
    R: Nsi,
    for<'call> R: Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    sink.create("cam", "perspectivecamera", None)?;
    sink.set_attribute("cam", &[nsi::f32!("fov", 45.0)])?;
    // The same attribute again: a `Scene` keeps the last value, a
    // stream keeps both calls.
    sink.set_attribute("cam", &[nsi::f32!("fov", 60.0)])?;
    sink.create("xf", "transform", None)?;
    sink.set_attribute_at_time("xf", 0.5, &[nsi::f32!("t", 1.0)])?;
    sink.connect("xf", None, ".root", "objects", None)?;
    sink.connect("cam", Some("out"), "xf", "objects", None)?;
    sink.delete_attribute("cam", "fov")?;
    sink.evaluate(&[nsi::string!("filename", "a.nsi")])?;
    Ok(())
}

/// Every call forwarded, unchanged.
struct Identity<N>(N);

/// Every call forwarded except `connect`, which is swallowed --
/// 3Delight's `nsicallbacks` returning `false`.
struct NoConnections<N>(N);

fn scene_of(stream: &[u8]) -> Scene {
    let recorder = Recorder::new();
    parse_stream(stream, &recorder).expect("parse");
    recorder.into_scene()
}

fn fixture() -> Vec<u8> {
    let writer = StreamWriter::new(Vec::new());
    build(&writer).expect("build");
    writer.into_inner().expect("into_inner")
}

/// The mechanism: a stream in, the same stream's meaning out.
#[test]
fn an_identity_filter_changes_nothing() {
    let source = fixture();

    let filtered = {
        let filter = Identity(StreamWriter::new(Vec::new()));
        parse_stream(&source, &filter).expect("filter");
        filter.0.into_inner().expect("into_inner")
    };

    assert_eq!(
        String::from_utf8(filtered.clone()).unwrap(),
        String::from_utf8(source.clone()).unwrap(),
        "an identity filter is byte-identical for a stream this crate wrote",
    );
    assert_eq!(scene_of(&filtered), scene_of(&source));
}

/// The property that distinguishes a filter from `write_stream`: the
/// second `SetAttribute` on one name is a call, not a value a scene
/// would coalesce away.
#[test]
fn repeated_calls_survive_in_order() {
    let text = String::from_utf8(fixture()).unwrap();

    assert_eq!(
        text.matches("SetAttribute \"cam\"").count(),
        2,
        "both calls, where a scene keeps one value",
    );
    assert!(
        text.find("45").unwrap() < text.find("60").unwrap(),
        "and in the order they were made",
    );

    // The same calls through a `Scene` keep the net state instead: two
    // sets and a delete of one name leave *nothing* to write, where
    // the stream carries all three calls. That is the difference
    // between the two writers, not an accident of this fixture.
    let recorder = Recorder::new();
    build(&recorder).expect("build");
    let mut scene_written = Vec::new();
    nsi_intermediate::write_stream(&recorder.into_scene(), &mut scene_written)
        .expect("write_stream");
    assert_eq!(
        String::from_utf8(scene_written)
            .unwrap()
            .matches("SetAttribute \"cam\"")
            .count(),
        0,
        "the delete undid both sets, and a scene records only the state",
    );
    assert!(text.contains("DeleteAttribute \"cam\" \"fov\""));
}

/// A filter drops a call by not forwarding it, and nothing else moves.
#[test]
fn a_swallowed_call_is_absent_from_the_output() {
    let source = fixture();

    let filter = NoConnections(StreamWriter::new(Vec::new()));
    parse_stream(&source, &filter).expect("filter");
    let filtered = filter.0.into_inner().expect("into_inner");
    let text = String::from_utf8(filtered.clone()).unwrap();

    assert!(!text.contains("Connect"), "the swallowed statement is gone");
    assert!(text.contains("Create \"cam\""), "the rest is not");
    assert!(scene_of(&filtered).edges().next().is_none());
    assert!(scene_of(&source).edges().next().is_some());
}

/// `RenderControl` carries its action as a parameter in the stream and
/// as an enum in the trait. Written from the enum, read back as the
/// same one, and exactly once.
#[test]
fn a_render_control_round_trips_its_action() {
    let writer = StreamWriter::new(Vec::new());
    writer
        .render_control(Action::Synchronize, None)
        .expect("render_control");
    let text = String::from_utf8(writer.into_inner().unwrap()).unwrap();

    assert_eq!(text.matches("\"action\"").count(), 1, "written once");

    let recorder = Recorder::new();
    parse_stream(text.as_bytes(), &recorder).expect("parse");
    assert_eq!(recorder.render_state(), nsi_intermediate::RenderState::Idle);
}

/// An `"action"` parameter that arrives in the argument list is
/// dropped, so the statement does not carry two of them.
#[test]
fn a_redundant_action_parameter_is_not_written_twice() {
    let writer = StreamWriter::new(Vec::new());
    writer
        .render_control(Action::Start, Some(&[nsi::string!("action", "stop")]))
        .expect("render_control");
    let text = String::from_utf8(writer.into_inner().unwrap()).unwrap();

    assert_eq!(text.matches("\"action\"").count(), 1);
    assert!(text.contains("\"start\""), "the typed action wins");
    assert!(!text.contains("\"stop\""));
}

/// The Lua writer says the same thing as the stream writer.
#[test]
#[cfg(feature = "lua")]
fn the_lua_writer_agrees_with_the_stream_writer() {
    let lua_writer = LuaWriter::new(Vec::new());
    build(&lua_writer).expect("build");
    let script = lua_writer.into_inner().expect("into_inner");

    let recorder = Recorder::new();
    nsi_parse::run_lua(&script, &recorder).expect("run_lua");

    assert_eq!(recorder.into_scene(), scene_of(&fixture()));
}

/// ɴsɪ's `NSICreate` takes optional parameters and 3Delight writes
/// them, so a filter must carry them through. A `Scene` drops them,
/// which is why this is the writer's business and not the recorder's.
#[test]
fn a_create_carries_its_parameters() {
    let writer = StreamWriter::new(Vec::new());
    writer
        .create("p", "procedural", Some(&[nsi::string!("type", "lua")]))
        .expect("create");
    let text = String::from_utf8(writer.into_inner().unwrap()).unwrap();

    assert!(text.starts_with("Create \"p\" \"procedural\"\n"));
    assert!(text.contains("\"type\" \"string\" 1 \"lua\""));
}

/// Lua's `nsi.Create` takes a handle and a type and nothing else, so a
/// create with parameters is refused rather than written without them.
#[test]
fn the_lua_writer_refuses_a_create_with_parameters() {
    let writer = LuaWriter::new(Vec::new());
    let refused = writer
        .create("p", "procedural", Some(&[nsi::string!("type", "lua")]))
        .expect_err("a create with parameters has no Lua spelling");

    assert!(format!("{refused}").contains("nsi.Create"));
}

macro_rules! forward {
    ($outer:ident) => {
        impl<N> Nsi for $outer<N>
        where
            N: Nsi,
        {
            type Arg<'call> = N::Arg<'call>;
            type Error = N::Error;

            fn create(
                &self,
                handle: &str,
                node_type: &str,
                args: Option<&[Self::Arg<'_>]>,
            ) -> Result<(), Self::Error> {
                self.0.create(handle, node_type, args)
            }

            fn delete(
                &self,
                handle: &str,
                args: Option<&[Self::Arg<'_>]>,
            ) -> Result<(), Self::Error> {
                self.0.delete(handle, args)
            }

            fn set_attribute(
                &self,
                handle: &str,
                args: &[Self::Arg<'_>],
            ) -> Result<(), Self::Error> {
                self.0.set_attribute(handle, args)
            }

            fn set_attribute_at_time(
                &self,
                handle: &str,
                time: f64,
                args: &[Self::Arg<'_>],
            ) -> Result<(), Self::Error> {
                self.0.set_attribute_at_time(handle, time, args)
            }

            fn delete_attribute(
                &self,
                handle: &str,
                name: &str,
            ) -> Result<(), Self::Error> {
                self.0.delete_attribute(handle, name)
            }

            fn connect(
                &self,
                from: &str,
                from_attribute: Option<&str>,
                to: &str,
                to_attribute: &str,
                args: Option<&[Self::Arg<'_>]>,
            ) -> Result<(), Self::Error> {
                if stringify!($outer) == "NoConnections" {
                    return Ok(());
                }
                self.0.connect(from, from_attribute, to, to_attribute, args)
            }

            fn disconnect(
                &self,
                from: &str,
                from_attribute: Option<&str>,
                to: &str,
                to_attribute: &str,
            ) -> Result<(), Self::Error> {
                self.0.disconnect(from, from_attribute, to, to_attribute)
            }

            fn evaluate(
                &self,
                args: &[Self::Arg<'_>],
            ) -> Result<(), Self::Error> {
                self.0.evaluate(args)
            }

            fn render_control(
                &self,
                action: Action,
                args: Option<&[Self::Arg<'_>]>,
            ) -> Result<(), Self::Error> {
                self.0.render_control(action, args)
            }
        }
    };
}

forward!(Identity);
forward!(NoConnections);
