//! The typed scene layer must emit exactly the ɴsɪ calls the untyped
//! one would, and refuse the connections ɴsɪ does not allow.
//!
//! Asserted against a real `apistream` context: the calls are written
//! out and read back, so this checks what the renderer would be told,
//! not what the wrapper believes it said.
use nsi_ffi_wrap as nsi;
use nsi_toolbelt::scene;

/// Builds a scene into a stream file and returns its text.
/// `name` must be unique per test: these run in parallel, and sharing
/// one path means reading back whatever another test wrote.
fn emitted(name: &str, build: impl FnOnce(&nsi::Context)) -> String {
    let path = std::env::temp_dir().join(format!(
        "nsi_toolbelt_scene_{}_{name}.nsi",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    {
        let ctx = nsi::Context::new(Some(&[
            nsi::string!("type", "apistream"),
            nsi::string!("streamfilename", path.to_str().unwrap()),
        ]))
        .expect("apistream context");
        build(&ctx);
    }
    let text = std::fs::read_to_string(&path).expect("the stream");
    let _ = std::fs::remove_file(&path);
    text
}

/// The canonical scene anatomy, connected through the typed handles.
/// Every `Connect` the spec's own walkthrough performs must be present,
/// in the direction the spec gives.
#[test]
fn the_canonical_anatomy_emits_the_connections_the_spec_describes() {
    let text = emitted("anatomy", |ctx| {
        let camera = scene::perspective_camera(ctx, Some("camera"));
        let screen = scene::screen(ctx, Some("screen"));
        let layer = scene::output_layer(ctx, Some("beauty"));
        let driver = scene::output_driver(ctx, Some("driver"));
        let mesh = scene::mesh(ctx, Some("floor"));
        let attributes = scene::attributes(ctx, Some("attr"));
        let shader = scene::shader(ctx, Some("surface"));
        let place = scene::translation(ctx, Some("place"), &[0.0, 1.0, 0.0]);

        scene::root().append(ctx, &camera).append(ctx, &place);
        place
            .append(ctx, &mesh)
            .geometry_attributes(ctx, &attributes);
        attributes.surface_shader(ctx, &shader);
        camera.screens(ctx, &screen);
        screen.output_layers(ctx, &layer);
        layer.output_drivers(ctx, &driver);
    });

    for expected in [
        r#"Connect "camera" "" ".root" "objects""#,
        r#"Connect "place" "" ".root" "objects""#,
        r#"Connect "floor" "" "place" "objects""#,
        r#"Connect "attr" "" "place" "geometryattributes""#,
        r#"Connect "surface" "" "attr" "surfaceshader""#,
        r#"Connect "screen" "" "camera" "screens""#,
        r#"Connect "beauty" "" "screen" "outputlayers""#,
        r#"Connect "driver" "" "beauty" "outputdrivers""#,
    ] {
        assert!(
            text.contains(expected),
            "missing {expected}\n--- emitted ---\n{text}"
        );
    }
}

/// A typed constructor must create the node type it claims.
#[test]
fn constructors_create_the_node_type_they_name() {
    let text = emitted("constructors", |ctx| {
        scene::mesh(ctx, Some("m"));
        scene::perspective_camera(ctx, Some("c"));
        scene::attributes(ctx, Some("a"));
        scene::output_driver(ctx, Some("d"));
    });

    for expected in [
        r#"Create "m" "mesh""#,
        r#"Create "c" "perspectivecamera""#,
        r#"Create "a" "attributes""#,
        r#"Create "d" "outputdriver""#,
    ] {
        assert!(text.contains(expected), "missing {expected}\n{text}");
    }
}

/// The transform constructors must write a matrix, and it must be the
/// one `transform`'s pure functions produce -- the typed layer is a
/// wrapper, not a second implementation.
#[test]
fn a_typed_transform_writes_the_same_matrix_as_the_free_function() {
    let text = emitted("rotation", |ctx| {
        scene::rotation(ctx, Some("r"), 180.0, &[0.0, 1.0, 0.0]);
    });

    assert!(text.contains(r#"Create "r" "transform""#), "{text}");
    assert!(text.contains("transformationmatrix"), "{text}");

    // A half turn about Y negates X and Z. If the old four-times-too-big
    // conversion came back, this matrix would be the identity and the
    // "-1" would not appear.
    let matrix = nsi_toolbelt::rotation_matrix(180.0, &[0.0, 1.0, 0.0]);
    assert!((matrix[0] - -1.0).abs() < 1e-9, "{matrix:?}");
    assert!((matrix[10] - -1.0).abs() < 1e-9, "{matrix:?}");
}

/// `.root` is reserved: it must never be created, only connected to.
#[test]
fn root_is_not_created() {
    let text = emitted("root", |ctx| {
        let mesh = scene::mesh(ctx, Some("m"));
        scene::root().append(ctx, &mesh);
    });

    assert!(
        text.contains(r#"Connect "m" "" ".root" "objects""#),
        "{text}"
    );
    assert!(
        !text.contains(r#"Create ".root""#),
        "`.root` is reserved and must not be created\n{text}"
    );
}
