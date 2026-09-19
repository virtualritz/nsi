//! A cube of six trimmed patches, welded and not.
#![cfg(feature = "nurbs")]

mod common;

use common::{Facing, cube, cube_with};
use nsi_tessellate::{NurbsOptions, nurbs_meshes};

#[test]
fn a_welded_cube_is_watertight() {
    let scene = cube(true);
    assert_eq!(scene.welds().welds.len(), 12, "twelve edges");

    let tessellation = nurbs_meshes(&scene, &NurbsOptions::default());

    assert_eq!(tessellation.problems, Vec::<String>::new());
    assert_eq!(tessellation.meshes.len(), 6);
    assert_eq!(
        tessellation.open_edges, 0,
        "every triangle edge has a partner"
    );
}

/// The negative control: the same patches without welds are six separate
/// pieces, and their borders are open.
#[test]
fn the_same_cube_without_welds_is_open() {
    let tessellation = nurbs_meshes(&cube(false), &NurbsOptions::default());
    assert_eq!(tessellation.meshes.len(), 6);
    assert!(tessellation.open_edges > 0, "unwelded borders stay open");
}

/// Along a seam, both sides carry the same position and the same normal,
/// bit for bit -- what a displacement needs to move them together.
#[test]
fn seam_points_carry_identical_normals_on_both_sides() {
    let tessellation = nurbs_meshes(&cube(true), &NurbsOptions::default());
    let mut seen = std::collections::HashMap::new();
    let mut shared = 0;
    for mesh in &tessellation.meshes {
        for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
            let key = position.map(f64::to_bits);
            if let Some(previous) = seen.insert(key, *normal) {
                assert_eq!(
                    previous, *normal,
                    "normals disagree at {position:?}"
                );
                shared += 1;
            }
        }
    }
    assert!(shared > 0, "the faces share seam points");
}

/// Faces 0, 2 and 4 -- z = 0, y = 0 and x = 0, pairwise adjacent -- are
/// untrimmed and welded by their natural sides; the rest by trim curves.
/// So the cube has side-to-side and side-to-trim joins.
const MIXED: [bool; 6] = [true, false, true, false, true, false];

/// Welds `sides` and `halves` as `cube_with` does, and returns the open
/// edges welded and unwelded, requiring no problems and `welds` welds.
fn open_edges(sides: [bool; 6], halves: bool, welds: usize) -> (usize, usize) {
    let welded = cube_with(true, Facing::Outward, sides, halves);
    let resolved = welded.welds();
    assert_eq!(resolved.problems.len(), 0, "{:?}", resolved.problems);
    assert_eq!(resolved.welds.len(), welds);
    assert!(
        resolved.welds.iter().all(|weld| weld.uses.len() == 2),
        "every weld joins two uses"
    );
    let options = NurbsOptions::default();
    let tessellation = nurbs_meshes(&welded, &options);
    assert_eq!(tessellation.problems, Vec::<String>::new());
    assert_eq!(tessellation.meshes.len(), 6);
    let unwelded = nurbs_meshes(
        &cube_with(false, Facing::Outward, sides, halves),
        &options,
    );
    (tessellation.open_edges, unwelded.open_edges)
}

#[test]
fn natural_sides_weld_to_sides_and_to_trim_curves() {
    let (welded, unwelded) = open_edges(MIXED, false, 12);
    assert_eq!(welded, 0, "every triangle edge has a partner");
    assert!(unwelded > 0, "unwelded borders stay open");
}

#[test]
fn a_cube_of_untrimmed_faces_welds_by_sides_alone() {
    let (welded, unwelded) = open_edges([true; 6], false, 12);
    assert_eq!(welded, 0, "every triangle edge has a partner");
    assert!(unwelded > 0, "unwelded borders stay open");
}

/// Every cube edge as two welds, one per half, selected by `weld.range`
/// on sides and on trim curves, counted from either end.
#[test]
fn ranges_weld_parts_of_sides_and_curves() {
    let (welded, unwelded) = open_edges(MIXED, true, 24);
    assert_eq!(welded, 0, "every triangle edge has a partner");
    assert!(unwelded > 0, "unwelded borders stay open");
}
