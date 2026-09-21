//! A cube of six trimmed patches, welded and not.
#![cfg(feature = "nurbs")]

mod common;

use common::{Facing, cube, cube_with};
use nsi_intermediate::{OwnedArgument, OwnedData};
use nsi_tessellate::{NurbsOptions, nurbs_meshes};
use nsi_trait::Type;

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

/// `trimcurves.inside` 0 asks for the surface *outside* the trim loops.
/// The mesher keeps the inside, so the patch is refused rather than
/// tessellated the wrong way round. The draft renames the attribute to
/// `trim-curves.hole`, which is per loop and inverted, so the two are
/// not aliases and a scene carries whichever it was given.
#[test]
fn keeping_the_outside_of_a_trim_is_refused() {
    let mut scene = cube(false);
    scene
        .set_attribute(
            "face0",
            vec![OwnedArgument::new(
                "trimcurves.inside",
                Type::IntegerI32,
                1,
                0,
                OwnedData::I32(vec![0]),
            )],
        )
        .unwrap();

    let tessellation = nurbs_meshes(&scene, &NurbsOptions::default());

    assert_eq!(tessellation.meshes.len(), 5, "the other five are meshed");
    assert!(
        tessellation
            .problems
            .iter()
            .any(|problem| problem.contains("trimcurves.inside")),
        "{:?}",
        tessellation.problems
    );
}

/// The two uses of a closed weld may start at different points: the
/// contract prefers a shared anchor but does not require one, and a
/// renderer has to cope. Here the upper cylinder's seam is a quarter
/// turn round from the lower one's, so the shared circle's two uses
/// begin a quarter apart.
#[test]
fn a_closed_weld_joins_uses_that_start_at_different_points() {
    let options = NurbsOptions::default();
    let shared = |rotate_seam: bool| {
        let tessellation = nurbs_meshes(
            &common::stacked_cylinders(true, rotate_seam),
            &options,
        );
        assert_eq!(tessellation.problems, Vec::<String>::new());
        assert_eq!(tessellation.meshes.len(), 2);
        // Points the two patches hold in common, which is what welding
        // them produced.
        let mut seen = std::collections::HashMap::new();
        let mut shared = 0;
        for mesh in &tessellation.meshes {
            for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
                let key = position.map(f64::to_bits);
                if let Some(previous) = seen.insert(key, *normal) {
                    assert_eq!(previous, *normal, "at {position:?}");
                    shared += 1;
                }
            }
        }
        (shared, tessellation.open_edges)
    };

    let (aligned, aligned_open) = shared(false);
    let (rotated, rotated_open) = shared(true);
    assert!(aligned > 0, "the aligned seams share their circle");
    assert!(rotated > 0, "and so do the rotated ones");
    // The shared counts differ -- the rotated join is two arcs, sampled
    // apart -- but a crack would show as open edges, and there are no
    // more of those than where the seams line up.
    assert_eq!(
        rotated_open, aligned_open,
        "no crack where the starts differ"
    );
}
