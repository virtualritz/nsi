//! The parallel parser's gate: the scene it records is the scene the
//! sequential parser records, and its errors are the sequential
//! parser's errors.
#![cfg(feature = "parallel")]

use nsi_intermediate::{Recorder, Scene};
use nsi_parse::{parse_stream, parse_stream_parallel};
use std::fmt::Write;

/// A scene in a form that ignores exactly the order the parallel parser
/// may change, and keeps every order it must not.
///
/// Nodes and their attributes are sorted by name; so are edges, grouped
/// by destination node and attribute -- but within a group they keep
/// the order they were connected in, since at equal priority the first
/// connection into an attribute wins. Time samples and evaluations keep
/// call order.
fn canonical(scene: &Scene) -> String {
    let mut out = String::new();

    let mut nodes = scene.nodes().collect::<Vec<_>>();
    nodes.sort_by_key(|(handle, _)| *handle);
    for (handle, node) in nodes {
        writeln!(out, "node {handle:?} {:?}", node.node_type()).unwrap();
        let mut attributes = node.attributes().collect::<Vec<_>>();
        attributes.sort_by_key(|(name, _)| *name);
        for (name, value) in attributes {
            writeln!(out, "  {name:?} {value:?}").unwrap();
        }
        let mut samples = node.samples().collect::<Vec<_>>();
        samples.sort_by_key(|(name, _)| *name);
        for (name, calls) in samples {
            writeln!(out, "  {name:?} at {calls:?}").unwrap();
        }
    }

    let mut edges = scene
        .edges()
        .enumerate()
        .map(|(order, edge)| {
            (
                (edge.to().to_string(), format!("{:?}", edge.kind)),
                order,
                edge,
            )
        })
        .collect::<Vec<_>>();
    // Stable, and `order` breaks ties explicitly: stream order within a
    // destination attribute survives.
    edges.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    for (_, _, edge) in edges {
        writeln!(
            out,
            "edge {:?} -> {:?} {:?} {:?}",
            edge.from(),
            edge.to(),
            edge.kind,
            edge.args
        )
        .unwrap();
    }

    for evaluation in scene.evaluations() {
        writeln!(out, "evaluate {evaluation:?}").unwrap();
    }
    out
}

/// Both parsers' scenes for `input`, which must parse.
fn both(input: &str) -> (String, String) {
    let sequential = Recorder::new();
    parse_stream(input.as_bytes(), &sequential).expect("sequential parse");
    let parallel = Recorder::new();
    parse_stream_parallel(input.as_bytes(), &parallel).expect("parallel parse");
    (
        canonical(&sequential.into_scene()),
        canonical(&parallel.into_scene()),
    )
}

fn assert_same_scene(input: &str) {
    let (sequential, parallel) = both(input);
    assert!(!sequential.is_empty());
    assert_eq!(sequential, parallel);
}

/// The throughput corpus: many nodes, each with data and a connection
/// into one shared parent.
fn corpus(meshes: usize) -> String {
    let mut out = String::from(
        "Create \"grp\" \"transform\"\nConnect \"grp\" \"\" \".root\" \"objects\"\n",
    );
    for mesh in 0..meshes {
        writeln!(out, "Create \"mesh{mesh}\" \"mesh\"").unwrap();
        writeln!(out, "SetAttribute \"mesh{mesh}\" \"name\" \"string\" 1 \"object_{mesh}\"").unwrap();
        writeln!(out, "  \"P\" \"point\" 2 [ {mesh} 0 0 {mesh}.5 1 1 ]")
            .unwrap();
        writeln!(out, "Connect \"mesh{mesh}\" \"\" \"grp\" \"objects\"")
            .unwrap();
    }
    out
}

#[test]
fn a_large_scene_is_the_same_scene() {
    assert_same_scene(&corpus(2_000));
}

#[test]
fn the_last_set_attribute_on_a_node_wins_as_in_the_stream() {
    let mut stream =
        String::from("Create \"a\" \"mesh\"\nCreate \"b\" \"mesh\"\n");
    for value in 0..500 {
        writeln!(stream, "SetAttribute \"a\" \"x\" \"int\" 1 {value}").unwrap();
        writeln!(stream, "SetAttribute \"b\" \"x\" \"int\" 1 {}", 500 - value)
            .unwrap();
    }
    assert_same_scene(&stream);
}

