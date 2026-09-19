//! Convert each face of a `CompressedTrimmedShell` into the data layout ɴsɪ's
//! `nurbs` node expects (control net + knot vectors + optional `trimcurves.*`
//! block).
//!
//! Lifted from `monster-step-viewer`'s `src/nsi_render/brep.rs`, itself
//! adapted from akatela's `nsi_render/brep.rs`. Its conventions -- domain
//! alignment, face orientation, the 3Delight v-axis flip, periodic seam
//! handling, girdle bands -- were validated against 3Delight renders and are
//! kept as they are. Changes against the viewer:
//!
//! * Every trim curve remembers the boundary edge-use it came from
//!   ([`EdgeUse`]), so the caller can declare weld tables. Curves the
//!   conversion makes up -- closing lines, domain rectangles, seam lines --
//!   come from no edge-use.
//! * Loops that merely trace the parameter domain can be kept
//!   ([`TrimLoopPolicy::KeepAll`]): their edges must stay selectable for
//!   welding.
//! * No `trimcurves.nloops` and no `trimcurves.sense`: the shipped `nurbs`
//!   node has neither, and 3Delight 2.9.210 renders the same with or
//!   without the latter.
//! * A boundary that lies wholly on another lap of a periodic axis is moved
//!   onto the surface's own ([`shift_periodic_trim_entries_into_domain`]).
//!
//! The viewer's notes on its differences from akatela:
//!
//! * Our STEP loader stores the inner 2D trim curve as `Box<Curve2D>` and the
//!   `Curve2D` / `Conic2D` / `Curve3D` enums live in `monstertruck-step` rather
//!   than `monstertruck-modeling` — we deref + import accordingly.
//! * Akatela's 3D-curve fallback uses `Curve::to_parameter_curve_on`. The step
//!   exact analogue is `ExactParameterBoundary2D::exact_parameter_boundary_2d`.
//!   If both embedded and exact projected trims are unavailable, we fall back
//!   to the same sampled parameter boundary used by the mesher and keep a
//!   per-face diagnostic count because those trims can create visible cracks.
use std::{cmp::Ordering, f64::consts::FRAC_PI_2};

use monstertruck::{
    core::{MetricSpace, tolerance::TOLERANCE},
    geometry::prelude::TryIntoHomogeneousBsplineSurface,
    modeling::{
        BoundedCurve, BsplineCurve, KnotVector, Matrix3, Point2, Processor,
        TrimmedCurve, UnitCircle, Vector3,
    },
    step::load::step_geometry::{
        Conic2D, Curve2D, Curve3D, StepParameterCurve, Surface,
    },
    topology::compress::{
        CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace,
    },
    traits::{
        ExactParameterBoundary2D, Invertible, ParameterBoundary2D,
        ParameterDivision1D, ParametricSurface,
        algo::surface::{girdle_axis_and_value, girdle_band_range},
    },
};

use super::loader::StepCompressedShell;

const MAX_RATIONAL_QUADRATIC_ARC: f64 = FRAC_PI_2;
/// Tolerance for sampling 3D curves into 2D parameter-space polylines.
/// Looser than the tessellator's typical `diameter * 0.001` is fine —
/// trim-curve geometry only needs to be accurate enough for the
/// renderer to clip the surface; the surface itself is exact NURBS.
/// Tight tolerances (e.g. 1e-5) make the sampler bail out for circles
/// (`Conic3D::parameter_division` won't run below ~1e-6 after axis
/// scaling, returning None).
const PARAMETER_CURVE_TOLERANCE: f64 = 1.0e-3;
const TRIM_CLOSURE_TOLERANCE: f64 = 1.0e-3;
type StepFaceTrim = CompressedEdgeUse<StepParameterCurve>;
type StepCompressedFace = CompressedTrimmedFace<Surface, StepParameterCurve>;

/// One boundary edge-use of a face: which shared edge it uses, and where
/// in the face's boundaries it sits.
///
/// The edge index is what a weld declaration needs: both faces that share
/// an edge refer to it by the same index into the shell's `edges`. The
/// position tells two uses of one edge on the same face apart -- a seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeUse {
    /// The edge, an index into the shell's `edges`.
    pub edge: usize,
    /// The boundary of the face the use is in.
    pub boundary: usize,
    /// The use's position in that boundary.
    pub position: usize,
}

/// What to do with a trim loop that merely traces the surface's parameter
/// domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimLoopPolicy {
    /// Drop it, as the viewer does: it says nothing the patch's domain does
    /// not already say.
    DropFullDomain,
    /// Keep it: its curves are the face's boundary edges, and a weld
    /// declaration must be able to select them.
    KeepAll,
}

/// Trim-aware NURBS surface in NSI's flat-arrays layout.
#[derive(Clone, Debug, PartialEq)]
pub struct NsiBrepSurfaceData {
    pub face_index: usize,
    pub nu: i32,
    pub nv: i32,
    pub uorder: i32,
    pub vorder: i32,
    pub uknot: Vec<f32>,
    pub vknot: Vec<f32>,
    pub umin: f32,
    pub umax: f32,
    pub vmin: f32,
    pub vmax: f32,
    pub pw: Vec<[f32; 4]>,
    pub trims: Option<NsiBrepTrimData>,
    pub sampled_trim_fallback_count: usize,
}

/// Per-face trim data in NSI's `trimcurves.*` layout.
#[derive(Clone, Debug, PartialEq)]
pub struct NsiBrepTrimData {
    pub ncurves: Vec<i32>,
    pub n: Vec<i32>,
    pub order: Vec<i32>,
    pub knot: Vec<f32>,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    pub w: Vec<f32>,
    /// Per curve, the boundary edge-use it traces; `None` for a curve the
    /// conversion made up.
    pub edge_uses: Vec<Option<EdgeUse>>,
}

#[derive(Clone, Debug, PartialEq)]
struct NsiBrepTrimCurveData {
    n: i32,
    order: i32,
    knot: Vec<f32>,
    min: f32,
    max: f32,
    u: Vec<f32>,
    v: Vec<f32>,
    w: Vec<f32>,
    /// The boundary edge-use the curve traces, if any.
    edge_use: Option<EdgeUse>,
}

