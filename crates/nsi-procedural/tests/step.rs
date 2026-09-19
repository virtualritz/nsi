//! The `step_procedural` example, compiled in and run through
//! [`nsi_procedural::execute`] into a [`Recorder`], on real parts.
//!
//! The parts are `monster-step-viewer`'s fixtures. Their headers state no
//! license, so they are not copied here: the tests read them where they
//! are and skip a part that is absent.
//!
//! `io1-ec-214` is the part the example is built for. `boxy` has
//! cylinder bands bounded only by circles, whose trim loop the conversion
//! replaces with a rectangle -- without mapping the rectangle's sides back
//! to the circles, 52 of its welds are open. `ap224` has cones and seams.
use nsi_ffi_wrap as nsi;
use nsi_intermediate::{OwnedArgument, Recorder, Scene};
use nsi_procedural::Report;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../examples/step/lib.rs"]
mod step;

const FIXTURES: &str = "/home/ritz/code/crates/monster-step-viewer/step-files";
const PART: &str = "io1-ec-214.stp";
const PARTS: [&str; 3] =
    [PART, "boxy_with_surfacetex.stp", "ap224_995277945.stp"];

/// The part called `name`, or `None` -- with a note -- when it is not on
/// this machine.
fn part(name: &str) -> Option<PathBuf> {
    let path = Path::new(FIXTURES).join(name);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: {} is absent", path.display());
        None
    }
}

/// Runs the procedural on `path` and returns the scene it made.
fn record(path: &Path, weld: i32) -> Scene {
    let recorder = Recorder::new();
    let sink = |_, message: &str| eprintln!("{message}");
    nsi_procedural::execute(
        &step::StepProcedural,
        &recorder,
        &Report::new(&sink),
        &[
            nsi::string!("filename", path.to_str().unwrap()),
            nsi::integer_i32!("weld", weld),
        ],
    )
    .expect("the procedural runs");
    recorder.into_scene()
}

/// The handles of the face nodes, by the naming the example documents.
fn faces(scene: &Scene) -> Vec<&str> {
    scene
        .nodes()
        .map(|(handle, _)| handle)
        .filter(|handle| handle.contains("_face"))
        .collect()
}

fn floats<'a>(scene: &'a Scene, face: &str, name: &str) -> &'a [f32] {
    scene
        .node(face)
        .unwrap()
        .attribute(name)
        .and_then(OwnedArgument::as_f32s)
        .unwrap_or_default()
}

fn integers<'a>(scene: &'a Scene, face: &str, name: &str) -> &'a [i32] {
    scene
        .node(face)
        .unwrap()
        .attribute(name)
        .and_then(OwnedArgument::as_i32s)
        .unwrap_or_default()
}

#[test]
fn every_face_is_a_nurbs_node() {
    let Some(path) = part(PART) else { return };
    let shells = step::loader::load_step_file(&path).unwrap();
    let scene = record(&path, 1);

    let faces = faces(&scene);
    let source_faces: usize =
        shells.iter().map(|shell| shell.shell.faces.len()).sum();
    assert_eq!(
        faces.len(),
        source_faces,
        "one node per B-rep face, none skipped"
    );
    for face in faces {
        let node_type = scene.node(face).unwrap().node_type();
        assert_eq!(node_type, "nurbs", "{face}");
    }
}

#[test]
fn the_weld_tables_are_valid_and_every_edge_is_manifold() {
    for name in PARTS {
        let Some(path) = part(name) else { continue };
        let shells = step::loader::load_step_file(&path).unwrap();
        let scene = record(&path, 1);

        let welds = scene.welds();
        assert!(
            welds.problems.is_empty(),
            "{name}: {}",
            welds
                .problems
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );

        // A closed solid: every edge is shared by exactly two face uses.
        let not_manifold = welds
            .welds
            .iter()
            .filter(|weld| weld.uses.len() != 2)
            .map(|weld| {
                let uses = weld
                    .uses
                    .iter()
                    .map(|weld_use| weld_use.geometry)
                    .collect::<Vec<_>>();
                format!("{} id {}: {uses:?}", weld.weld, weld.id)
            })
            .collect::<Vec<_>>();
        assert!(not_manifold.is_empty(), "{name}: {not_manifold:#?}");

        // And every edge of the source is declared: none lost to a made-up
        // trim curve or a dropped loop.
        let stem = path.file_stem().unwrap().to_str().unwrap();
        for (shell_index, shell) in shells.iter().enumerate() {
            let namespace = format!("{stem}_shell{shell_index}_welds");
            let mut ids = welds
                .welds
                .iter()
                .filter(|weld| weld.weld == namespace)
                .map(|weld| weld.id)
                .collect::<Vec<_>>();
            ids.sort_unstable();
            let edges = (0..shell.shell.edges.len() as u32).collect::<Vec<_>>();
            assert_eq!(ids, edges, "{namespace} declares every edge once");
        }
    }
}