/// At equal priority the first connection into an attribute wins, so
/// their order is part of the scene. Hundreds of them, so a parser that
/// reordered them could not pass by luck.
#[test]
fn connections_into_one_attribute_keep_their_order() {
    let mut stream = String::from("Create \"target\" \"mesh\"\n");
    for index in 0..300 {
        writeln!(stream, "Create \"attributes{index}\" \"attributes\"")
            .unwrap();
        writeln!(
            stream,
            "Connect \"attributes{index}\" \"\" \"target\" \"geometryattributes\""
        )
        .unwrap();
    }
    assert_same_scene(&stream);
}

#[test]
fn time_samples_keep_their_order() {
    let mut stream = String::from("Create \"xf\" \"transform\"\n");
    for sample in 0..200 {
        writeln!(
            stream,
            "SetAttributeAtTime \"xf\" {} \"t\" \"float\" 1 {sample}",
            f64::from(sample) / 200.0
        )
        .unwrap();
    }
    assert_same_scene(&stream);
}

/// A node deleted and re-created as another type: everything after the
/// `Delete` must see the new node, everything before it the old one.
#[test]
fn a_delete_is_a_barrier() {
    assert_same_scene(
        "Create \"a\" \"mesh\"
         SetAttribute \"a\" \"x\" \"int\" 1 1
         Create \"keep\" \"transform\"
         Delete \"a\"
         Create \"a\" \"transform\"
         SetAttribute \"a\" \"y\" \"int\" 1 2
         SetAttribute \"keep\" \"z\" \"int\" 1 3",
    );
}

#[test]
fn a_delete_attribute_is_a_barrier() {
    assert_same_scene(
        "Create \"a\" \"mesh\"
         SetAttribute \"a\" \"x\" \"int\" 1 1
         DeleteAttribute \"a\" \"x\"
         SetAttribute \"a\" \"x\" \"int\" 1 2",
    );
}

#[test]
fn a_disconnect_is_a_barrier() {
    assert_same_scene(
        "Create \"a\" \"transform\"
         Create \"b\" \"transform\"
         Connect \"a\" \"\" \".root\" \"objects\"
         Disconnect \"a\" \"\" \".root\" \"objects\"
         Connect \"b\" \"\" \".root\" \"objects\"
         Connect \"a\" \"\" \".root\" \"objects\"",
    );
}

#[test]
fn a_stream_with_render_control_and_evaluate_is_the_same_scene() {
    assert_same_scene(
        "Create \"a\" \"mesh\"
         Evaluate \"type\" \"string\" 1 \"apistream\" \"filename\" \"string\" 1 \"x.nsi\"
         Create \"b\" \"mesh\"
         RenderControl \"action\" \"string\" 1 \"start\"
         SetAttribute \"b\" \"x\" \"int\" 1 1
         RenderControl \"action\" \"string\" 1 \"synchronize\"
         Create \"c\" \"mesh\"",
    );
}

#[test]
fn an_empty_stream_is_fine() {
    let recorder = Recorder::new();
    parse_stream_parallel(b"  # only a comment\n", &recorder).unwrap();
    assert!(recorder.into_scene().is_empty());
}

/// Both parsers' errors for `input`, which must fail.
fn errors(input: &str) -> (String, String) {
    let sequential = parse_stream(input.as_bytes(), &Recorder::new())
        .expect_err("sequential must fail");
    let parallel = parse_stream_parallel(input.as_bytes(), &Recorder::new())
        .expect_err("parallel must fail");
    (format!("{sequential:?}"), format!("{parallel:?}"))
}

fn assert_same_error(input: &str) {
    let (sequential, parallel) = errors(input);
    assert_eq!(sequential, parallel);
}

#[test]
fn leading_junk_is_the_same_error() {
    assert_same_error("42 Create \"a\" \"mesh\"");
}

#[test]
fn junk_between_statements_is_the_same_error() {
    assert_same_error(
        "Create \"a\" \"mesh\"\nSetAttribute \"a\" \"x\" \"int\" 1 1 2\nCreate \"b\" \"mesh\"",
    );
}

