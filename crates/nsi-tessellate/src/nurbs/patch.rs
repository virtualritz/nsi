//! One ɴsɪ `nurbs` node, read into `monstertruck` geometry.
//!
//! The names are the ones a [`Scene`](nsi_intermediate::Scene) holds --
//! the naming-convention draft's `u.count`, `u.order`, `u.knot`,
//! `trim-curves.*` -- which is what an exporter writing 3Delight
//! 2.9.210's `nu`, `uorder`, `trimcurves.*` is recorded as. `P` and `Pw`
//! keep their names, being what a shader reads them as; the two rows
//! the draft marks as API changes, `trimcurves.u`/`.v`/`.w` and
//! `trimcurves.inside`, keep theirs too.
//!
//! Control points are stored u-fastest and `Pw` premultiplied by its
//! weight, `(w·x, w·y, w·z, w)`, the layout `monstertruck` uses too.

use monstertruck::{
    geometry::prelude::{
        BsplineCurve, BsplineSurface, KnotVector, NurbsCurve, NurbsSurface,
        Vector3, Vector4,
    },
    traits::{BoundedCurve, Cut},
};
use nsi_intermediate::{Node, OwnedArgument};

/// A surface: `monstertruck`'s rational B-spline, homogeneous points.
pub(super) type Surface = NurbsSurface<Vector4>;

/// A trim curve: a rational B-spline in the surface's parameter space,
/// homogeneous `(w·u, w·v, w)`.
pub(super) type TrimCurve = NurbsCurve<Vector3>;

/// A `nurbs` node, read.
pub(super) struct Patch {
    /// The surface.
    pub surface: Surface,
    /// Trim loops, each its curves in stored order; empty when untrimmed.
    pub loops: Vec<Vec<TrimCurve>>,
    /// The active domain, `((umin, umax), (vmin, vmax))`: the knot range
    /// unless `umin` and its kin narrow it. Its sides are what
    /// `nurbs-side` welds select.
    pub domain: ((f64, f64), (f64, f64)),
}

fn integers<'a>(node: &'a Node, name: &str) -> Option<&'a [i32]> {
    node.attribute(name).and_then(OwnedArgument::as_i32s)
}

fn integer(node: &Node, name: &str) -> Option<usize> {
    integers(node, name)
        .and_then(|values| values.first())
        .and_then(|&value| usize::try_from(value).ok())
}

fn floats<'a>(node: &'a Node, name: &str) -> Option<&'a [f32]> {
    node.attribute(name).and_then(OwnedArgument::as_f32s)
}

/// Why a `nurbs` node could not be read.
pub(super) fn read(node: &Node) -> Result<Patch, String> {
    let nu = integer(node, "u.count").ok_or("missing `u.count`")?;
    let nv = integer(node, "v.count").ok_or("missing `v.count`")?;
    let uorder = integer(node, "u.order").ok_or("missing `u.order`")?;
    let vorder = integer(node, "v.order").ok_or("missing `v.order`")?;
    let uknot = floats(node, "u.knot").ok_or("missing `u.knot`")?;
    let vknot = floats(node, "v.knot").ok_or("missing `v.knot`")?;
    if uknot.len() != nu + uorder || vknot.len() != nv + vorder {
        return Err(format!(
            "knot vectors have {} and {} values, expected {} and {}",
            uknot.len(),
            vknot.len(),
            nu + uorder,
            nv + vorder
        ));
    }

    // `Pw` is premultiplied, as `monstertruck`'s homogeneous points are;
    // `P` is the same with every weight 1.
    let homogeneous: Vec<Vector4> =
        match (floats(node, "Pw"), floats(node, "P")) {
            (Some(pw), _) if pw.len() == 4 * nu * nv => pw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    Vector4::new(
                        c[0] as f64,
                        c[1] as f64,
                        c[2] as f64,
                        c[3] as f64,
                    )
                })
                .collect(),
            (None, Some(p)) if p.len() == 3 * nu * nv => p
                .as_chunks::<3>()
                .0
                .iter()
                .map(|c| {
                    Vector4::new(c[0] as f64, c[1] as f64, c[2] as f64, 1.0)
                })
                .collect(),
            _ => {
                return Err(format!("no `P` or `Pw` with {} points", nu * nv));
            }
        };

    // ɴsɪ stores u fastest; `monstertruck` indexes `[u][v]`.
    let control_points: Vec<Vec<Vector4>> = (0..nu)
        .map(|u| (0..nv).map(|v| homogeneous[v * nu + u]).collect())
        .collect();
    let knots = |values: &[f32]| {
        KnotVector::from(values.iter().map(|&k| k as f64).collect::<Vec<_>>())
    };
    let bspline =
        BsplineSurface::try_new((knots(uknot), knots(vknot)), control_points)
            .map_err(|error| format!("not a valid surface: {error}"))?;
    let surface = NurbsSurface::new(bspline);

    let domain = (
        (uknot[uorder - 1] as f64, uknot[nu] as f64),
        (vknot[vorder - 1] as f64, vknot[nv] as f64),
    );
    let bound = |name: &str, default: f64| {
        floats(node, name)
            .and_then(|values| values.first())
            .map_or(default, |&value| value as f64)
    };
    let active = (
        (bound("u.min", domain.0.0), bound("u.max", domain.0.1)),
        (bound("v.min", domain.1.0), bound("v.max", domain.1.1)),
    );
    Ok(Patch {
        surface,
        loops: read_trims(node, domain)?,
        domain: active,
    })
}

