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
    step::load::step_geometry::Curve3D,
    topology::compress::CompressedEdge,
    traits::{BoundedCurve, ParametricCurve, ParametricSurface},
};

use super::brep::{NsiBrepSurfaceData, NsiBrepTrimData};

/// How many points to take along a selector. Enough that the sense of a
/// closed boundary is unambiguous, few enough to be cheap.
const SAMPLES: usize = 16;

/// A boundary, sampled in space along its own direction.
///
/// Comparisons here never assume the two sides run at the same parameter
/// speed: the contract says a shared start and direction do not imply
/// one, and a girdle band's side crosses its circle at a different rate.
pub struct Traversal(Vec<Point3>);

impl Traversal {
    fn start(&self) -> Point3 {
        self.0[0]
    }

    fn end(&self) -> Point3 {
        self.0[self.0.len() - 1]
    }

    /// Whether it comes back to where it started.
    pub fn is_closed(&self, tolerance: f64) -> bool {
        distance(self.start(), self.end()) <= tolerance * tolerance
    }

    /// Twice the area vector the sampled polygon sweeps, which says
    /// which way round a closed boundary runs whatever its speed.
    fn sense(&self) -> Vector3 {
        let cross = |a: Point3, b: Point3| {
            Vector3::new(
                a.y * b.z - a.z * b.y,
                a.z * b.x - a.x * b.z,
                a.x * b.y - a.y * b.x,
            )
        };
        self.0
            .windows(2)
            .map(|pair| cross(pair[0], pair[1]))
            .fold(Vector3::new(0.0, 0.0, 0.0), |sum, area| sum + area)
    }

    /// Whether this runs against `reference`.
    ///
    /// A closed boundary is compared by the sense it sweeps, an open one
    /// by which end it starts at.
    pub fn runs_against(&self, reference: &Self, tolerance: f64) -> bool {
        if reference.is_closed(tolerance) {
            let (theirs, ours) = (reference.sense(), self.sense());
            ours.x * theirs.x + ours.y * theirs.y + ours.z * theirs.z < 0.0
        } else {
            let direct = distance(self.start(), reference.start())
                + distance(self.end(), reference.end());
            let reversed = distance(self.start(), reference.end())
                + distance(self.end(), reference.start());
            reversed < direct
        }
    }

    /// Whether it starts where `reference` does -- the anchor the
    /// contract asks every use of a weld to share.
    pub fn anchors_with(&self, reference: &Self, tolerance: f64) -> bool {
        let tolerance = tolerance * tolerance;
        distance(self.start(), reference.start()) <= tolerance
            || distance(self.end(), reference.start()) <= tolerance
    }
}

fn distance(a: Point3, b: Point3) -> f64 {
    let offset = a - b;
    offset.x * offset.x + offset.y * offset.y + offset.z * offset.z
}

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

    /// The part of trim curve `curve` that `range` selects, in the
    /// curve's own direction.
    pub fn trim(&self, curve: usize, range: [f32; 2]) -> Option<Traversal> {
        let curve = self.curves.get(curve)?;
        let (t0, t1) = curve.range_tuple();
        Some(Traversal(
            along(range)
                .map(|at| {
                    let uv = curve.subs(t0 + at * (t1 - t0));
                    self.surface.subs(uv.x, uv.y)
                })
                .collect(),
        ))
    }

    /// The part of a side of the active domain that `range` selects, in
    /// the side's own direction: increasing `v` for a u-side, increasing
    /// `u` for a v-side.
    pub fn side(
        &self,
        surface: &NsiBrepSurfaceData,
        side: i32,
        range: [f32; 2],
    ) -> Option<Traversal> {
        let (u0, u1) = (surface.umin as f64, surface.umax as f64);
        let (v0, v1) = (surface.vmin as f64, surface.vmax as f64);
        let (from, to) = match side {
            0 => ((u0, v0), (u0, v1)),
            1 => ((u1, v0), (u1, v1)),
            2 => ((u0, v0), (u1, v0)),
            3 => ((u0, v1), (u1, v1)),
            _ => return None,
        };
        Some(Traversal(
            along(range)
                .map(|at| {
                    let u = from.0 + at * (to.0 - from.0);
                    let v = from.1 + at * (to.1 - from.1);
                    self.surface.subs(u, v)
                })
                .collect(),
        ))
    }
}

/// Where along a selector to sample, over the part `range` selects.
fn along(range: [f32; 2]) -> impl Iterator<Item = f64> {
    let (from, to) = (range[0] as f64, range[1] as f64);
    (0..SAMPLES)
        .map(move |at| from + (to - from) * at as f64 / (SAMPLES - 1) as f64)
}

/// The edge itself, in its own direction: the one traversal both faces
/// share, and for a closed edge an anchor at its vertex rather than
/// wherever a face's trim happens to start.
pub fn edge(edge: &CompressedEdge<Curve3D>) -> Traversal {
    let (t0, t1) = edge.curve.range_tuple();
    Traversal(
        along([0.0, 1.0])
            .map(|at| edge.curve.subs(t0 + at * (t1 - t0)))
            .collect(),
    )
}