#[test]
fn a_bad_value_mid_stream_is_the_same_error() {
    assert_same_error(
        "Create \"a\" \"mesh\"\nCreate \"b\" \"mesh\"\nSetAttribute \"b\" \"x\" \"int\" 1 oops\nSetAttribute \"a\" \"x\" \"int\" 1 1",
    );
}

#[test]
fn an_unterminated_string_is_the_same_error() {
    assert_same_error("Create \"a\" \"mesh\"\nCreate \"b");
}

#[test]
fn a_missing_render_control_action_is_the_same_error() {
    assert_same_error("Create \"a\" \"mesh\"\nRenderControl \"x\" \"int\" 1 1");
}

#[test]
fn a_sink_refusal_is_the_same_error() {
    // A node re-created as another type is refused by the recorder.
    assert_same_error("Create \"a\" \"mesh\"\nCreate \"a\" \"transform\"");
}

/// A sink that logs each call's kind and handle, in arrival order, and
/// accepts everything.
///
/// The recorded scene cannot say *when* a `RenderControl` or `Evaluate`
/// arrived relative to the edits around it, which is exactly what makes
/// them barriers; this can.
#[derive(Default)]
struct Log(std::sync::Mutex<Vec<String>>);

impl Log {
    fn push(&self, entry: String) -> Result<(), core::convert::Infallible> {
        self.0.lock().unwrap().push(entry);
        Ok(())
    }
}

impl nsi_trait::Nsi for Log {
    type Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>;
    type Error = core::convert::Infallible;

    fn create(
        &self,
        handle: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.push(format!("create {handle}"))
    }

    fn delete(
        &self,
        handle: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.push(format!("delete {handle}"))
    }

    fn set_attribute(
        &self,
        handle: &str,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        self.push(format!("set {handle}"))
    }

    fn set_attribute_at_time(
        &self,
        handle: &str,
        _: f64,
        _: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        self.push(format!("set {handle}"))
    }

    fn delete_attribute(
        &self,
        handle: &str,
        _: &str,
    ) -> Result<(), Self::Error> {
        self.push(format!("delete_attribute {handle}"))
    }

    fn connect(
        &self,
        from: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.push(format!("connect {from}"))
    }

    fn disconnect(
        &self,
        from: &str,
        _: Option<&str>,
        _: &str,
        _: &str,
    ) -> Result<(), Self::Error> {
        self.push(format!("disconnect {from}"))
    }

    fn evaluate(&self, _: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        self.push("barrier".to_string())
    }

    fn render_control(
        &self,
        _: nsi_trait::Action,
        _: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.push("barrier".to_string())
    }
}

/// Every call before a barrier in the stream arrives before it, and
/// every call after it arrives after -- with enough statements on each
/// side that a parser ignoring the barrier could not pass by luck.
#[test]
fn nothing_crosses_a_render_control_or_an_evaluate() {
    let mut stream = String::new();
    for segment in 0..4 {
        for node in 0..100 {
            let handle = format!("s{segment}n{node}");
            writeln!(stream, "Create \"{handle}\" \"mesh\"").unwrap();
            writeln!(stream, "SetAttribute \"{handle}\" \"x\" \"int\" 1 1")
                .unwrap();
            writeln!(stream, "Connect \"{handle}\" \"\" \".root\" \"objects\"")
                .unwrap();
        }
        stream.push_str(if segment % 2 == 0 {
            "RenderControl \"action\" \"string\" 1 \"synchronize\"\n"
        } else {
            "Evaluate \"type\" \"string\" 1 \"apistream\"\n"
        });
    }

    let log = Log::default();
    parse_stream_parallel(stream.as_bytes(), &log).unwrap();
    let log = log.0.into_inner().unwrap();

    // Split at the barriers; each piece must hold exactly its segment.
    let pieces = log.split(|entry| entry == "barrier").collect::<Vec<_>>();
    assert_eq!(pieces.len(), 5, "four barriers, in order");
    for (segment, piece) in pieces[..4].iter().enumerate() {
        assert_eq!(piece.len(), 300, "segment {segment}");
        let prefix = format!("s{segment}n");
        assert!(
            piece.iter().all(|entry| entry.contains(&prefix)),
            "segment {segment} holds only its own calls"
        );
    }
    assert!(pieces[4].is_empty());
}
