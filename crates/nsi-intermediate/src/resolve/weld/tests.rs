//! Tests for [`super`]: the draft's own examples, then every invalid
//! declaration it names.

use super::{NurbsSide, WeldKind, WeldProblemKind};
use crate::{OwnedArgument, OwnedData, Scene};
use nsi_ffi_wrap::nsi_sys::NSIParamFlags;
use nsi_trait::Type;

fn integers(name: &str, values: &[i32]) -> OwnedArgument {
    OwnedArgument {
        name: name.to_string(),
        type_tag: Type::IntegerI32,
        array_length: 1,
        flags: 0,
        data: OwnedData::I32(values.to_vec()),
    }
}

/// `int[3]` tuples, as `weld.index` is declared.
fn triples(name: &str, values: &[[i32; 3]]) -> OwnedArgument {
    OwnedArgument {
        name: name.to_string(),
        type_tag: Type::IntegerI32,
        array_length: 3,
        flags: NSIParamFlags::IsArray.bits(),
        data: OwnedData::I32(values.iter().flatten().copied().collect()),
    }
}

fn floats2(name: &str, values: &[[f32; 2]]) -> OwnedArgument {
    OwnedArgument {
        name: name.to_string(),
        type_tag: Type::RealF32,
        array_length: 2,
        flags: NSIParamFlags::IsArray.bits(),
        data: OwnedData::F32(values.iter().flatten().copied().collect()),
    }
}

fn strings(name: &str, values: &[&str]) -> OwnedArgument {
    OwnedArgument {
        name: name.to_string(),
        type_tag: Type::String,
        array_length: 1,
        flags: 0,
        data: OwnedData::String(
            values
                .iter()
                .map(|value| value.as_bytes().to_vec())
                .collect(),
        ),
    }
}

/// A scene with a `weld` namespace, a NURBS `patch` whose single trim
/// loop has five curves, and a one-face `subdiv` mesh with four edges.
fn solid() -> Scene {
    let mut scene = Scene::default();
    scene.create("solid_welds", "weld").unwrap();
    scene.create("patch", "nurbs").unwrap();
    scene
        .set_attribute("patch", vec![integers("trimcurves.ncurves", &[5])])
        .unwrap();
    scene.create("subdiv", "mesh").unwrap();
    scene
        .set_attribute("subdiv", vec![integers("nvertices", &[4])])
        .unwrap();
    for geometry in ["patch", "subdiv"] {
        scene.connect(geometry, None, ".root", "objects").unwrap();
        scene
            .connect("solid_welds", None, geometry, "weld")
            .unwrap();
    }
    scene
}

fn declare(scene: &mut Scene, geometry: &str, table: Vec<OwnedArgument>) {
    scene.set_attribute(geometry, table).unwrap();
}

/// The draft's "five-to-one join": trim loop 0 of `patch` and one edge
/// of `subdiv` are one boundary.
#[test]
fn the_five_to_one_join_is_one_weld_with_two_uses() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            strings("weld.kind", &["trim-loop"]),
            triples("weld.index", &[[0, 0, 0]]),
        ],
    );
    declare(
        &mut scene,
        "subdiv",
        vec![
            integers("weld.id", &[12]),
            strings("weld.kind", &["mesh-edge"]),
            triples("weld.index", &[[0, 0, 2]]),
        ],
    );

    let welds = scene.welds();
    assert_eq!(welds.problems, []);
    assert_eq!(welds.welds.len(), 1);
    let weld = &welds.welds[0];
    assert_eq!((weld.weld, weld.id), ("solid_welds", 12));
    assert!(!weld.is_open() && !weld.is_non_manifold());

    let kinds = weld
        .uses
        .iter()
        .map(|weld_use| (weld_use.geometry, weld_use.segments[0].kind))
        .collect::<Vec<_>>();
    assert!(kinds.contains(&("patch", WeldKind::TrimLoop { loop_index: 0 })));
    assert!(kinds.contains(&(
        "subdiv",
        WeldKind::MeshEdge {
            face_index: 0,
            loop_index: 0,
            edge_index: 2
        }
    )));
}

/// The same loop listed as five consecutive trim curves is one use of
/// five segments, and checked as a chain.
#[test]
fn five_consecutive_trim_curves_are_one_use() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            integers("weld.segment-count", &[5]),
            strings("weld.kind", &["trim-curve"; 5]),
            triples(
                "weld.index",
                &[[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0], [4, 0, 0]],
            ),
        ],
    );

    let table = scene.weld_table("patch");
    assert_eq!(table.problems, []);
    assert_eq!(table.uses.len(), 1);
    assert_eq!(table.uses[0].segments.len(), 5);
}