#[derive(Clone, Debug, PartialEq)]
struct TrimPiece {
    curves: Vec<NsiBrepTrimCurveData>,
    topology_points: Vec<Point2>,
    closed_by_topology: bool,
    sampled_fallback_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct TrimLoop {
    curves: Vec<NsiBrepTrimCurveData>,
    topology_points: Vec<Point2>,
    sampled_fallback_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrimCurveSource {
    Exact,
    SampledFallback,
}

#[derive(Clone, Debug, PartialEq)]
struct TrimEdgeCurve {
    curve: NsiBrepTrimCurveData,
    topology_points: Vec<Point2>,
    source: TrimCurveSource,
}

impl NsiBrepTrimCurveData {
    fn reverse(&mut self) {
        let min = self.min;
        let max = self.max;
        self.u.reverse();
        self.v.reverse();
        self.w.reverse();
        self.knot = self
            .knot
            .iter()
            .rev()
            .map(|knot| min + max - *knot)
            .collect();
    }

    fn translate_axis(&mut self, axis: usize, shift: f64) {
        let shift = shift as f32;
        match axis {
            0 => self
                .u
                .iter_mut()
                .zip(self.w.iter())
                .for_each(|(u, w)| *u += shift * *w),
            _ => self
                .v
                .iter_mut()
                .zip(self.w.iter())
                .for_each(|(v, w)| *v += shift * *w),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceAxis {
    U,
    V,
}

/// Walk a shell's compressed BRep and emit one `NsiBrepSurfaceData` per face
/// whose surface can be expressed as a homogeneous B-spline AND whose trim
/// boundaries (if any) close cleanly. Faces with non-NURBS surfaces or
/// incomplete trim boundaries are skipped with a warning.
pub fn shell_to_nsi_surfaces(
    shell: &StepCompressedShell,
    policy: TrimLoopPolicy,
) -> Vec<NsiBrepSurfaceData> {
    shell
        .faces
        .iter()
        .enumerate()
        .filter_map(|(face_index, face)| {
            face_to_nsi(face_index, face, &shell.edges, policy)
        })
        .collect()
}

fn face_to_nsi(
    face_index: usize,
    face: &StepCompressedFace,
    edges: &[CompressedEdge<Curve3D>],
    policy: TrimLoopPolicy,
) -> Option<NsiBrepSurfaceData> {
    let mut surface = surface_to_nsi_data(&face.surface)?;
    let has_boundaries =
        face.boundaries.iter().any(|boundary| !boundary.is_empty());
    let mut trim_loops = if has_boundaries {
        trim_loops_from_boundaries(&face.boundaries, edges, &face.surface)
            .or_else(|| {
                log::warn!(
                    "NSI BRep emitter: skipped face {face_index} with incomplete trim boundary"
                );
                None
            })?
    } else {
        None
    };

    if let Some(trim_loop_data) = trim_loops.as_ref()
        && let Some(trim_bbox) = trim_loops_uv_bbox(trim_loop_data)
    {
        align_surface_to_trim_domain(&mut surface, &face.surface, trim_bbox);
    }
    apply_face_orientation_convention(
        &mut surface,
        trim_loops.as_deref_mut(),
        face.orientation,
    );
    apply_three_delight_v_axis_convention(
        &mut surface,
        trim_loops.as_deref_mut(),
    );
    let sampled_trim_fallback_count = trim_loops
        .as_ref()
        .map(|loops| {
            loops
                .iter()
                .map(|trim_loop| trim_loop.sampled_fallback_count)
                .sum()
        })
        .unwrap_or(0);
    if sampled_trim_fallback_count > 0 {
        log::warn!(
            "NSI BRep emitter: face {face_index} uses {sampled_trim_fallback_count} sampled trim fallback(s)"
        );
    }
    // Trim curves are optional in NSI, and a loop that only traces the
    // surface's own parameter domain says nothing the patch's domain does not
    // already say -- the face is untrimmed. Measured on the fixtures, that is
    // 43 of 80 faces on `boxy` and 8 of 17 on `io1-ec-214`, each costing the
    // renderer a trim evaluation for nothing.
    //
    // A weld declaration needs those loops, though: their curves are the
    // face's boundary edges, shared with its neighbours.
    let trims = trim_loops
        .map(|loops| {
            loops
                .into_iter()
                .filter(|trim_loop| {
                    policy == TrimLoopPolicy::KeepAll
                        || !trim_loop_is_full_domain(trim_loop, &surface)
                })
                .collect::<Vec<_>>()
        })
        .filter(|loops| !loops.is_empty())
        .map(trim_loops_to_nsi_data);

    Some(NsiBrepSurfaceData {
        face_index,
        trims,
        sampled_trim_fallback_count,
        ..surface
    })
}

/// Aligns the exported NURBS parameter ranges to the trim curves.
///
/// Non-linear axes only need a knot-domain relabel.
/// Linear two-CV axes also need their homogeneous control points extrapolated.
/// STEP planes and cylinders often use trim parameters outside the unit
/// analytic generator patch.
fn align_surface_to_trim_domain(
    surface: &mut NsiBrepSurfaceData,
    face_surface: &Surface,
    (u_lo, u_hi, v_lo, v_hi): (f32, f32, f32, f32),
) {
    if !align_periodic_surface_seam_to_trim_domain(
        surface,
        SurfaceAxis::U,
        (u_lo, u_hi),
        surface_u_range(face_surface),
        face_surface.period_u(),
    ) {
        let target_u = target_axis_range(
            surface,
            SurfaceAxis::U,
            (u_lo, u_hi),
            surface_u_range(face_surface),
        );
        align_surface_axis(surface, SurfaceAxis::U, target_u.0, target_u.1);
    }
    if !align_periodic_surface_seam_to_trim_domain(
        surface,
        SurfaceAxis::V,
        (v_lo, v_hi),
        surface_v_range(face_surface),
        face_surface.period_v(),
    ) {
        let target_v = target_axis_range(
            surface,
            SurfaceAxis::V,
            (v_lo, v_hi),
            surface_v_range(face_surface),
        );
        align_surface_axis(surface, SurfaceAxis::V, target_v.0, target_v.1);
    }
}

fn align_periodic_surface_seam_to_trim_domain(
    surface: &mut NsiBrepSurfaceData,
    axis: SurfaceAxis,
    trim_range: (f32, f32),
    natural_range: Option<(f32, f32)>,
    period: Option<f64>,
) -> bool {
    let Some(period) = period.map(|period| period as f32) else {
        return false;
    };
    let Some((lower, upper)) = natural_range else {
        return false;
    };
    let straddles_lower_seam = trim_range.0
        < lower - TRIM_CLOSURE_TOLERANCE as f32
        && trim_range.1 > lower + TRIM_CLOSURE_TOLERANCE as f32;
    let natural_span = upper - lower;
    let trim_span = trim_range.1 - trim_range.0;
    let is_full_period =
        (natural_span - period).abs() <= TRIM_CLOSURE_TOLERANCE as f32;
    if surface_axis_is_linear(surface, axis)
        || !straddles_lower_seam
        || !is_full_period
        || trim_span >= period * 0.5
    {
        false
    } else {
        align_surface_axis(surface, axis, lower, upper);
        rotate_periodic_surface_axis_by_half_period(surface, axis, period)
    }
}

fn rotate_periodic_surface_axis_by_half_period(
    surface: &mut NsiBrepSurfaceData,
    axis: SurfaceAxis,
    period: f32,
) -> bool {
    match axis {
        SurfaceAxis::U => {
            rotate_periodic_surface_u_axis_by_half_period(surface)
        }
        SurfaceAxis::V => {
            rotate_periodic_surface_v_axis_by_half_period(surface)
        }
    }
    .then(|| shift_surface_axis_domain(surface, axis, -period * 0.5))
    .is_some()
}

fn rotate_periodic_surface_u_axis_by_half_period(
    surface: &mut NsiBrepSurfaceData,
) -> bool {
    let Some((nu, _)) = surface_dimensions(surface) else {
        return false;
    };
    if !surface.pw.len().is_multiple_of(nu) || nu < 5 || nu.is_multiple_of(2) {
        false
    } else {
        let half = (nu - 1) / 2;
        surface.pw.chunks_mut(nu).for_each(|row| {
            let rotated = row[half..]
                .iter()
                .chain(row[1..=half].iter())
                .copied()
                .collect::<Vec<_>>();
            row.copy_from_slice(&rotated);
        });
        true
    }
}

fn rotate_periodic_surface_v_axis_by_half_period(
    surface: &mut NsiBrepSurfaceData,
) -> bool {
    let Some((nu, nv)) = surface_dimensions(surface) else {
        return false;
    };
    if surface.pw.len() != nu.saturating_mul(nv)
        || nv < 5
        || nv.is_multiple_of(2)
    {
        false
    } else {
        let half = (nv - 1) / 2;
        let rows = surface.pw.chunks(nu).collect::<Vec<_>>();
        surface.pw = rows[half..]
            .iter()
            .chain(rows[1..=half].iter())
            .flat_map(|row| row.iter().copied())
            .collect();
        true
    }
}

fn shift_surface_axis_domain(
    surface: &mut NsiBrepSurfaceData,
    axis: SurfaceAxis,
    shift: f32,
) {
    match axis {
        SurfaceAxis::U => {
            surface.uknot.iter_mut().for_each(|knot| *knot += shift);
            surface.umin += shift;
            surface.umax += shift;
        }
        SurfaceAxis::V => {
            surface.vknot.iter_mut().for_each(|knot| *knot += shift);
            surface.vmin += shift;
            surface.vmax += shift;
        }
    }
}

fn target_axis_range(
    surface: &NsiBrepSurfaceData,
    axis: SurfaceAxis,
    trim_range: (f32, f32),
    natural_range: Option<(f32, f32)>,
) -> (f32, f32) {
    if surface_axis_is_linear(surface, axis) {
        trim_range
    } else {
        natural_range.unwrap_or(trim_range)
    }
}

fn align_surface_axis(
    surface: &mut NsiBrepSurfaceData,
    axis: SurfaceAxis,
    lo: f32,
    hi: f32,
) {
    if surface_axis_is_linear(surface, axis) {
        extrapolate_linear_surface_axis(surface, axis, lo, hi);
    }
    match axis {
        SurfaceAxis::U => {
            rescale_knot_vector(&mut surface.uknot, lo, hi);
            surface.umin = lo;
            surface.umax = hi;
        }
        SurfaceAxis::V => {
            rescale_knot_vector(&mut surface.vknot, lo, hi);
            surface.vmin = lo;
            surface.vmax = hi;
        }
    }
}

fn surface_axis_is_linear(
    surface: &NsiBrepSurfaceData,
    axis: SurfaceAxis,
) -> bool {
    match axis {
        SurfaceAxis::U => surface.nu == 2 && surface.uorder == 2,
        SurfaceAxis::V => surface.nv == 2 && surface.vorder == 2,
    }
}

fn extrapolate_linear_surface_axis(
    surface: &mut NsiBrepSurfaceData,
    axis: SurfaceAxis,
    lo: f32,
    hi: f32,
) {
    let old_range = match axis {
        SurfaceAxis::U => (surface.umin, surface.umax),
        SurfaceAxis::V => (surface.vmin, surface.vmax),
    };
    if (old_range.1 - old_range.0).abs() <= f32::EPSILON {
        return;
    }
    let Some((nu, nv)) = surface_dimensions(surface) else {
        return;
    };
    match axis {
        SurfaceAxis::U => {
            (0..nv).for_each(|v| {
                if let (Some(start_index), Some(end_index)) = (
                    surface_control_index(surface, 0, v),
                    surface_control_index(surface, 1, v),
                ) {
                    let start = surface.pw[start_index];
                    let end = surface.pw[end_index];
                    surface.pw[start_index] =
                        linear_homogeneous_point(start, end, old_range, lo);
                    surface.pw[end_index] =
                        linear_homogeneous_point(start, end, old_range, hi);
                }
            });
        }
        SurfaceAxis::V => {
            (0..nu).for_each(|u| {
                if let (Some(start_index), Some(end_index)) = (
                    surface_control_index(surface, u, 0),
                    surface_control_index(surface, u, 1),
                ) {
                    let start = surface.pw[start_index];
                    let end = surface.pw[end_index];
                    surface.pw[start_index] =
                        linear_homogeneous_point(start, end, old_range, lo);
                    surface.pw[end_index] =
                        linear_homogeneous_point(start, end, old_range, hi);
                }
            });
        }
    }
}

fn linear_homogeneous_point(
    start: [f32; 4],
    end: [f32; 4],
    old_range: (f32, f32),
    target: f32,
) -> [f32; 4] {
    let t = (target - old_range.0) / (old_range.1 - old_range.0);
    [
        start[0] + (end[0] - start[0]) * t,
        start[1] + (end[1] - start[1]) * t,
        start[2] + (end[2] - start[2]) * t,
        start[3] + (end[3] - start[3]) * t,
    ]
}

fn surface_dimensions(surface: &NsiBrepSurfaceData) -> Option<(usize, usize)> {
    Some((
        usize::try_from(surface.nu).ok()?,
        usize::try_from(surface.nv).ok()?,
    ))
}

fn surface_control_index(
    surface: &NsiBrepSurfaceData,
    u: usize,
    v: usize,
) -> Option<usize> {
    let (nu, nv) = surface_dimensions(surface)?;
    (u < nu && v < nv).then_some(v.checked_mul(nu)?.checked_add(u)?)
}

/// Linearly rescale a clamped knot vector to span `[lo, hi]`.
fn rescale_knot_vector(knots: &mut [f32], lo: f32, hi: f32) {
    let Some((kmin, kmax)) =
        knots.iter().copied().fold(None, |acc, k| match acc {
            None => Some((k, k)),
            Some((mn, mx)) => Some((mn.min(k), mx.max(k))),
        })
    else {
        return;
    };
    let span = kmax - kmin;
    if span <= f32::EPSILON {
        return;
    }
    let target_span = hi - lo;
    knots.iter_mut().for_each(|k| {
        *k = lo + (*k - kmin) / span * target_span;
    });
}

/// Compute the (u_min, u_max, v_min, v_max) bounding box covered by all
/// trim curves' control points (after dehomogenising via `u/w, v/w`).
fn trim_loops_uv_bbox(trim_loops: &[TrimLoop]) -> Option<(f32, f32, f32, f32)> {
    let init = (
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
    );
    let bbox = trim_loops
        .iter()
        .flat_map(|trim_loop| trim_loop.curves.iter())
        .flat_map(|curve| {
            curve.u.iter().zip(curve.v.iter()).zip(curve.w.iter())
        })
        .filter(|(_, w)| w.abs() > f32::EPSILON)
        .map(|((u, v), w)| (u / w, v / w))
        .fold(init, |(u_lo, u_hi, v_lo, v_hi), (u, v)| {
            (u_lo.min(u), u_hi.max(u), v_lo.min(v), v_hi.max(v))
        });
    let (u_lo, u_hi, v_lo, v_hi) = bbox;
    (u_lo.is_finite() && u_hi > u_lo && v_hi > v_lo)
        .then_some((u_lo, u_hi, v_lo, v_hi))
}

/// Returns the surface's natural u-range if bounded.
fn surface_u_range(surface: &Surface) -> Option<(f32, f32)> {
    let (u_range, _) = surface.try_range_tuple();
    u_range.map(|(lo, hi)| (lo as f32, hi as f32))
}

/// Returns the surface's natural v-range if bounded.
fn surface_v_range(surface: &Surface) -> Option<(f32, f32)> {
    let (_, v_range) = surface.try_range_tuple();
    v_range.map(|(lo, hi)| (lo as f32, hi as f32))
}

fn surface_to_nsi_data(surface: &Surface) -> Option<NsiBrepSurfaceData> {
    let Some(surface) = surface.try_into_homogeneous_bspline_surface() else {
        log::warn!("NSI BRep emitter: skipped a surface with no B-spline form");
        return None;
    };
    let controls = surface.control_points();
    let nu = i32::try_from(controls.len()).ok()?;
    let nv = i32::try_from(controls.first()?.len()).ok()?;
    let udegree = surface.udegree();
    let vdegree = surface.vdegree();
    let uorder = i32::try_from(udegree.checked_add(1)?).ok()?;
    let vorder = i32::try_from(vdegree.checked_add(1)?).ok()?;
    let nu_usize = usize::try_from(nu).ok()?;
    let nv_usize = usize::try_from(nv).ok()?;
    let uknot_raw = surface.knot_vector_u();
    let vknot_raw = surface.knot_vector_v();
    let umin = *uknot_raw.get(udegree)? as f32;
    let umax = *uknot_raw.get(nu_usize)? as f32;
    let vmin = *vknot_raw.get(vdegree)? as f32;
    let vmax = *vknot_raw.get(nv_usize)? as f32;
    let uknot = uknot_raw.iter().map(|knot| *knot as f32).collect();
    let vknot = vknot_raw.iter().map(|knot| *knot as f32).collect();
    // monstertruck stores rational CVs as `Vector4 = (w*x, w*y, w*z, w)`.
    // NSI's `Pw` uses the same weighted homogeneous layout.
    let pw: Vec<[f32; 4]> = (0..nv_usize)
        .flat_map(|v| {
            (0..nu_usize).map(move |u| {
                let point = controls[u][v];
                [
                    point.x as f32,
                    point.y as f32,
                    point.z as f32,
                    point.w as f32,
                ]
            })
        })
        .collect();

    Some(NsiBrepSurfaceData {
        face_index: 0,
        nu,
        nv,
        uorder,
        vorder,
        uknot,
        vknot,
        umin,
        umax,
        vmin,
        vmax,
        pw,
        trims: None,
        sampled_trim_fallback_count: 0,
    })
}

fn trim_loops_from_boundaries(
    boundaries: &[Vec<StepFaceTrim>],
    edges: &[CompressedEdge<Curve3D>],
    face_surface: &Surface,
) -> Option<Option<Vec<TrimLoop>>> {
    let pieces: Vec<TrimPiece> = boundaries
        .iter()
        .enumerate()
        .filter(|(_, boundary)| !boundary.is_empty())
        .map(|(boundary_index, boundary)| {
            trim_boundary_to_piece(
                boundary_index,
                boundary,
                edges,
                face_surface,
            )
        })
        .collect::<Option<Vec<_>>>()?;

    if pieces.is_empty() {
        Some(None)
    } else {
        let loops = pieces_to_trim_loops(pieces, face_surface);
        if loops.is_empty() {
            None
        } else {
            Some(Some(loops))
        }
    }
}

fn apply_three_delight_v_axis_convention(
    surface: &mut NsiBrepSurfaceData,
    trim_loops: Option<&mut [TrimLoop]>,
) {
    mirror_surface_v_axis(surface);
    if let Some(trim_loops) = trim_loops {
        trim_loops.iter_mut().for_each(|trim_loop| {
            mirror_trim_loop_v_axis(trim_loop, surface.vmin, surface.vmax);
            reverse_trim_loop(trim_loop);
        });
    }
}

fn apply_face_orientation_convention(
    surface: &mut NsiBrepSurfaceData,
    trim_loops: Option<&mut [TrimLoop]>,
    face_orientation: bool,
) {
    if !face_orientation {
        mirror_surface_u_axis(surface);
        if let Some(trim_loops) = trim_loops {
            trim_loops.iter_mut().for_each(|trim_loop| {
                mirror_trim_loop_u_axis(trim_loop, surface.umin, surface.umax);
                reverse_trim_loop(trim_loop);
            });
        }
    }
}

fn mirror_surface_u_axis(surface: &mut NsiBrepSurfaceData) {
    let Some((nu, nv)) = surface_dimensions(surface) else {
        return;
    };
    if surface.pw.len() != nu.saturating_mul(nv) {
        return;
    }
    surface.pw.chunks_mut(nu).for_each(|row| row.reverse());
    let u_origin = surface.umin + surface.umax;
    surface.uknot = surface
        .uknot
        .iter()
        .rev()
        .map(|knot| u_origin - *knot)
        .collect();
}

fn mirror_surface_v_axis(surface: &mut NsiBrepSurfaceData) {
    let Some((nu, nv)) = surface_dimensions(surface) else {
        return;
    };
    if surface.pw.len() != nu.saturating_mul(nv) {
        return;
    }
    let flipped_pw: Vec<[f32; 4]> = surface
        .pw
        .chunks(nu)
        .rev()
        .flat_map(|row| row.iter().copied())
        .collect();
    if flipped_pw.len() == surface.pw.len() {
        surface.pw = flipped_pw;
    }
    let v_origin = surface.vmin + surface.vmax;
    surface.vknot = surface
        .vknot
        .iter()
        .rev()
        .map(|knot| v_origin - *knot)
        .collect();
}

fn mirror_trim_loop_v_axis(trim_loop: &mut TrimLoop, vmin: f32, vmax: f32) {
    let v_origin = vmin + vmax;
    trim_loop.curves.iter_mut().for_each(|curve| {
        curve
            .v
            .iter_mut()
            .zip(curve.w.iter())
            .for_each(|(v, w)| *v = v_origin * *w - *v);
    });
    trim_loop
        .topology_points
        .iter_mut()
        .for_each(|point| point.y = v_origin as f64 - point.y);
}

fn mirror_trim_loop_u_axis(trim_loop: &mut TrimLoop, umin: f32, umax: f32) {
    let u_origin = umin + umax;
    trim_loop.curves.iter_mut().for_each(|curve| {
        curve
            .u
            .iter_mut()
            .zip(curve.w.iter())
            .for_each(|(u, w)| *u = u_origin * *w - *u);
    });
    trim_loop
        .topology_points
        .iter_mut()
        .for_each(|point| point.x = u_origin as f64 - point.x);
}

/// Whether a trim loop merely traces the surface's parameter domain.
///
/// Such a loop bounds the whole patch, which the patch's own domain already
/// does. Detected as every point sitting on the domain perimeter with all four
/// corners visited, so a loop that dips inside -- a real trim -- is kept.
fn trim_loop_is_full_domain(
    trim_loop: &TrimLoop,
    surface: &NsiBrepSurfaceData,
) -> bool {
    let near = |value: f64, edge: f32| {
        (value - edge as f64).abs() <= TRIM_CLOSURE_TOLERANCE
    };
    let corners = [
        (surface.umin, surface.vmin),
        (surface.umax, surface.vmin),
        (surface.umax, surface.vmax),
        (surface.umin, surface.vmax),
    ];

    !trim_loop.topology_points.is_empty()
        && trim_loop.topology_points.iter().all(|point| {
            near(point.x, surface.umin)
                || near(point.x, surface.umax)
                || near(point.y, surface.vmin)
                || near(point.y, surface.vmax)
        })
        && corners.iter().all(|(corner_u, corner_v)| {
            trim_loop.topology_points.iter().any(|point| {
                near(point.x, *corner_u) && near(point.y, *corner_v)
            })
        })
}

fn trim_loops_to_nsi_data(mut loops: Vec<TrimLoop>) -> NsiBrepTrimData {
    loops.iter_mut().for_each(snap_trim_loop_curve_endpoints);
    let ncurves = loops
        .iter()
        .map(|trim_loop| trim_loop.curves.len() as i32)
        .collect();
    let curves: Vec<NsiBrepTrimCurveData> = loops
        .into_iter()
        .flat_map(|trim_loop| trim_loop.curves)
        .collect();

    NsiBrepTrimData {
        ncurves,
        n: curves.iter().map(|curve| curve.n).collect(),
        order: curves.iter().map(|curve| curve.order).collect(),
        knot: curves
            .iter()
            .flat_map(|curve| curve.knot.iter().copied())
            .collect(),
        min: curves.iter().map(|curve| curve.min).collect(),
        max: curves.iter().map(|curve| curve.max).collect(),
        u: curves
            .iter()
            .flat_map(|curve| curve.u.iter().copied())
            .collect(),
        v: curves
            .iter()
            .flat_map(|curve| curve.v.iter().copied())
            .collect(),
        w: curves
            .iter()
            .flat_map(|curve| curve.w.iter().copied())
            .collect(),
        edge_uses: curves.iter().map(|curve| curve.edge_use).collect(),
    }
}

fn snap_trim_loop_curve_endpoints(trim_loop: &mut TrimLoop) {
    let end_points = trim_loop
        .curves
        .iter()
        .map(trim_curve_back_point)
        .collect::<Vec<_>>();
    let len = trim_loop.curves.len();
    (0..len).for_each(|index| {
        let next_index = (index + 1) % len;
        if let Some(end_point) = end_points[index]
            && let Some(next_start) =
                trim_curve_front_point(&trim_loop.curves[next_index])
            && uv_distance(end_point, next_start) <= TRIM_CLOSURE_TOLERANCE
        {
            set_trim_curve_front_point(
                &mut trim_loop.curves[next_index],
                end_point,
            );
        }
    });
}

fn trim_curve_front_point(curve: &NsiBrepTrimCurveData) -> Option<Point2> {
    trim_curve_point(curve, 0)
}

fn trim_curve_back_point(curve: &NsiBrepTrimCurveData) -> Option<Point2> {
    curve
        .u
        .len()
        .checked_sub(1)
        .and_then(|index| trim_curve_point(curve, index))
}

fn trim_curve_point(
    curve: &NsiBrepTrimCurveData,
    index: usize,
) -> Option<Point2> {
    let u = *curve.u.get(index)?;
    let v = *curve.v.get(index)?;
    let w = *curve.w.get(index)?;
    (w.abs() > f32::EPSILON)
        .then(|| Point2::new(u as f64 / w as f64, v as f64 / w as f64))
}

fn set_trim_curve_front_point(curve: &mut NsiBrepTrimCurveData, point: Point2) {
    if let (Some(u), Some(v), Some(w)) =
        (curve.u.first_mut(), curve.v.first_mut(), curve.w.first())
    {
        *u = point.x as f32 * *w;
        *v = point.y as f32 * *w;
    }
}

fn trim_boundary_to_piece(
    boundary_index: usize,
    boundary: &[StepFaceTrim],
    edges: &[CompressedEdge<Curve3D>],
    face_surface: &Surface,
) -> Option<TrimPiece> {
    let mut entries: Vec<TrimEdgeCurve> = boundary
        .iter()
        .enumerate()
        .map(|(position, edge_use)| {
            let mut entry = trim_edge_to_curve(edge_use, edges, face_surface)?;
            entry.curve.edge_use = Some(EdgeUse {
                edge: edge_use.index,
                boundary: boundary_index,
                position,
            });
            Some(entry)
        })
        .collect::<Option<Vec<_>>>()?;
    align_periodic_trim_entries(&mut entries, face_surface);
    shift_periodic_trim_entries_into_domain(&mut entries, face_surface);
    let curves = entries
        .iter()
        .map(|entry| entry.curve.clone())
        .collect::<Vec<_>>();
    let sampled_fallback_count = entries
        .iter()
        .filter(|entry| entry.source == TrimCurveSource::SampledFallback)
        .count();
    let concatenated = entries
        .into_iter()
        .map(|entry| entry.topology_points)
        .fold(Vec::<Point2>::new(), |mut acc, mut boundary| {
            if !acc.is_empty() && !boundary.is_empty() {
                boundary.remove(0);
            }
            acc.extend(boundary);
            acc
        });
    let (topology_points, closed_by_topology) =
        parameter_boundary_to_topology_piece(face_surface, concatenated)?;
    Some(TrimPiece {
        curves,
        topology_points,
        closed_by_topology,
        sampled_fallback_count,
    })
}

fn align_periodic_trim_entries(
    entries: &mut [TrimEdgeCurve],
    surface: &Surface,
) {
    (0..=1).for_each(|axis| {
        let (period, range) = surface_axis_range(surface, axis);
        if let Some(period) = period
            && entries_have_periodic_jump(entries, axis, period)
            && let Some(shifts) =
                periodic_trim_entry_shifts(entries, axis, period, range)
        {
            entries.iter_mut().zip(shifts).for_each(|(entry, shift)| {
                translate_trim_entry_axis(entry, axis, shift)
            });
        }
    });
}

/// Moves a boundary that lies wholly on another lap of a periodic axis onto
/// the surface's own, by whole periods.
///
/// Not in the viewer. `monstertruck`'s exact trims can sit whole periods
/// away from the natural range -- on `io1-ec-214`, four cylinder faces
/// bounded by a seam and two circles come out at u in [6π, 8π] or
/// [-4π, -2π] against a [0, 2π] surface. With no jump inside the boundary,
/// [`align_periodic_trim_entries`] leaves them there, and 3Delight 2.9.210
/// does not wrap trims: the whole face is trimmed away. Only a boundary
/// with no overlap at all is moved, so every boundary that renders keeps
/// the lap it had.
fn shift_periodic_trim_entries_into_domain(
    entries: &mut [TrimEdgeCurve],
    surface: &Surface,
) {
    (0..=1).for_each(|axis| {
        let (period, range) = surface_axis_range(surface, axis);
        let (Some(period), Some((lower, upper))) = (period, range) else {
            return;
        };
        let Some((min, max)) = curve_axis_span(
            &entries
                .iter()
                .flat_map(|entry| entry.topology_points.iter().copied())
                .collect::<Vec<_>>(),
            axis,
        ) else {
            return;
        };
        if max <= lower + TOLERANCE || upper - TOLERANCE <= min {
            let shift = ((lower - min) / period).round() * period;
            entries.iter_mut().for_each(|entry| {
                translate_trim_entry_axis(entry, axis, shift)
            });
        }
    });
}

fn entries_have_periodic_jump(
    entries: &[TrimEdgeCurve],
    axis: usize,
    period: f64,
) -> bool {
    entries.iter().any(|entry| {
        entry.topology_points.windows(2).any(|points| {
            (uv_axis(points[1], axis) - uv_axis(points[0], axis)).abs()
                > period * 0.5
        })
    }) || entries
        .iter()
        .zip(entries.iter().cycle().skip(1))
        .take(entries.len())
        .any(|(current, next)| {
            current
                .topology_points
                .last()
                .zip(next.topology_points.first())
                .is_some_and(|(current, next)| {
                    (uv_axis(*next, axis) - uv_axis(*current, axis)).abs()
                        > period * 0.5
                })
        })
}

fn periodic_trim_entry_shifts(
    entries: &[TrimEdgeCurve],
    axis: usize,
    period: f64,
    range: Option<(f64, f64)>,
) -> Option<Vec<f64>> {
    let best = (-2..=2)
        .filter_map(|lap| {
            let initial_shift = lap as f64 * period;
            periodic_trim_entry_shift_candidate(
                entries,
                axis,
                period,
                range,
                initial_shift,
            )
        })
        .min_by(periodic_shift_score_cmp);
    best.map(|candidate| candidate.shifts)
}

#[derive(Clone, Debug)]
struct PeriodicTrimEntryShiftCandidate {
    shifts: Vec<f64>,
    closure_error: f64,
    domain_overflow: f64,
    span: f64,
}

fn periodic_trim_entry_shift_candidate(
    entries: &[TrimEdgeCurve],
    axis: usize,
    period: f64,
    range: Option<(f64, f64)>,
    initial_shift: f64,
) -> Option<PeriodicTrimEntryShiftCandidate> {
    let first_entry = entries.first()?;
    let first_start =
        uv_axis(*first_entry.topology_points.first()?, axis) + initial_shift;
    let init = (
        Vec::with_capacity(entries.len()),
        None,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
    );
    let (shifts, previous_end, min, max, domain_overflow) =
        entries.iter().enumerate().try_fold(
            init,
            |(mut shifts, previous_end, min, max, overflow), (index, entry)| {
                let shift = if index == 0 {
                    initial_shift
                } else {
                    let previous_end = previous_end?;
                    closest_periodic_shift(
                        uv_axis(*entry.topology_points.first()?, axis),
                        previous_end,
                        period,
                    )
                };
                let (entry_min, entry_max, entry_overflow) =
                    shifted_entry_axis_metrics(entry, axis, shift, range)?;
                shifts.push(shift);
                Some((
                    shifts,
                    entry
                        .topology_points
                        .last()
                        .map(|point| uv_axis(*point, axis) + shift),
                    min.min(entry_min),
                    max.max(entry_max),
                    overflow + entry_overflow,
                ))
            },
        )?;
    let previous_end = previous_end?;
    Some(PeriodicTrimEntryShiftCandidate {
        shifts,
        closure_error: (previous_end - first_start).abs(),
        domain_overflow,
        span: max - min,
    })
}

fn periodic_shift_score_cmp(
    lhs: &PeriodicTrimEntryShiftCandidate,
    rhs: &PeriodicTrimEntryShiftCandidate,
) -> Ordering {
    lhs.closure_error
        .total_cmp(&rhs.closure_error)
        .then(lhs.domain_overflow.total_cmp(&rhs.domain_overflow))
        .then(lhs.span.total_cmp(&rhs.span))
}

fn closest_periodic_shift(value: f64, target: f64, period: f64) -> f64 {
    (-4..=4)
        .map(|lap| lap as f64 * period)
        .min_by(|lhs, rhs| {
            (value + *lhs - target)
                .abs()
                .total_cmp(&(value + *rhs - target).abs())
        })
        .unwrap_or(0.0)
}

fn shifted_entry_axis_metrics(
    entry: &TrimEdgeCurve,
    axis: usize,
    shift: f64,
    range: Option<(f64, f64)>,
) -> Option<(f64, f64, f64)> {
    entry
        .topology_points
        .iter()
        .map(|point| uv_axis(*point, axis) + shift)
        .try_fold(
            (f64::INFINITY, f64::NEG_INFINITY, 0.0),
            |(min, max, overflow), value| {
                value.is_finite().then(|| {
                    (
                        min.min(value),
                        max.max(value),
                        overflow + axis_domain_overflow(value, range),
                    )
                })
            },
        )
}

fn axis_domain_overflow(value: f64, range: Option<(f64, f64)>) -> f64 {
    range
        .map(|(min, max)| {
            if value < min {
                min - value
            } else if value > max {
                value - max
            } else {
                0.0
            }
        })
        .unwrap_or(0.0)
}

fn translate_trim_entry_axis(
    entry: &mut TrimEdgeCurve,
    axis: usize,
    shift: f64,
) {
    if shift.abs() > TOLERANCE {
        entry.curve.translate_axis(axis, shift);
        entry.topology_points.iter_mut().for_each(|point| {
            set_uv_axis(point, axis, uv_axis(*point, axis) + shift)
        });
    }
}

fn trim_edge_to_curve(
    edge_use: &StepFaceTrim,
    edges: &[CompressedEdge<Curve3D>],
    face_surface: &Surface,
) -> Option<TrimEdgeCurve> {
    let exact = if let Some(trim_curve) = edge_use.trim_curve.as_ref() {
        let curve_2d = (**trim_curve.curve()).clone();
        if let Some(edge) = edges.get(edge_use.index) {
            trim_curve_to_edge_curve(
                curve_2d,
                edge_use,
                edge,
                face_surface,
                TrimCurveSource::Exact,
            )
        } else {
            trim_curve_to_oriented_curve(
                curve_2d,
                edge_use.orientation,
                face_surface,
                TrimCurveSource::Exact,
            )
        }
    } else {
        edges.get(edge_use.index).and_then(|edge| {
            edge.curve
                .exact_parameter_boundary_2d(face_surface)
                .map(|trim_curve: StepParameterCurve| {
                    (**trim_curve.curve()).clone()
                })
                .and_then(|curve_2d| {
                    trim_curve_to_edge_curve(
                        curve_2d,
                        edge_use,
                        edge,
                        face_surface,
                        TrimCurveSource::Exact,
                    )
                })
        })
    };

    exact.or_else(|| sampled_trim_edge_to_curve(edge_use, edges, face_surface))
}

fn trim_curve_to_oriented_curve(
    curve_2d: Curve2D,
    orientation: bool,
    face_surface: &Surface,
    source: TrimCurveSource,
) -> Option<TrimEdgeCurve> {
    let curve_2d = if orientation {
        curve_2d
    } else {
        curve_2d.inverse()
    };
    let topology_points = curve_to_topology_boundary(&curve_2d, face_surface)?;
    let bspline = curve2d_to_homogeneous_bspline(&curve_2d)?;
    let curve = bspline_curve_to_trim_curve(&bspline)?;
    Some(TrimEdgeCurve {
        curve,
        topology_points,
        source,
    })
}

fn trim_curve_to_edge_curve(
    curve_2d: Curve2D,
    edge_use: &StepFaceTrim,
    edge: &CompressedEdge<Curve3D>,
    face_surface: &Surface,
    source: TrimCurveSource,
) -> Option<TrimEdgeCurve> {
    let topology_points = curve_to_topology_boundary(&curve_2d, face_surface)?;
    let (curve_2d, topology_points) = if trim_boundary_should_reverse_to_edge(
        &topology_points,
        &edge.curve,
        face_surface,
        edge_use.orientation,
    ) {
        let reversed = curve_2d.inverse();
        let topology_points =
            curve_to_topology_boundary(&reversed, face_surface)?;
        (reversed, topology_points)
    } else {
        (curve_2d, topology_points)
    };
    let bspline = curve2d_to_homogeneous_bspline(&curve_2d)?;
    let curve = bspline_curve_to_trim_curve(&bspline)?;
    Some(TrimEdgeCurve {
        curve,
        topology_points,
        source,
    })
}

fn trim_boundary_should_reverse_to_edge(
    boundary: &[Point2],
    edge_curve: &Curve3D,
    face_surface: &Surface,
    orientation: bool,
) -> bool {
    let (edge_front, edge_back) = if orientation {
        (edge_curve.front(), edge_curve.back())
    } else {
        (edge_curve.back(), edge_curve.front())
    };
    if edge_front.distance2(edge_back) <= TOLERANCE * TOLERANCE {
        false
    } else {
        boundary.first().zip(boundary.last()).is_some_and(
            |(front_uv, back_uv)| {
                let front_point = face_surface.evaluate(front_uv.x, front_uv.y);
                let back_point = face_surface.evaluate(back_uv.x, back_uv.y);
                let direct = front_point.distance2(edge_front)
                    + back_point.distance2(edge_back);
                let reversed = front_point.distance2(edge_back)
                    + back_point.distance2(edge_front);
                reversed < direct
            },
        )
    }
}

fn sampled_trim_edge_to_curve(
    edge_use: &StepFaceTrim,
    edges: &[CompressedEdge<Curve3D>],
    face_surface: &Surface,
) -> Option<TrimEdgeCurve> {
    let edge = edges.get(edge_use.index)?;
    let mut topology_points = edge
        .curve
        .parameter_boundary_2d(face_surface, PARAMETER_CURVE_TOLERANCE)?;
    if trim_boundary_should_reverse_to_edge(
        &topology_points,
        &edge.curve,
        face_surface,
        edge_use.orientation,
    ) {
        topology_points.reverse();
    }
    let bspline = polyline_to_bspline(&topology_points)?;
    let curve = bspline_curve_to_trim_curve(&bspline)?;
    Some(TrimEdgeCurve {
        curve,
        topology_points,
        source: TrimCurveSource::SampledFallback,
    })
}

fn curve_to_topology_boundary(
    curve: &Curve2D,
    _face_surface: &Surface,
) -> Option<Vec<Point2>> {
    let boundary = match curve {
        Curve2D::Line(line) => vec![line.0, line.1],
        Curve2D::Polyline(polyline) => polyline.0.clone(),
        _ => {
            curve
                .parameter_division(
                    curve.range_tuple(),
                    PARAMETER_CURVE_TOLERANCE,
                )
                .1
        }
    };
    (!boundary.is_empty()).then_some(boundary)
}

fn parameter_boundary_to_topology_piece(
    _surface: &Surface,
    boundary: Vec<Point2>,
) -> Option<(Vec<Point2>, bool)> {
    close_topology_piece(boundary)
}

fn close_topology_piece(points: Vec<Point2>) -> Option<(Vec<Point2>, bool)> {
    let mut points = points.into_iter().fold(Vec::new(), |mut acc, point| {
        if acc.last().is_none_or(|last| !uv_near(*last, point)) {
            acc.push(point);
        }
        acc
    });
    if points.is_empty() {
        None
    } else {
        let closed_by_topology = points
            .first()
            .zip(points.last())
            .is_some_and(|(front, back)| !uv_near(*front, *back));
        if closed_by_topology {
            points.push(points[0]);
        }
        Some((points, closed_by_topology))
    }
}

/// Replaces a face boundary made entirely of girdle pieces with the
/// axis-aligned parameter rectangle they bound.
///
/// A girdle encloses no area of its own — the region it bounds on a closed
/// surface is the band between girdles (or between a lone girdle and the
/// surface's apex). Emitting girdles directly produces zero-area loops on
/// whatever parameter lap they were authored on — usually outside the
/// exported domain, where renderers clamp them into visible slivers — while
/// the fallback outer loop lands on the surface's natural rectangle, which
/// sits at the wrong place along the band axis. The rectangle is exact
/// because girdles are constant-parameter curves.
fn girdle_pieces_to_band_loop(
    pieces: &[TrimPiece],
    surface: &Surface,
) -> Option<TrimLoop> {
    let girdles: Vec<(usize, f64)> = pieces
        .iter()
        .map(|piece| {
            // Girdles close in 3D only; pieces that already close in
            // parameter space enclose real area and must be kept as authored.
            piece
                .closed_by_topology
                .then(|| {
                    let points: Vec<(f64, f64)> = piece
                        .topology_points
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect();
                    girdle_axis_and_value(
                        &points,
                        surface,
                        TRIM_CLOSURE_TOLERANCE,
                    )
                })
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    let (axis, (band_lo, band_hi)) =
        girdle_band_range(&girdles, surface, TRIM_CLOSURE_TOLERANCE)?;
    let (_, periodic_range) = surface_axis_range(surface, axis);
    let (periodic_lo, periodic_hi) = periodic_range?;
    let corner = |periodic: f64, band: f64| match axis {
        0 => Point2::new(periodic, band),
        _ => Point2::new(band, periodic),
    };
    let points = [
        corner(periodic_lo, band_lo),
        corner(periodic_hi, band_lo),
        corner(periodic_hi, band_hi),
        corner(periodic_lo, band_hi),
    ];
    // A band side at a girdle's value traces that girdle: a
    // constant-parameter curve over the full period. It keeps the girdle's
    // edge-use when the girdle is one edge; a girdle split over several
    // edges has no single use for the side to stand in for.
    let girdle_edge_use = |band: f64| {
        pieces
            .iter()
            .zip(&girdles)
            .find(|(_, (_, value))| {
                (value - band).abs() <= TRIM_CLOSURE_TOLERANCE
            })
            .and_then(|(piece, _)| match piece.curves.as_slice() {
                [curve] => curve.edge_use,
                _ => None,
            })
    };
    let with_edge_use = |mut curve: NsiBrepTrimCurveData,
                         edge_use: Option<EdgeUse>| {
        curve.edge_use = edge_use;
        curve
    };
    let mut band_loop = TrimLoop {
        curves: vec![
            with_edge_use(
                line_trim_curve(points[0], points[1]),
                girdle_edge_use(band_lo),
            ),
            line_trim_curve(points[1], points[2]),
            with_edge_use(
                line_trim_curve(points[2], points[3]),
                girdle_edge_use(band_hi),
            ),
            line_trim_curve(points[3], points[0]),
        ],
        topology_points: connect_edges([
            uv_line(points[0], points[1]),
            uv_line(points[1], points[2]),
            uv_line(points[2], points[3]),
            uv_line(points[3], points[0]),
        ]),
        sampled_fallback_count: pieces
            .iter()
            .map(|piece| piece.sampled_fallback_count)
            .sum(),
    };
    if !loop_orientation(&band_loop.topology_points) {
        reverse_trim_loop(&mut band_loop);
    }
    Some(band_loop)
}

fn pieces_to_trim_loops(
    pieces: Vec<TrimPiece>,
    surface: &Surface,
) -> Vec<TrimLoop> {
    if let Some(band_loop) = girdle_pieces_to_band_loop(&pieces, surface) {
        return vec![band_loop];
    }
    let (mut closed, mut open) = (Vec::new(), Vec::new());
    pieces.into_iter().for_each(|mut piece| {
        if piece
            .topology_points
            .first()
            .zip(piece.topology_points.last())
            .is_some_and(|(front, back)| {
                uv_distance(*front, *back) < TRIM_CLOSURE_TOLERANCE
            })
        {
            piece.topology_points.pop();
            if piece.closed_by_topology
                && let (Some(front), Some(back)) = (
                    piece.topology_points.first().copied(),
                    piece.topology_points.last().copied(),
                )
            {
                piece.curves.push(line_trim_curve(back, front));
            }
            closed.push(TrimLoop {
                curves: piece.curves,
                topology_points: piece.topology_points,
                sampled_fallback_count: piece.sampled_fallback_count,
            });
        } else if let Some((axis, period)) =
            periodic_axis_full_span(&piece.topology_points, surface)
        {
            piece.topology_points = open_periodic_closed_loop(
                piece.topology_points,
                surface,
                axis,
                period,
            );
            open.push(piece);
        } else {
            open.push(piece);
        }
    });
    open.retain(|piece| piece.topology_points.len() >= 2);
    closed.retain(|trim_loop| trim_loop.topology_points.len() >= 2);
    connect_open_mesher_pieces(&mut closed, open, surface);
    if closed.len() == 1 && !loop_orientation(&closed[0].topology_points) {
        reverse_trim_loop(&mut closed[0]);
    }
    if !closed
        .iter()
        .any(|trim_loop| loop_orientation(&trim_loop.topology_points))
        && let (Some((u0, u1)), Some((v0, v1))) = surface.try_range_tuple()
    {
        let points = [
            Point2::new(u0, v0),
            Point2::new(u1, v0),
            Point2::new(u1, v1),
            Point2::new(u0, v1),
        ];
        closed.push(TrimLoop {
            curves: vec![
                line_trim_curve(points[0], points[1]),
                line_trim_curve(points[1], points[2]),
                line_trim_curve(points[2], points[3]),
                line_trim_curve(points[3], points[0]),
            ],
            topology_points: connect_edges([
                uv_line(points[0], points[1]),
                uv_line(points[1], points[2]),
                uv_line(points[2], points[3]),
                uv_line(points[3], points[0]),
            ]),
            sampled_fallback_count: 0,
        });
    }
    normalize_closed_loop_winding(&mut closed);
    closed
}

fn connect_open_mesher_pieces(
    closed: &mut Vec<TrimLoop>,
    mut open: Vec<TrimPiece>,
    surface: &Surface,
) {
    match open.len() {
        1 => {
            let Some(mut curve) = open.pop() else {
                return;
            };
            let p = curve.topology_points[0];
            let q = curve.topology_points[curve.topology_points.len() - 1];
            if let (Some((u0, u1)), Some((v0, v1))) = surface.try_range_tuple()
            {
                if p.x < q.x - TOLERANCE {
                    normalize_piece_range(&mut curve, 0, (u0, u1));
                    let p = curve.topology_points[0];
                    let q =
                        curve.topology_points[curve.topology_points.len() - 1];
                    let x = Point2::new(u0, v1);
                    let y = Point2::new(u1, v1);
                    closed.push(prefix_closure_loop(curve, [q, y, x, p]));
                } else if q.x < p.x - TOLERANCE {
                    normalize_piece_range(&mut curve, 0, (u0, u1));
                    let p = curve.topology_points[0];
                    let q =
                        curve.topology_points[curve.topology_points.len() - 1];
                    let x = Point2::new(u1, v0);
                    let y = Point2::new(u0, v0);
                    closed.push(prefix_closure_loop(curve, [q, y, x, p]));
                } else if p.y < q.y - TOLERANCE {
                    normalize_piece_range(&mut curve, 1, (v0, v1));
                    let p = curve.topology_points[0];
                    let q =
                        curve.topology_points[curve.topology_points.len() - 1];
                    let x = Point2::new(u0, v0);
                    let y = Point2::new(u0, v1);
                    closed.push(prefix_closure_loop(curve, [q, y, x, p]));
                } else if q.y < p.y - TOLERANCE {
                    normalize_piece_range(&mut curve, 1, (v0, v1));
                    let p = curve.topology_points[0];
                    let q =
                        curve.topology_points[curve.topology_points.len() - 1];
                    let x = Point2::new(u1, v1);
                    let y = Point2::new(u1, v0);
                    closed.push(prefix_closure_loop(curve, [q, y, x, p]));
                }
            }
        }
        2 => {
            let (Some(mut curve1), Some(mut curve0)) = (open.pop(), open.pop())
            else {
                return;
            };
            let ((p0, p1), (q0, q1)) = (
                end_points(&curve0.topology_points),
                end_points(&curve1.topology_points),
            );
            if !f64_near(p0.x, p1.x) && !f64_near(q0.x, q1.x) {
                if let Some(period) = surface.period_u() {
                    align_periodic_open_piece_pair(
                        &curve0,
                        &mut curve1,
                        0,
                        period,
                    );
                } else if let (Some(urange), _) = surface.try_range_tuple() {
                    normalize_piece_range(&mut curve0, 0, urange);
                    normalize_piece_range(&mut curve1, 0, urange);
                }
            } else if !f64_near(p0.y, p1.y) && !f64_near(q0.y, q1.y) {
                if let Some(period) = surface.period_v() {
                    align_periodic_open_piece_pair(
                        &curve0,
                        &mut curve1,
                        1,
                        period,
                    );
                } else if let (_, Some(vrange)) = surface.try_range_tuple() {
                    normalize_piece_range(&mut curve0, 1, vrange);
                    normalize_piece_range(&mut curve1, 1, vrange);
                }
            }
            let ((p0, p1), (q0, q1)) = (
                end_points(&curve0.topology_points),
                end_points(&curve1.topology_points),
            );
            let mut curves = curve0.curves;
            curves.push(line_trim_curve(p1, q0));
            curves.extend(curve1.curves);
            curves.push(line_trim_curve(q1, p0));
            closed.push(TrimLoop {
                curves,
                topology_points: connect_edges([
                    curve0.topology_points,
                    uv_line(p1, q0),
                    curve1.topology_points,
                    uv_line(q1, p0),
                ]),
                sampled_fallback_count: curve0.sampled_fallback_count
                    + curve1.sampled_fallback_count,
            });
        }
        _ => {}
    }
}

fn end_points(points: &[Point2]) -> (Point2, Point2) {
    (points[0], points[points.len() - 1])
}

fn prefix_closure_loop(mut piece: TrimPiece, closure: [Point2; 4]) -> TrimLoop {
    let [q, y, x, p] = closure;
    let mut curves = vec![
        line_trim_curve(q, y),
        line_trim_curve(y, x),
        line_trim_curve(x, p),
    ];
    curves.append(&mut piece.curves);
    TrimLoop {
        curves,
        topology_points: connect_edges([
            uv_line(q, y),
            uv_line(y, x),
            uv_line(x, p),
            piece.topology_points,
        ]),
        sampled_fallback_count: piece.sampled_fallback_count,
    }
}

fn connect_edges(vecs: impl IntoIterator<Item = Vec<Point2>>) -> Vec<Point2> {
    vecs.into_iter()
        .flat_map(|vec| {
            let len = vec.len();
            vec.into_iter().take(len.saturating_sub(1))
        })
        .collect()
}

fn uv_line(front: Point2, back: Point2) -> Vec<Point2> {
    vec![front, back]
}

fn line_trim_curve(front: Point2, back: Point2) -> NsiBrepTrimCurveData {
    NsiBrepTrimCurveData {
        n: 2,
        order: 2,
        knot: vec![0.0, 0.0, 1.0, 1.0],
        min: 0.0,
        max: 1.0,
        u: vec![front.x as f32, back.x as f32],
        v: vec![front.y as f32, back.y as f32],
        w: vec![1.0, 1.0],
        edge_use: None,
    }
}

fn normalize_axis(
    value: f64,
    previous: Option<f64>,
    period: Option<f64>,
    range: Option<(f64, f64)>,
) -> Option<f64> {
    if !value.is_finite() {
        None
    } else if let Some(previous) = previous {
        if let Some(period) = period {
            (-2..=2).map(|index| value + index as f64 * period).min_by(
                |lhs, rhs| {
                    (lhs - previous).abs().total_cmp(&(rhs - previous).abs())
                },
            )
        } else {
            Some(value)
        }
    } else if let Some((min, max)) = range {
        if let Some(period) = period {
            let span = max - min;
            if span.abs() <= TOLERANCE {
                Some(min)
            } else {
                let mut normalized =
                    value - f64::floor((value - min) / period) * period;
                if normalized < min {
                    normalized += period;
                }
                if normalized > max {
                    normalized -= period;
                }
                Some(normalized.clamp(min, max))
            }
        } else {
            Some(value)
        }
    } else {
        Some(value)
    }
}

fn normalize_range(curve: &mut Vec<Point2>, axis: usize, (u0, u1): (f64, f64)) {
    if curve.len() < 2 || (u1 - u0).abs() <= TOLERANCE {
        return;
    }
    let p = curve[0];
    let q = curve[curve.len() - 1];
    let tmp = f64::min(uv_axis(p, axis), uv_axis(q, axis)) + TOLERANCE;
    let span = u1 - u0;
    let del = f64::floor((tmp - u0) / span) * span;
    curve.iter_mut().for_each(|point| {
        set_uv_axis(point, axis, uv_axis(*point, axis) - del)
    });
    let Some(index) = curve.iter().position(|point| {
        (uv_axis(curve[0], axis) - u1) * (uv_axis(*point, axis) - u1) < 0.0
    }) else {
        return;
    };
    let mut curve1 = curve.split_off(index + 1);
    curve1.pop();
    curve1.insert(0, curve[index]);
    if uv_axis(curve[0], axis) < uv_axis(curve[curve.len() - 1], axis) {
        curve1.iter_mut().for_each(|point| {
            set_uv_axis(point, axis, uv_axis(*point, axis) - span);
        });
    } else {
        curve.iter_mut().for_each(|point| {
            set_uv_axis(point, axis, uv_axis(*point, axis) - span);
        });
    }
    curve1.append(curve);
    *curve = curve1;
}

fn normalize_piece_range(
    piece: &mut TrimPiece,
    axis: usize,
    range: (f64, f64),
) {
    let before = piece.topology_points.clone();
    normalize_range(&mut piece.topology_points, axis, range);
    let shift = before
        .first()
        .zip(piece.topology_points.first())
        .map(|(before, after)| uv_axis(*after, axis) - uv_axis(*before, axis))
        .unwrap_or(0.0);
    if shift.abs() > TOLERANCE {
        piece
            .curves
            .iter_mut()
            .for_each(|curve| curve.translate_axis(axis, shift));
    }
}

fn loop_signed_area(curve: &[Point2]) -> f64 {
    curve
        .windows(2)
        .map(|points| (points[1].x + points[0].x) * (points[1].y - points[0].y))
        .sum::<f64>()
        + curve
            .first()
            .zip(curve.last())
            .map(|(front, back)| (front.x + back.x) * (front.y - back.y))
            .unwrap_or(0.0)
}

fn loop_orientation(curve: &[Point2]) -> bool {
    loop_signed_area(curve) > 0.0
}

fn normalize_closed_loop_winding(closed: &mut [TrimLoop]) {
    let outer_index = closed
        .iter()
        .enumerate()
        .max_by(|(_, lhs), (_, rhs)| {
            loop_signed_area(&lhs.topology_points)
                .abs()
                .total_cmp(&loop_signed_area(&rhs.topology_points).abs())
        })
        .map(|(index, _)| index);
    if let Some(outer_index) = outer_index {
        closed
            .iter_mut()
            .enumerate()
            .for_each(|(index, trim_loop)| {
                let should_be_positive = index == outer_index;
                if loop_orientation(&trim_loop.topology_points)
                    != should_be_positive
                {
                    reverse_trim_loop(trim_loop);
                }
            });
    }
}

fn reverse_trim_loop(trim_loop: &mut TrimLoop) {
    trim_loop.topology_points.reverse();
    trim_loop.curves.reverse();
    trim_loop
        .curves
        .iter_mut()
        .for_each(NsiBrepTrimCurveData::reverse);
}

fn periodic_axis_full_span(
    curve: &[Point2],
    surface: &Surface,
) -> Option<(usize, f64)> {
    let closed = curve
        .first()
        .zip(curve.last())
        .is_some_and(|(front, back)| uv_near(*front, *back));
    if curve.len() < 4 || !closed {
        None
    } else {
        [(0usize, surface.period_u()), (1usize, surface.period_v())]
            .into_iter()
            .filter_map(|(axis, period)| Some((axis, period?)))
            .find(|(axis, period)| {
                let other_axis = 1 - *axis;
                match (
                    curve_axis_span(curve, *axis),
                    curve_axis_span(curve, other_axis),
                ) {
                    (Some((min, max)), Some((other_min, other_max))) => {
                        max - min + TOLERANCE >= period * 0.75
                            && other_max - other_min <= TOLERANCE
                    }
                    _ => false,
                }
            })
    }
}

fn curve_axis_span(curve: &[Point2], axis: usize) -> Option<(f64, f64)> {
    curve
        .iter()
        .map(|point| uv_axis(*point, axis))
        .fold(None, |span, value| {
            Some(match span {
                Some((min, max)) => {
                    (f64::min(min, value), f64::max(max, value))
                }
                None => (value, value),
            })
        })
}

fn surface_axis_range(
    surface: &Surface,
    axis: usize,
) -> (Option<f64>, Option<(f64, f64)>) {
    let (urange, vrange) = surface.try_range_tuple();
    match axis {
        0 => (surface.period_u(), urange),
        _ => (surface.period_v(), vrange),
    }
}

fn unwrap_periodic_open_curve(curve: &mut [Point2], axis: usize, period: f64) {
    let mut previous = None;
    curve.iter_mut().for_each(|point| {
        if let Some(value) =
            normalize_axis(uv_axis(*point, axis), previous, Some(period), None)
        {
            set_uv_axis(point, axis, value);
            previous = Some(value);
        }
    });
}

fn shift_curve_axis(curve: &mut [Point2], axis: usize, shift: f64) {
    curve.iter_mut().for_each(|point| {
        set_uv_axis(point, axis, uv_axis(*point, axis) + shift);
    });
}

fn paired_curve_axis_span(
    curve0: &[Point2],
    curve1: &[Point2],
    axis: usize,
    curve1_shift: f64,
) -> Option<f64> {
    curve0
        .iter()
        .map(|point| uv_axis(*point, axis))
        .chain(
            curve1
                .iter()
                .map(|point| uv_axis(*point, axis) + curve1_shift),
        )
        .fold(None, |span, value| {
            Some(match span {
                Some((min, max)) => {
                    (f64::min(min, value), f64::max(max, value))
                }
                None => (value, value),
            })
        })
        .map(|(min, max)| max - min)
}

fn align_periodic_open_pair(
    curve0: &[Point2],
    curve1: &mut [Point2],
    axis: usize,
    period: f64,
) {
    let best_shift = (-4..=4)
        .filter_map(|lap| {
            let shift = lap as f64 * period;
            paired_curve_axis_span(curve0, curve1, axis, shift)
                .map(|span| (span, shift))
        })
        .min_by(|lhs, rhs| lhs.0.total_cmp(&rhs.0))
        .map(|(_, shift)| shift);
    if let Some(shift) = best_shift {
        shift_curve_axis(curve1, axis, shift);
    }
}

fn align_periodic_open_piece_pair(
    curve0: &TrimPiece,
    curve1: &mut TrimPiece,
    axis: usize,
    period: f64,
) {
    let before = curve1.topology_points.clone();
    align_periodic_open_pair(
        &curve0.topology_points,
        &mut curve1.topology_points,
        axis,
        period,
    );
    let shift = before
        .first()
        .zip(curve1.topology_points.first())
        .map(|(before, after)| uv_axis(*after, axis) - uv_axis(*before, axis))
        .unwrap_or(0.0);
    if shift.abs() > TOLERANCE {
        curve1
            .curves
            .iter_mut()
            .for_each(|curve| curve.translate_axis(axis, shift));
    }
}

fn periodic_seam_bounds(
    lower: f64,
    upper: f64,
    period: f64,
    range: Option<(f64, f64)>,
) -> (f64, f64) {
    let origin = range.map(|(min, _)| min).unwrap_or(0.0);
    let mut seam_lower = origin + ((lower - origin) / period).floor() * period;
    while upper > seam_lower + period + TOLERANCE {
        seam_lower += period;
    }
    (seam_lower, seam_lower + period)
}

fn seam_uv(mut template: Point2, axis: usize, value: f64) -> Point2 {
    set_uv_axis(&mut template, axis, value);
    template
}

fn open_periodic_closed_loop(
    mut curve: Vec<Point2>,
    surface: &Surface,
    axis: usize,
    period: f64,
) -> Vec<Point2> {
    curve.pop();
    let jump = curve
        .windows(2)
        .enumerate()
        .map(|(index, points)| {
            (index, uv_axis(points[1], axis) - uv_axis(points[0], axis))
        })
        .filter(|(_, delta)| delta.abs() > period * 0.5)
        .max_by(|lhs, rhs| lhs.1.abs().total_cmp(&rhs.1.abs()));
    if let Some((index, delta)) = jump {
        let lower = f64::min(
            uv_axis(curve[index], axis),
            uv_axis(curve[index + 1], axis),
        );
        let upper = f64::max(
            uv_axis(curve[index], axis),
            uv_axis(curve[index + 1], axis),
        );
        let (_, range) = surface_axis_range(surface, axis);
        let (seam_lower, seam_upper) =
            periodic_seam_bounds(lower, upper, period, range);
        let mut opened = Vec::with_capacity(curve.len() + 2);
        if delta > 0.0 {
            opened.push(seam_uv(curve[index + 1], axis, seam_upper));
            opened.extend_from_slice(&curve[index + 1..]);
            opened.extend_from_slice(&curve[..=index]);
            opened.push(seam_uv(curve[index], axis, seam_lower));
        } else {
            opened.push(seam_uv(curve[index + 1], axis, seam_lower));
            opened.extend_from_slice(&curve[index + 1..]);
            opened.extend_from_slice(&curve[..=index]);
            opened.push(seam_uv(curve[index], axis, seam_upper));
        }
        opened
    } else {
        unwrap_periodic_open_curve(&mut curve, axis, period);
        if let (Some(front), Some(back)) =
            (curve.first().copied(), curve.last().copied())
        {
            let delta = uv_axis(back, axis) - uv_axis(front, axis);
            if delta.abs() + TOLERANCE < period {
                let shift = if delta < 0.0 { -period } else { period };
                let seam = seam_uv(front, axis, uv_axis(front, axis) + shift);
                curve.push(seam);
            }
        }
        curve
    }
}

fn uv_axis(point: Point2, axis: usize) -> f64 {
    match axis {
        0 => point.x,
        _ => point.y,
    }
}

fn set_uv_axis(point: &mut Point2, axis: usize, value: f64) {
    match axis {
        0 => point.x = value,
        _ => point.y = value,
    }
}

fn f64_near(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= TOLERANCE
}

fn uv_near(lhs: Point2, rhs: Point2) -> bool {
    uv_distance(lhs, rhs) <= TOLERANCE
}

fn uv_distance(lhs: Point2, rhs: Point2) -> f64 {
    ((lhs.x - rhs.x).powi(2) + (lhs.y - rhs.y).powi(2)).sqrt()
}

fn bspline_curve_to_trim_curve(
    curve: &BsplineCurve<Vector3>,
) -> Option<NsiBrepTrimCurveData> {
    let n = i32::try_from(curve.control_points().len()).ok()?;
    let order = i32::try_from(curve.degree().checked_add(1)?).ok()?;
    let (min, max) = curve.range_tuple();
    let knot = curve
        .knot_vector()
        .iter()
        .map(|knot| *knot as f32)
        .collect();
    let u = curve
        .control_points()
        .iter()
        .map(|point| point.x as f32)
        .collect();
    let v = curve
        .control_points()
        .iter()
        .map(|point| point.y as f32)
        .collect();
    let w = curve
        .control_points()
        .iter()
        .map(|point| point.z as f32)
        .collect();

    Some(NsiBrepTrimCurveData {
        n,
        order,
        knot,
        min: min as f32,
        max: max as f32,
        u,
        v,
        w,
        edge_use: None,
    })
}

fn curve2d_to_homogeneous_bspline(
    curve: &Curve2D,
) -> Option<BsplineCurve<Vector3>> {
    match curve {
        Curve2D::Line(line) => Some(BsplineCurve::new(
            KnotVector::bezier_knot(1),
            vec![point2_to_homogeneous(line.0), point2_to_homogeneous(line.1)],
        )),
        Curve2D::Polyline(polyline) => polyline_to_bspline(&polyline.0),
        Curve2D::Conic(Conic2D::Ellipse(ellipse)) => {
            Some(ellipse_to_homogeneous_bspline(ellipse))
        }
        Curve2D::Conic(conic) => {
            log::warn!(
                "NSI BRep emitter: skipped non-ellipse conic trim curve to avoid approximating watertight trims"
            );
            let _ = conic;
            None
        }
        Curve2D::BsplineCurve(curve) => Some(BsplineCurve::new(
            curve.knot_vector().clone(),
            curve
                .control_points()
                .iter()
                .copied()
                .map(point2_to_homogeneous)
                .collect(),
        )),
        Curve2D::NurbsCurve(curve) => Some(curve.non_rationalized().clone()),
    }
}

fn ellipse_to_homogeneous_bspline(
    ellipse: &Processor<TrimmedCurve<UnitCircle<Point2>>, Matrix3>,
) -> BsplineCurve<Vector3> {
    let (start, end) = ellipse.range_tuple();
    let span = end - start;
    let segments =
        ((span.abs() / MAX_RATIONAL_QUADRATIC_ARC).ceil() as usize).max(1);
    let knots = rational_quadratic_arc_knots(segments);
    let transform = *ellipse.transform();
    let control_points = (0..segments)
        .flat_map(|segment| {
            let segment_start = start + span * segment as f64 / segments as f64;
            let segment_end =
                start + span * (segment + 1) as f64 / segments as f64;
            let source_start =
                ellipse_source_parameter(ellipse, start, end, segment_start);
            let source_end =
                ellipse_source_parameter(ellipse, start, end, segment_end);
            let mid = (source_start + source_end) / 2.0;
            let weight = ((source_end - source_start) / 2.0).cos();
            let first = transform
                * Vector3::new(source_start.cos(), source_start.sin(), 1.0);
            let middle = transform * Vector3::new(mid.cos(), mid.sin(), weight);
            let last = transform
                * Vector3::new(source_end.cos(), source_end.sin(), 1.0);
            if segment == 0 {
                vec![first, middle, last]
            } else {
                vec![middle, last]
            }
        })
        .collect();

    BsplineCurve::new(KnotVector::from(knots), control_points)
}

fn ellipse_source_parameter(
    ellipse: &Processor<TrimmedCurve<UnitCircle<Point2>>, Matrix3>,
    start: f64,
    end: f64,
    parameter: f64,
) -> f64 {
    if ellipse.orientation() {
        parameter
    } else {
        start + end - parameter
    }
}

fn rational_quadratic_arc_knots(segments: usize) -> Vec<f64> {
    (0..segments + 3)
        .flat_map(|index| {
            if index < 3 {
                vec![0.0]
            } else if index == segments + 2 {
                vec![1.0, 1.0, 1.0]
            } else {
                let knot = (index - 2) as f64 / segments as f64;
                vec![knot, knot]
            }
        })
        .collect()
}

fn polyline_to_bspline(points: &[Point2]) -> Option<BsplineCurve<Vector3>> {
    (points.len() >= 2).then(|| {
        let last = points.len() - 1;
        let knots: Vec<f64> = (0..points.len() + 2)
            .map(|index| {
                if index == 0 {
                    0.0
                } else if index == points.len() + 1 {
                    1.0
                } else {
                    (index - 1) as f64 / last as f64
                }
            })
            .collect();

        BsplineCurve::new(
            KnotVector::from(knots),
            points.iter().copied().map(point2_to_homogeneous).collect(),
        )
    })
}

fn point2_to_homogeneous(point: Point2) -> Vector3 {
    Vector3::new(point.x, point.y, 1.0)
}
