//! Sampling a face's emitted boundary in space, to declare weld
//! directions.
//!
//! The [weld contract] has every use of a weld follow one reference
//! traversal once its ranges, segment order and `weld.reverse` are
//! applied, and forbids a renderer from inferring a closed use's
//! direction from its endpoints. The exporter therefore has to say which
//! way each use runs, which means comparing them in space: it is the side
//! that holds the geometry.
//!
//! [weld contract]: https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html

use monstertruck::{
    geometry::prelude::{
        BsplineCurve, BsplineSurface, KnotVector, NurbsCurve, NurbsSurface,
        Point3, Vector3, Vector4,
    },
    traits::{BoundedCurve, ParametricCurve, ParametricSurface},
};

use super::brep::{NsiBrepSurfaceData, NsiBrepTrimData};

/// Where along a selector to sample. The ends alone cannot tell the two
/// senses of a closed boundary apart, and neither can its middle: both
/// traversals run through it. The quarter points can.
const AT: [f64; 4] = [0.0, 0.25, 0.75, 1.0];

/// A face's surface and trim curves, as `monstertruck` geometry.
pub struct FaceGeometry {
    surface: NurbsSurface<Vector4>,
    curves: Vec<NurbsCurve<Vector3>>,
}

impl FaceGeometry {
    /// Reads what was emitted, or `None` when it is not a surface this
    /// can evaluate.
    pub fn of(
        surface: &NsiBrepSurfaceData,
        trims: Option<&NsiBrepTrimData>,
    ) -> Option<Self> {
        let nu = usize::try_from(surface.nu).ok()?;
        let nv = usize::try_from(surface.nv).ok()?;
        let knots = |values: &[f32]| {
            KnotVector::from(
                values.iter().map(|knot| *knot as f64).collect::<Vec<_>>(),
            )
        };
        // ɴsɪ stores control points u-fastest; `monstertruck` indexes
        // `[u][v]`.
        let control_points: Vec<Vec<Vector4>> = (0..nu)
            .map(|u| {
                (0..nv)
                    .map(|v| {
                        let point = surface.pw[v * nu + u];
                        Vector4::new(
                            point[0] as f64,
                            point[1] as f64,
                            point[2] as f64,
                            point[3] as f64,
                        )
                    })
                    .collect()
            })
            .collect();
        let bspline = BsplineSurface::try_new(
            (knots(&surface.uknot), knots(&surface.vknot)),
            control_points,
        )
        .ok()?;

        let mut curves = Vec::new();
        if let Some(trims) = trims {
            let (mut point, mut knot) = (0, 0);
            for (count, order) in trims.n.iter().zip(&trims.order) {
                let count = usize::try_from(*count).ok()?;
                let order = usize::try_from(*order).ok()?;
                let control_points = (point..point + count)
                    .map(|index| {
                        Vector3::new(
                            trims.u[index] as f64,
                            trims.v[index] as f64,
                            trims.w[index] as f64,
                        )
                    })
                    .collect();
                let curve = BsplineCurve::try_new(
                    knots(&trims.knot[knot..knot + count + order]),
                    control_points,
                )
                .ok()?;
                curves.push(NurbsCurve::new(curve));
                point += count;
                knot += count + order;
            }
        }

        Some(Self {
            surface: NurbsSurface::new(bspline),
            curves,
        })
    }

    /// Four points along trim curve `curve`, over the part `range`
    /// selects, in the curve's own direction.
    pub fn trim_samples(
        &self,
        curve: usize,
        range: [f32; 2],
    ) -> Option<[Point3; 4]> {
        let curve = self.curves.get(curve)?;
        let (t0, t1) = curve.range_tuple();
        Some(AT.map(|at| {
            let along =
                range[0] as f64 + at * (range[1] as f64 - range[0] as f64);
            let uv = curve.subs(t0 + along * (t1 - t0));
            self.surface.subs(uv.x, uv.y)
        }))
    }

    /// Four points along a side of the active domain, over the part
    /// `range` selects, in the side's own direction: increasing `v` for a
    /// u-side, increasing `u` for a v-side.
    pub fn side_samples(
        &self,
        surface: &NsiBrepSurfaceData,
        side: i32,
        range: [f32; 2],
    ) -> Option<[Point3; 4]> {
        let (u0, u1) = (surface.umin as f64, surface.umax as f64);
        let (v0, v1) = (surface.vmin as f64, surface.vmax as f64);
        let (from, to) = match side {
            0 => ((u0, v0), (u0, v1)),
            1 => ((u1, v0), (u1, v1)),
            2 => ((u0, v0), (u1, v0)),
            3 => ((u0, v1), (u1, v1)),
            _ => return None,
        };
        Some(AT.map(|at| {
            let along =
                range[0] as f64 + at * (range[1] as f64 - range[0] as f64);
            let u = from.0 + along * (to.0 - from.0);
            let v = from.1 + along * (to.1 - from.1);
            self.surface.subs(u, v)
        }))
    }
}

/// Whether `samples` run against `reference`: the same four points, the
/// other way round, fit better than in order.
pub fn runs_against(reference: &[Point3; 4], samples: &[Point3; 4]) -> bool {
    let distance = |a: Point3, b: Point3| {
        let offset = a - b;
        offset.x * offset.x + offset.y * offset.y + offset.z * offset.z
    };
    let direct: f64 =
        (0..4).map(|at| distance(samples[at], reference[at])).sum();
    let reversed: f64 = (0..4)
        .map(|at| distance(samples[at], reference[3 - at]))
        .sum();
    reversed < direct
}