/// Every trim curve starts and ends inside its surface's domain.
/// 3Delight does not wrap trims: a loop a whole period away trims the
/// whole face away.
#[test]
fn every_trim_curve_ends_inside_its_domain() {
    const TOLERANCE: f32 = 1.0e-3;
    for name in PARTS {
        let Some(path) = part(name) else { continue };
        for weld in [0, 1] {
            let scene = record(&path, weld);
            for face in faces(&scene) {
                let [umin, umax, vmin, vmax] = ["umin", "umax", "vmin", "vmax"]
                    .map(|name| floats(&scene, face, name)[0]);
                let (u, v, w) = (
                    floats(&scene, face, "trimcurves.u"),
                    floats(&scene, face, "trimcurves.v"),
                    floats(&scene, face, "trimcurves.w"),
                );
                let mut first = 0;
                for &count in integers(&scene, face, "trimcurves.n") {
                    // A clamped curve passes through its end points.
                    for point in [first, first + count as usize - 1] {
                        let (u, v) = (u[point] / w[point], v[point] / w[point]);
                        assert!(
                            umin - TOLERANCE <= u
                                && u <= umax + TOLERANCE
                                && vmin - TOLERANCE <= v
                                && v <= vmax + TOLERANCE,
                            "{name} weld {weld} {face}: ({u}, {v}) outside \
                             [{umin}, {umax}] × [{vmin}, {vmax}]"
                        );
                    }
                    first += count as usize;
                }
            }
        }
    }
}

#[test]
fn without_welds_there_are_no_weld_tables() {
    let Some(path) = part(PART) else { return };
    let scene = record(&path, 0);

    assert!(
        scene.nodes().all(|(_, node)| node.node_type() != "weld"),
        "no weld namespace"
    );
    for face in faces(&scene) {
        assert!(scene.weld_table(face).uses.is_empty(), "{face}");
        assert!(scene.node(face).unwrap().attribute("weld.id").is_none());
    }
    let welds = scene.welds();
    assert!(welds.welds.is_empty() && welds.problems.is_empty());
}

/// A face whose only trim loop traced its domain comes out untrimmed, and
/// declares the edges it shares as parts of its natural sides. `io1-ec-214`
/// and `boxy` have such faces; each one's every weld use is `nurbs-side`.
#[test]
fn untrimmed_faces_weld_by_their_natural_sides() {
    for name in [PART, "boxy_with_surfacetex.stp"] {
        let Some(path) = part(name) else { continue };
        let scene = record(&path, 1);
        let untrimmed: Vec<&str> = faces(&scene)
            .into_iter()
            .filter(|&face| {
                scene
                    .node(face)
                    .unwrap()
                    .attribute("trimcurves.ncurves")
                    .is_none()
            })
            .collect();
        assert!(!untrimmed.is_empty(), "{name} has untrimmed faces");
        for face in untrimmed {
            let table = scene.weld_table(face);
            assert!(table.problems.is_empty(), "{face}: {:?}", table.problems);
            assert!(!table.uses.is_empty(), "{face} declares its edges");
            assert!(
                table.uses.iter().flat_map(|use_| &use_.segments).all(
                    |segment| matches!(
                        segment.kind,
                        nsi_intermediate::WeldKind::NurbsSide { .. }
                    )
                ),
                "{face} welds by its sides"
            );
        }
    }
}