/// A reversed chain runs backwards, wrapping around the loop.
#[test]
fn a_reversed_chain_runs_backwards_around_the_loop() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            integers("weld.segment-count", &[3]),
            strings("weld.kind", &["trim-curve"; 3]),
            triples("weld.index", &[[1, 0, 0], [0, 0, 0], [4, 0, 0]]),
            integers("weld.reverse", &[1, 1, 1]),
        ],
    );
    assert_eq!(scene.weld_table("patch").problems, []);
}

#[test]
fn a_gap_in_a_trim_curve_chain_is_reported_and_dropped() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            integers("weld.segment-count", &[3]),
            strings("weld.kind", &["trim-curve"; 3]),
            triples("weld.index", &[[0, 0, 0], [2, 0, 0], [3, 0, 0]]),
        ],
    );
    let table = scene.weld_table("patch");
    assert_eq!(table.uses, []);
    assert_eq!(
        table.problems[0].kind,
        WeldProblemKind::NotConnected { segment: 1 }
    );
}

/// Two joins on one patch, no second node: the draft's "several
/// independent joins".
#[test]
fn one_table_declares_independent_joins() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12, 13]),
            strings("weld.kind", &["trim-loop", "nurbs-side"]),
            triples("weld.index", &[[0, 0, 0], [1, 0, 0]]),
        ],
    );
    let welds = scene.welds();
    assert_eq!(welds.problems, []);
    assert_eq!(welds.welds.len(), 2);
    assert!(welds.welds.iter().all(|weld| weld.is_open()));
    let side = welds
        .welds
        .iter()
        .find(|weld| weld.id == 13)
        .map(|weld| weld.uses[0].segments[0].kind);
    assert_eq!(
        side,
        Some(WeldKind::NurbsSide {
            side: NurbsSide::UMax
        })
    );
}

/// Two uses of one geometry with one id: a self-seam. Three uses: a
/// non-manifold join. Both are reported as they are, not refused.
#[test]
fn self_seams_and_non_manifold_joins_are_kept() {
    let mut scene = solid();
    declare(
        &mut scene,
        "subdiv",
        vec![
            integers("weld.id", &[7, 7, 7]),
            strings("weld.kind", &["mesh-edge"; 3]),
            triples("weld.index", &[[0, 0, 0], [0, 0, 1], [0, 0, 2]]),
        ],
    );
    let welds = scene.welds();
    assert_eq!(welds.problems, []);
    assert_eq!(welds.welds.len(), 1);
    assert!(welds.welds[0].is_non_manifold());
}

/// The same id in two namespaces is two boundaries.
#[test]
fn ids_are_local_to_their_weld_node() {
    let mut scene = solid();
    scene.create("other_welds", "weld").unwrap();
    scene.create("elsewhere", "mesh").unwrap();
    scene
        .set_attribute("elsewhere", vec![integers("nvertices", &[3])])
        .unwrap();
    scene
        .connect("elsewhere", None, ".root", "objects")
        .unwrap();
    scene
        .connect("other_welds", None, "elsewhere", "weld")
        .unwrap();
    for geometry in ["subdiv", "elsewhere"] {
        declare(
            &mut scene,
            geometry,
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["mesh-edge"]),
                triples("weld.index", &[[0, 0, 0]]),
            ],
        );
    }
    let welds = scene.welds();
    assert_eq!(welds.welds.len(), 2);
    assert!(welds.welds.iter().all(|weld| weld.is_open()));
}

/// A chain mixing a natural side with trim curves cannot be checked from
/// topology; it is kept and flagged.
#[test]
fn an_unverifiable_chain_is_kept_and_flagged() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            integers("weld.segment-count", &[2]),
            strings("weld.kind", &["nurbs-side", "trim-curve"]),
            triples("weld.index", &[[0, 0, 0], [0, 0, 0]]),
        ],
    );
    let table = scene.weld_table("patch");
    assert_eq!(table.uses.len(), 1);
    assert_eq!(table.problems.len(), 1);
    assert_eq!(
        table.problems[0].kind,
        WeldProblemKind::ConnectivityUnverified
    );
    assert!(!table.problems[0].drops_use());
}