/// A parameter a rounding past `(start, end)` moved outside it, moved
/// back; any other parameter as it is.
///
/// A trim that runs round a closed surface ends on the domain's edge, and
/// an `f32` that should be 2π can be a rounding past it. The mesher reads
/// such a point as lying on the far side of the seam and folds the face
/// over itself.
fn clamp_rounding(parameter: f64, (start, end): (f64, f64)) -> f64 {
    let slack = 1e-5 * (end - start).abs().max(1.0);
    if parameter < start && start - parameter <= slack {
        start
    } else if end < parameter && parameter - end <= slack {
        end
    } else {
        parameter
    }
}

/// The trim loops, from `trimcurves.*`.
fn read_trims(
    node: &Node,
    (u_domain, v_domain): ((f64, f64), (f64, f64)),
) -> Result<Vec<Vec<TrimCurve>>, String> {
    let Some(curves_per_loop) = integers(node, "trim-curves.curve-count")
    else {
        return Ok(Vec::new());
    };
    // 3Delight 2.9.210 renders what lies inside a trim loop, and
    // `trimcurves.inside` 0 asks for the outside instead. The mesher
    // keeps the inside, so the outside would come back inverted; refuse
    // it rather than tessellate the wrong side.
    if integer(node, "trimcurves.inside") == Some(0) {
        return Err("`trimcurves.inside` 0 -- the surface outside its \
                    trim loops -- is not tessellated yet"
            .to_string());
    }
    let points = integers(node, "trim-curves.point-count")
        .ok_or("missing `trim-curves.point-count`")?;
    let orders = integers(node, "trim-curves.order")
        .ok_or("missing `trim-curves.order`")?;
    let knots =
        floats(node, "trim-curves.knot").ok_or("missing `trim-curves.knot`")?;
    let minimum =
        floats(node, "trim-curves.min").ok_or("missing `trim-curves.min`")?;
    let maximum =
        floats(node, "trim-curves.max").ok_or("missing `trim-curves.max`")?;
    let u = floats(node, "trimcurves.u").ok_or("missing `trimcurves.u`")?;
    let v = floats(node, "trimcurves.v").ok_or("missing `trimcurves.v`")?;
    let w = floats(node, "trimcurves.w").ok_or("missing `trimcurves.w`")?;

    let total: usize = curves_per_loop.iter().map(|&n| n.max(0) as usize).sum();
    if [points.len(), orders.len(), minimum.len(), maximum.len()]
        .iter()
        .any(|&length| length != total)
    {
        return Err(format!("trim arrays do not all have {total} curves"));
    }

    let mut knot_start = 0;
    let mut point_start = 0;
    let mut curves = Vec::with_capacity(total);
    for curve in 0..total {
        let count = usize::try_from(points[curve])
            .map_err(|_| "negative `trimcurves.n`")?;
        let order = usize::try_from(orders[curve])
            .map_err(|_| "negative `trimcurves.order`")?;
        let knot_end = knot_start + count + order;
        let point_end = point_start + count;
        if knots.len() < knot_end
            || u.len() < point_end
            || v.len() < point_end
            || w.len() < point_end
        {
            return Err(format!("trim curve {curve} runs past its arrays"));
        }
        let control_points = (point_start..point_end)
            .map(|i| {
                let w = w[i] as f64;
                Vector3::new(
                    clamp_rounding(u[i] as f64 / w, u_domain) * w,
                    clamp_rounding(v[i] as f64 / w, v_domain) * w,
                    w,
                )
            })
            .collect();
        let knot_vector = KnotVector::from(
            knots[knot_start..knot_end]
                .iter()
                .map(|&k| k as f64)
                .collect::<Vec<_>>(),
        );
        let bspline = BsplineCurve::try_new(knot_vector, control_points)
            .map_err(|error| format!("trim curve {curve}: {error}"))?;
        let mut trim = NurbsCurve::new(bspline);
        restrict(&mut trim, minimum[curve] as f64, maximum[curve] as f64);
        curves.push(trim);
        knot_start = knot_end;
        point_start = point_end;
    }

    let mut curves = curves.into_iter();
    Ok(curves_per_loop
        .iter()
        .map(|&count| curves.by_ref().take(count.max(0) as usize).collect())
        .collect())
}

/// Restricts `curve` to `[minimum, maximum]`, when that is narrower than
/// its knot range.
fn restrict(curve: &mut TrimCurve, minimum: f64, maximum: f64) {
    let (start, end) = curve.range_tuple();
    if start < minimum && minimum < end {
        *curve = curve.cut(minimum);
    }
    let (start, end) = curve.range_tuple();
    if start < maximum && maximum < end {
        curve.cut(maximum);
    }
}
