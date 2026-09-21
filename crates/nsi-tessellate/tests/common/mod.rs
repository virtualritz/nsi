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

/// The fixtures below write the naming-convention draft's names, as a new
/// exporter would: a `Scene` stores them either way. `P` and `Pw` keep
/// theirs, being what a shader reads, and so do `trimcurves.u`, `.v`,
/// `.w`, which the draft replaces with one interleaved array rather than
/// renaming.
///
/// A cube of six bilinear patches, each trimmed by the four lines around
/// its domain -- the shape a STEP solid's faces take in ɴsɪ. With
/// `welded`, one weld per cube edge, each used by the two faces it joins.
/// Faces point outward.
pub fn cube(welded: bool) -> Scene {
    cube_facing(welded, Facing::Outward)
}

/// The same, facing either way.
pub fn cube_facing(welded: bool, facing: Facing) -> Scene {
    cube_with(welded, facing, [false; 6], false)
}

/// A cube whose faces marked in `sides` are untrimmed and welded by their
/// natural sides (`nurbs-side`); the others are trimmed and welded by
/// their trim curves. With `halves`, every cube edge is two welds, one per
/// half, each use selecting its half with `weld.range`.
pub fn cube_with(
    welded: bool,
    facing: Facing,
    sides: [bool; 6],
    halves: bool,
) -> Scene {
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
                    nsi::integer_i32!("u.count", 2),
                    nsi::integer_i32!("v.count", 2),
                    nsi::integer_i32!("u.order", 2),
                    nsi::integer_i32!("v.order", 2),
                    nsi::real_f32_slice!("u.knot", &[0.0, 0.0, 1.0, 1.0]),
                    nsi::real_f32_slice!("v.knot", &[0.0, 0.0, 1.0, 1.0]),
                    nsi::point3_f32_slice!("P", &control),
                ],
            )
            .unwrap();
        if !sides[face] {
            recorder
                .set_attribute(
                    &handle,
                    &[
                        nsi::integer_i32_slice!("trim-curves.curve-count", &[4]),
                        nsi::integer_i32_slice!("trim-curves.point-count", &[2; 4]),
                        nsi::integer_i32_slice!("trim-curves.order", &[2; 4]),
                        nsi::real_f32_slice!("trim-curves.knot", &knots),
                        nsi::real_f32_slice!("trim-curves.min", &[0.0; 4]),
                        nsi::real_f32_slice!("trim-curves.max", &[1.0; 4]),
                        nsi::real_f32_slice!("trimcurves.u", &u),
                        nsi::real_f32_slice!("trimcurves.v", &v),
                        nsi::real_f32_slice!("trimcurves.w", &[1.0; 8]),
                    ],
                )
                .unwrap();
        }

        if welded {
            recorder
                .connect("cube_welds", None, &handle, "weld", None)
                .unwrap();
            // Border piece k of the domain runs from `corners[k]` to
            // `corners[k + 1]`. As a trim curve it is curve k, stored in
            // that direction. As a natural side it is v-min, u-max, v-max,
            // u-min in turn; the first two run along the border piece and
            // the last two against it, a u-side running along increasing
            // `v` and a v-side along increasing `u`.
            const SIDE: [i32; 4] = [2, 1, 3, 0];
            let uses: Vec<(i32, i32, i32, [f32; 2])> = (0..4)
                .flat_map(|k| {
                    let (a, b) = (corners[k], corners[(k + 1) % 4]);
                    // A cube edge's id: its two corners, smaller first.
                    let edge = (a.min(b) * 8 + a.max(b)) as i32;
                    // Every use of a weld follows one reference
                    // traversal. This exporter's is from the edge's
                    // smaller corner to its larger, and `weld.reverse`
                    // says which uses run against it.
                    let index = if sides[face] { SIDE[k] } else { k as i32 };
                    // The corner the selector itself starts at, whatever
                    // the loop does: which end `weld.range` counts from.
                    let start = if sides[face] && k >= 2 { b } else { a };
                    let reverse = i32::from(start != a.min(b));
                    let halves: Vec<(i32, i32, i32, [f32; 2])> = if halves {
                        [[0.0, 0.5], [0.5, 1.0]]
                            .into_iter()
                            .map(|range| {
                                // Half 0 is the one at the smaller corner.
                                let near_start = range[0] == 0.0;
                                let half = i32::from(
                                    near_start != (start == a.min(b)),
                                );
                                (edge * 2 + half, index, reverse, range)
                            })
                            .collect()
                    } else {
                        vec![(edge, index, reverse, [0.0, 1.0])]
                    };
                    halves
                })
                .collect();
            let ids: Vec<i32> = uses.iter().map(|use_| use_.0).collect();
            let indices: Vec<i32> =
                uses.iter().flat_map(|use_| [use_.1, 0, 0]).collect();
            let reverse: Vec<i32> = uses.iter().map(|use_| use_.2).collect();
            let ranges: Vec<f32> =
                uses.iter().flat_map(|use_| use_.3).collect();
            let kind = if sides[face] {
                "nurbs-side"
            } else {
                "trim-curve"
            };
            recorder
                .set_attribute(
                    &handle,
                    &[
                        nsi::integer_i32_slice!("weld.id", &ids),
                        nsi::string_slice!(
                            "weld.kind",
                            &vec![kind; uses.len()]
                        ),
                        nsi::integer_i32_slice!("weld.index", &indices)
                            .array_len(NonZeroUsize::new(3).unwrap()),
                        nsi::integer_i32_slice!("weld.reverse", &reverse),
                        nsi::real_f32_slice!("weld.range", &ranges)
                            .array_len(NonZeroUsize::new(2).unwrap()),
                    ],
                )
                .unwrap();
        }
    }
    recorder.into_scene()
}