/// One case per invalid declaration: the table, and the problem it must
/// produce. Each drops its use.
#[test]
fn every_invalid_declaration_is_reported() {
    let cases: Vec<(&str, Vec<OwnedArgument>, WeldProblemKind)> = vec![
        (
            "a negative id",
            vec![
                integers("weld.id", &[-1]),
                strings("weld.kind", &["trim-loop"]),
                triples("weld.index", &[[0, 0, 0]]),
            ],
            WeldProblemKind::NegativeId { id: -1 },
        ),
        (
            "an empty use",
            vec![
                integers("weld.id", &[12]),
                integers("weld.segment-count", &[0]),
                strings("weld.kind", &[]),
                triples("weld.index", &[]),
            ],
            WeldProblemKind::EmptyUse,
        ),
        (
            "a kind the geometry cannot have",
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["mesh-edge"]),
                triples("weld.index", &[[0, 0, 0]]),
            ],
            WeldProblemKind::UnknownKind {
                kind: "mesh-edge".to_string(),
            },
        ),
        (
            "a loop that does not exist",
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["trim-loop"]),
                triples("weld.index", &[[1, 0, 0]]),
            ],
            WeldProblemKind::BadIndex { index: [1, 0, 0] },
        ),
        (
            "a side beyond 3",
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["nurbs-side"]),
                triples("weld.index", &[[4, 0, 0]]),
            ],
            WeldProblemKind::BadIndex { index: [4, 0, 0] },
        ),
        (
            "an inverted range",
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["trim-curve"]),
                triples("weld.index", &[[0, 0, 0]]),
                floats2("weld.range", &[[0.5, 0.2]]),
            ],
            WeldProblemKind::BadRange { range: [0.5, 0.2] },
        ),
        (
            "a partial loop",
            vec![
                integers("weld.id", &[12]),
                strings("weld.kind", &["trim-loop"]),
                triples("weld.index", &[[0, 0, 0]]),
                floats2("weld.range", &[[0.0, 0.5]]),
            ],
            WeldProblemKind::PartialLoop,
        ),
        (
            "a loop with company",
            vec![
                integers("weld.id", &[12]),
                integers("weld.segment-count", &[2]),
                strings("weld.kind", &["trim-loop", "trim-curve"]),
                triples("weld.index", &[[0, 0, 0], [0, 0, 0]]),
            ],
            WeldProblemKind::PartialLoop,
        ),
    ];

    for (what, table, expected) in cases {
        let mut scene = solid();
        declare(&mut scene, "patch", table);
        let table = scene.weld_table("patch");
        assert_eq!(table.uses, [], "{what}: the use is dropped");
        assert_eq!(
            table.problems.iter().map(|p| &p.kind).collect::<Vec<_>>(),
            [&expected],
            "{what}"
        );
        assert!(table.problems[0].drops_use(), "{what}");
        assert!(!table.problems[0].to_string().is_empty());
    }
}

#[test]
fn counts_that_do_not_partition_the_arrays_are_reported() {
    let mut scene = solid();
    declare(
        &mut scene,
        "patch",
        vec![
            integers("weld.id", &[12]),
            integers("weld.segment-count", &[2]),
            strings("weld.kind", &["trim-curve"]),
            triples("weld.index", &[[0, 0, 0]]),
        ],
    );
    assert_eq!(
        scene.weld_table("patch").problems[0].kind,
        WeldProblemKind::BadAttribute {
            attribute: "weld.kind",
            expected: Some(2),
        }
    );
}

#[test]
fn a_table_without_a_namespace_is_reported() {
    let mut scene = Scene::default();
    scene.create("loose", "mesh").unwrap();
    declare(
        &mut scene,
        "loose",
        vec![
            integers("nvertices", &[4]),
            integers("weld.id", &[1]),
            strings("weld.kind", &["mesh-edge"]),
            triples("weld.index", &[[0, 0, 0]]),
        ],
    );
    assert_eq!(
        scene.weld_table("loose").problems[0].kind,
        WeldProblemKind::NoWeldNode
    );
}

#[test]
fn two_namespaces_on_one_geometry_are_reported() {
    let mut scene = solid();
    scene.create("second_welds", "weld").unwrap();
    scene
        .connect("second_welds", None, "subdiv", "weld")
        .unwrap();
    declare(
        &mut scene,
        "subdiv",
        vec![
            integers("weld.id", &[1]),
            strings("weld.kind", &["mesh-edge"]),
            triples("weld.index", &[[0, 0, 0]]),
        ],
    );
    assert_eq!(
        scene.weld_table("subdiv").problems[0].kind,
        WeldProblemKind::SeveralWeldNodes { count: 2 }
    );
}

/// A mesh with holes: face 0 has an outer loop of 4 and a hole of 3, so
/// loop 1 has three edges and no fourth.
#[test]
fn mesh_edges_index_into_holes() {
    let mut scene = solid();
    scene.create("holey", "mesh").unwrap();
    scene
        .set_attribute(
            "holey",
            vec![integers("nvertices", &[4, 3]), integers("nholes", &[1])],
        )
        .unwrap();
    scene.connect("holey", None, ".root", "objects").unwrap();
    scene.connect("solid_welds", None, "holey", "weld").unwrap();
    declare(
        &mut scene,
        "holey",
        vec![
            integers("weld.id", &[1, 2]),
            strings("weld.kind", &["mesh-edge", "mesh-edge"]),
            triples("weld.index", &[[0, 1, 2], [0, 1, 3]]),
        ],
    );
    let table = scene.weld_table("holey");
    assert_eq!(table.uses.len(), 1, "edge 2 of the hole exists");
    assert_eq!(
        table.problems[0].kind,
        WeldProblemKind::BadIndex { index: [0, 1, 3] }
    );
}
