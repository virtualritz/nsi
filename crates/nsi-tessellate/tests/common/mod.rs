//! A cube of six trimmed NURBS patches, the shape a STEP solid's faces
//! take in ɴsɪ, welded along its twelve edges or not.

// Each test binary uses its own part of this module.
#![allow(dead_code)]

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, Scene};
use nsi_trait::Nsi;
use std::num::NonZeroUsize;

/// The unit cube's corners, indexed by bits: x = 1, y = 2, z = 4.
fn corner(index: usize) -> [f32; 3] {
    [
        (index & 1) as f32,
        ((index >> 1) & 1) as f32,
        ((index >> 2) & 1) as f32,
    ]
}

/// The six faces, as four corners in the order (u, v) = (0,0), (1,0),
/// (1,1), (0,1).
const FACES: [[usize; 4]; 6] = [
    [0, 2, 3, 1], // z = 0
    [4, 5, 7, 6], // z = 1
    [0, 1, 5, 4], // y = 0
    [2, 6, 7, 3], // y = 1
    [0, 4, 6, 2], // x = 0
    [1, 3, 7, 5], // x = 1
];

/// Which way a cube's faces face, by 3Delight's convention for `nurbs`:
/// the front is the side `∂P/∂u × ∂P/∂v` points to. Rendered, not
/// assumed: `nurbs_fronts_face_the_standard_way` in `tests/displacement.rs`.
#[derive(Clone, Copy)]
pub enum Facing {
    /// Every face's front outside.
    Outward,
    /// Every face's front inside: the negative control.
    Inward,
}

/// A cube of six bilinear patches, each trimmed by the four lines around
/// its domain -- the shape a STEP solid's faces take in ɴsɪ. With
/// `welded`, one weld per cube edge, each used by the two faces it joins.
/// Faces point outward.
pub fn cube(welded: bool) -> Scene {
    cube_facing(welded, Facing::Outward)
}

/// The same, facing either way.
pub fn cube_facing(welded: bool, facing: Facing) -> Scene {
    let recorder = Recorder::new();
    if welded {
        recorder.create("cube_welds", "weld", None).unwrap();
    }
    for (face, corners) in FACES.iter().enumerate() {
        // `FACES` lists corners with ∂P/∂u × ∂P/∂v outward; swapping the
        // roles of u and v turns the cube inside out.
        let corners = match facing {
            Facing::Outward => *corners,
            Facing::Inward => [corners[0], corners[3], corners[2], corners[1]],
        };
        let handle = format!("face{face}");
        recorder.create(&handle, nsi::NURBS, None).unwrap();
        recorder
            .connect(&handle, None, nsi::ROOT, "objects", None)
            .unwrap();

        // ɴsɪ stores u fastest: (0,0), (1,0), (0,1), (1,1).
        let control =
            [corners[0], corners[1], corners[3], corners[2]].map(corner);
        // The domain's border, (0,0) → (1,0) → (1,1) → (0,1) → (0,0).
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let (u, v): (Vec<f32>, Vec<f32>) = (0..4)
            .flat_map(|side| [square[side], square[(side + 1) % 4]])
            .map(|[u, v]| (u, v))
            .unzip();
        let knots = [0.0, 0.0, 1.0, 1.0].repeat(4);

        recorder
            .set_attribute(
                &handle,
                &[
                    nsi::integer_i32!("nu", 2),
                    nsi::integer_i32!("nv", 2),
                    nsi::integer_i32!("uorder", 2),
                    nsi::integer_i32!("vorder", 2),
                    nsi::real_f32_slice!("uknot", &[0.0, 0.0, 1.0, 1.0]),
                    nsi::real_f32_slice!("vknot", &[0.0, 0.0, 1.0, 1.0]),
                    nsi::point3_f32_slice!("P", &control),
                    nsi::integer_i32_slice!("trimcurves.ncurves", &[4]),
                    nsi::integer_i32_slice!("trimcurves.n", &[2; 4]),
                    nsi::integer_i32_slice!("trimcurves.order", &[2; 4]),
                    nsi::real_f32_slice!("trimcurves.knot", &knots),
                    nsi::real_f32_slice!("trimcurves.min", &[0.0; 4]),
                    nsi::real_f32_slice!("trimcurves.max", &[1.0; 4]),
                    nsi::real_f32_slice!("trimcurves.u", &u),
                    nsi::real_f32_slice!("trimcurves.v", &v),
                    nsi::real_f32_slice!("trimcurves.w", &[1.0; 8]),
                ],
            )
            .unwrap();

        if welded {
            recorder
                .connect("cube_welds", None, &handle, "weld", None)
                .unwrap();
            // A cube edge's id: its two corners, smaller first.
            let ids: Vec<i32> = (0..4)
                .map(|side| {
                    let (a, b) = (corners[side], corners[(side + 1) % 4]);
                    (a.min(b) * 8 + a.max(b)) as i32
                })
                .collect();
            let indices: Vec<i32> =
                (0..4).flat_map(|curve| [curve, 0, 0]).collect();
            recorder
                .set_attribute(
                    &handle,
                    &[
                        nsi::integer_i32_slice!("weld.id", &ids),
                        nsi::string_slice!("weld.kind", &["trim-curve"; 4]),
                        nsi::integer_i32_slice!("weld.index", &indices)
                            .array_len(NonZeroUsize::new(3).unwrap()),
                    ],
                )
                .unwrap();
        }
    }
    recorder.into_scene()
}
