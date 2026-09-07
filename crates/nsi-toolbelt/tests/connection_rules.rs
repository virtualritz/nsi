//! The typed connection API and `nsi-intermediate`'s classifier encode
//! the same rules. This asserts they still agree.
//!
//! `scene.rs` says which connections ɴsɪ allows by making the illegal
//! ones fail to compile; `nsi_intermediate::classify` says what a
//! connection *means* by mapping its destination slot to an
//! [`EdgeKind`]. Both are transcriptions of the same `<connection>`
//! sections of the specification, and two transcriptions drift -- four
//! times inside `nsi-intermediate` alone, which is why its
//! `latest_per_time` carries a comment about it.
//!
//! The test is end to end rather than a table of slot names: it drives
//! the real typed methods into a real ɴsɪ `apistream` context, parses
//! what the renderer wrote, and asserts the classifier's verdict on the
//! edges that came back. A slot renamed on either side reddens it; a
//! table of strings would only have duplicated the rule a third time.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{EdgeKind, Recorder};
use nsi_toolbelt::scene;
use std::{fs, path::PathBuf, sync::OnceLock};

/// Every connection the typed API can make, and what it must mean.
///
/// Read as `(from handle, to handle, the kind the classifier owes us)`.
fn expected() -> Vec<(&'static str, &'static str, EdgeKind)> {
    vec![
        ("xf", ".root", EdgeKind::SceneMember),
        ("mesh", "xf", EdgeKind::SceneMember),
        ("cam", "xf", EdgeKind::SceneMember),
        ("attr", "xf", EdgeKind::AttributeBinding),
        ("shader_attr", "xf", EdgeKind::ShaderAttributes),
        ("surface", "attr", EdgeKind::SurfaceShader),
        ("displacement", "attr", EdgeKind::DisplacementShader),
        ("volume", "attr", EdgeKind::VolumeShader),
        ("screen", "cam", EdgeKind::Screen),
        ("layer", "screen", EdgeKind::OutputLayer),
        ("driver", "layer", EdgeKind::OutputDriver),
    ]
}

/// Writes a scene through the typed API and returns the stream.
///
/// Written once for the whole binary: both tests ask the same scene a
/// different question, and one `apistream` context is cheaper than two.
/// The file name carries the process id, as `tests/scene.rs` does, so
/// two runs of this binary cannot read each other's stream.
fn stream() -> &'static [u8] {
    static STREAM: OnceLock<Vec<u8>> = OnceLock::new();
    STREAM.get_or_init(write_stream)
}

fn write_stream() -> Vec<u8> {
    let path: PathBuf = std::env::temp_dir().join(format!(
        "nsi-toolbelt-connection-rules-{}.nsi",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);

    {
        let ctx = nsi::Context::new(Some(&[
            nsi::string!("type", "apistream"),
            nsi::string!("streamfilename", path.to_str().expect("utf-8")),
            nsi::string!("streamformat", "nsi"),
        ]))
        .expect("could not create an apistream ɴsɪ context");

        let root = scene::root();
        let transform = scene::transform(&ctx, Some("xf"));
        let mesh = scene::mesh(&ctx, Some("mesh"));
        let camera = scene::perspective_camera(&ctx, Some("cam"));
        let attributes = scene::attributes(&ctx, Some("attr"));
        let shader_attributes = scene::attributes(&ctx, Some("shader_attr"));
        let surface = scene::shader(&ctx, Some("surface"));
        let displacement = scene::shader(&ctx, Some("displacement"));
        let volume = scene::shader(&ctx, Some("volume"));
        let screen = scene::screen(&ctx, Some("screen"));
        let layer = scene::output_layer(&ctx, Some("layer"));
        let driver = scene::output_driver(&ctx, Some("driver"));

        root.append(&ctx, &transform);
        transform.append(&ctx, &mesh);
        transform.append(&ctx, &camera);
        transform.geometry_attributes(&ctx, &attributes);
        transform.shader_attributes(&ctx, &shader_attributes);
        attributes.surface_shader(&ctx, &surface);
        attributes.displacement_shader(&ctx, &displacement);
        attributes.volume_shader(&ctx, &volume);
        camera.screens(&ctx, &screen);
        screen.output_layers(&ctx, &layer);
        layer.output_drivers(&ctx, &driver);
    } // `Context::drop` calls `NSIEnd`, which flushes.

    fs::read(&path).expect("stream written")
}

/// Every typed connection classifies as the kind this crate's types
/// imply. Renaming a slot on either side reddens this.
#[test]
fn the_typed_connections_agree_with_the_classifier() {
    let written = stream();
    let recorder = Recorder::new();
    nsi_parse::parse_stream(written, &recorder).unwrap_or_else(|error| {
        panic!(
            "could not parse the stream the typed API wrote: {error}\n{}",
            String::from_utf8_lossy(written)
        )
    });
    let recorded = recorder.into_scene();

    for (from, to, kind) in expected() {
        let edge = recorded
            .edges()
            .find(|edge| edge.from() == from && edge.to() == to)
            .unwrap_or_else(|| {
                panic!(
                    "the typed API wrote no connection {from:?} -> {to:?}; \
                     the method that should make it changed or is gone\n\
                     --- stream ---\n{}",
                    String::from_utf8_lossy(stream())
                )
            });
        assert_eq!(
            edge.kind, kind,
            "{from:?} -> {to:?}: the classifier and the typed API \
             disagree about what this connection means",
        );
    }
}

/// The classifier knows every slot the typed API writes.
///
/// The check above compares kinds; this one catches the quieter
/// failure, where a slot the types allow falls into `EdgeKind::Other`
/// and is carried without ever being resolved.
#[test]
fn no_typed_connection_falls_through_to_other() {
    let recorder = Recorder::new();
    nsi_parse::parse_stream(stream(), &recorder).expect("parse");
    let recorded = recorder.into_scene();

    let unclassified: Vec<&str> = recorded
        .edges()
        .filter_map(|edge| match &edge.kind {
            EdgeKind::Other { to_attribute } => Some(to_attribute.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        unclassified.is_empty(),
        "the typed API writes slots the classifier does not know: \
         {unclassified:?}",
    );
}
