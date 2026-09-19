//! A cube of six trimmed patches, welded and not.
#![cfg(feature = "nurbs")]

mod common;

use common::cube;
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
