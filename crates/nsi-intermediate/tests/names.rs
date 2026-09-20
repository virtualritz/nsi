//! Spec 012: a scene speaks the naming-convention draft's vocabulary,
//! whichever one wrote it, and writes back the one the renderer of today
//! reads.
//!
//! The warnings are asserted here because a test binary is one process,
//! and `log` takes one logger per process.

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{EdgeKind, Recorder, Scene, write_stream};
use nsi_trait::Nsi;
use parking_lot::Mutex;

static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Collect;

impl log::Log for Collect {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.level() == log::Level::Warn {
            WARNINGS.lock().push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// Every test in this binary shares the logger and the scene, so that
/// "once per name" means what it says.
fn scene() -> &'static Scene {
    static SCENE: std::sync::OnceLock<Scene> = std::sync::OnceLock::new();
    SCENE.get_or_init(|| {
        let _ = log::set_logger(&Collect);
        log::set_max_level(log::LevelFilter::Warn);

        let recorder = Recorder::new();
        // An exporter writing the names 3Delight 2.9.210 reads.
        recorder.create("surface", nsi::NURBS, None).unwrap();
        recorder
            .set_attribute(
                "surface",
                &[
                    nsi::integer_i32!("nu", 4),
                    nsi::integer_i32!("nv", 4),
                    // Twice, to be warned about once.
                    nsi::integer_i32!("nu", 5),
                    nsi::string!("nicename", "a surface"),
                    // An ᴏsʟ global, and a row the draft marks an API
                    // change: both keep their names.
                    nsi::point3_f32_slice!("P", &[[0.0, 0.0, 0.0]]),
                    nsi::integer_i32!("trimcurves.inside", 1),
                    // Nothing the draft names.
                    nsi::integer_i32!("somethingelse", 1),
                ],
            )
            .unwrap();
        recorder.create("mesh", nsi::MESH, None).unwrap();
        recorder
            .set_attribute(
                "mesh",
                // Scoped to `instances`, so it is not renamed here.
                &[nsi::integer_i32!("sourcemodels", 1)],
            )
            .unwrap();
        recorder.create("models", "instances", None).unwrap();
        recorder
            .connect("mesh", None, "models", "sourcemodels", None)
            .unwrap();
        // The draft's name for the same destination.
        recorder.create("mesh2", nsi::MESH, None).unwrap();
        recorder
            .connect("mesh2", None, "models", "objects", None)
            .unwrap();
        recorder
            .connect("models", None, nsi::ROOT, "objects", None)
            .unwrap();
        recorder.create("driver", "outputdriver", None).unwrap();
        recorder.into_scene()
    })
}

/// N1, N2: the shipped names are stored as the draft's.
#[test]
fn shipped_names_are_stored_as_the_drafts() {
    let scene = scene();
    let surface = scene.node("surface").unwrap();
    assert!(surface.attribute("u.count").is_some());
    assert!(surface.attribute("nu").is_none(), "one vocabulary, not two");
    assert!(surface.attribute("nice-name").is_some());
}

/// N3: a row belongs to its node, so the same word elsewhere is untouched.
#[test]
fn a_scoped_row_renames_only_on_its_node() {
    let mesh = scene().node("mesh").unwrap();
    assert!(mesh.attribute("sourcemodels").is_some());
    assert!(mesh.attribute("objects").is_none());
}

/// N5, N6: what the draft does not rename is carried as written.
#[test]
fn unknown_and_kept_names_are_carried_as_written() {
    let surface = scene().node("surface").unwrap();
    for name in ["P", "trimcurves.inside", "somethingelse"] {
        assert!(surface.attribute(name).is_some(), "{name}");
    }
}

/// N8: a node created with either spelling is stored as the draft's.
#[test]
fn node_types_are_stored_as_the_drafts() {
    assert_eq!(scene().node("driver").unwrap().node_type(), "output-driver");
}

/// N9, N10: `objects` is the instance source on an `instances` node and
/// scene membership everywhere else.
#[test]
fn objects_classifies_by_the_node_it_reaches() {
    let scene = scene();
    let kinds: Vec<&EdgeKind> = scene.edges().map(|edge| &edge.kind).collect();
    // `sourcemodels` and the draft's `objects`, both into `instances`.
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == &EdgeKind::InstanceSource)
            .count(),
        2,
        "{kinds:?}"
    );
    // The same word into the scene root is membership.
    assert!(kinds.contains(&&EdgeKind::SceneMember), "{kinds:?}");
}

/// N11, N12: the stream carries the names the renderer of today reads.
#[test]
fn the_stream_carries_the_shipped_names() {
    let mut written = Vec::new();
    write_stream(scene(), &mut written).unwrap();
    let written = String::from_utf8(written).unwrap();

    for shipped in [
        "\"nu\" \"int\"",
        "\"nicename\" \"string\"",
        "Create \"driver\" \"outputdriver\"",
        "\"sourcemodels\"",
    ] {
        assert!(written.contains(shipped), "{shipped} in\n{written}");
    }
    for drafted in ["u.count", "nice-name", "output-driver"] {
        assert!(!written.contains(drafted), "{drafted} in\n{written}");
    }
    // Kept names are written as they were.
    assert!(written.contains("\"trimcurves.inside\""));
}

/// N4: once per name, naming the replacement -- the name is what an
/// exporter has to change, not the occurrence.
#[test]
fn a_deprecated_name_warns_once() {
    let _ = scene();
    let warnings = WARNINGS.lock().clone();
    let about_nu: Vec<&String> = warnings
        .iter()
        .filter(|warning| warning.contains("`nu`"))
        .collect();
    assert_eq!(about_nu.len(), 1, "{warnings:#?}");
    assert!(about_nu[0].contains("u.count"), "{}", about_nu[0]);
    assert!(
        warnings.iter().all(|warning| !warning.contains("`P`")),
        "an ᴏsʟ global is not deprecated: {warnings:#?}"
    );
}