/// Two cylinder patches stacked along `z`, sharing the circle between
/// them: the lower one's v-max side welded to the upper one's v-min
/// side.
///
/// With `rotate_seam`, the upper patch's control net starts a quarter
/// turn round, so its seam -- and with it the start of its side -- is
/// elsewhere than the lower patch's. The two uses of that weld then
/// begin at different points, which the [weld contract] allows and a
/// renderer has to cope with.
///
/// [weld contract]: https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html
pub fn stacked_cylinders(welded: bool, rotate_seam: bool) -> Scene {
    // A circle as four rational quadratic arcs: on-axis points at the
    // quarters, corner points a `sqrt(2)` out at the diagonals, weighted
    // to pull the arc back onto the circle.
    const CORNER: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let ring = |turn: f32| -> Vec<[f32; 3]> {
        (0..9)
            .map(|at| {
                let angle = turn + at as f32 * std::f32::consts::FRAC_PI_4;
                let radius = if at % 2 == 0 {
                    1.0
                } else {
                    std::f32::consts::SQRT_2
                };
                [radius * angle.cos(), radius * angle.sin(), 0.0]
            })
            .collect()
    };
    let weights: Vec<f32> = (0..9)
        .map(|at| if at % 2 == 0 { 1.0 } else { CORNER })
        .collect();
    let quarter = std::f32::consts::FRAC_PI_2;
    let u_knot: Vec<f32> = vec![
        0.0,
        0.0,
        0.0,
        quarter,
        quarter,
        2.0 * quarter,
        2.0 * quarter,
        3.0 * quarter,
        3.0 * quarter,
        4.0 * quarter,
        4.0 * quarter,
        4.0 * quarter,
    ];

    let recorder = Recorder::new();
    if welded {
        recorder.create("stack_welds", "weld", None).unwrap();
    }
    for (patch, (bottom, top)) in [(0.0, 1.0), (1.0, 2.0)].iter().enumerate() {
        let handle = format!("band{patch}");
        recorder.create(&handle, nsi::NURBS, None).unwrap();
        recorder
            .connect(&handle, None, nsi::ROOT, "objects", None)
            .unwrap();

        let turn = if patch == 1 && rotate_seam {
            quarter
        } else {
            0.0
        };
        // ɴsɪ stores control points u-fastest, and `Pw` premultiplied.
        let control: Vec<[f32; 4]> = [*bottom, *top]
            .iter()
            .flat_map(|z| {
                ring(turn)
                    .into_iter()
                    .zip(&weights)
                    .map(|(point, weight)| {
                        [
                            point[0] * weight,
                            point[1] * weight,
                            z * weight,
                            *weight,
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        recorder
            .set_attribute(
                &handle,
                &[
                    nsi::integer_i32!("u.count", 9),
                    nsi::integer_i32!("v.count", 2),
                    nsi::integer_i32!("u.order", 3),
                    nsi::integer_i32!("v.order", 2),
                    nsi::real_f32_slice!("u.knot", &u_knot),
                    nsi::real_f32_slice!(
                        "v.knot",
                        &[*bottom, *bottom, *top, *top]
                    ),
                    nsi::point4_f32_slice!("Pw", &control),
                ],
            )
            .unwrap();

        if welded {
            recorder
                .connect("stack_welds", None, &handle, "weld", None)
                .unwrap();
            // The lower patch's v-max side, the upper one's v-min: both
            // run along increasing `u`, which is the same way round the
            // axis, so neither is reversed.
            let side = if patch == 0 { 3 } else { 2 };
            recorder
                .set_attribute(
                    &handle,
                    &[
                        nsi::integer_i32_slice!("weld.id", &[0]),
                        nsi::string_slice!("weld.kind", &["nurbs-side"]),
                        nsi::integer_i32_slice!("weld.index", &[side, 0, 0])
                            .array_len(NonZeroUsize::new(3).unwrap()),
                        nsi::integer_i32_slice!("weld.reverse", &[0]),
                    ],
                )
                .unwrap();
        }
    }
    recorder.into_scene()
}
