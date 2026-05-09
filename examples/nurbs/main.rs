//! Build a NURBS surface with a trim-curve hole and render it.
//!
//! This file doubles as the API reference for the new `nurbs` node type
//! introduced in 3DelightNSI 2.9.199. Every attribute the renderer
//! recognises is set below, with the layout rules called out inline.
//!
//! ## The `nurbs` node — required attributes
//!
//! | Attribute | Type        | Meaning                                     |
//! |-----------|-------------|---------------------------------------------|
//! | `nu`      | i32         | control-point count along u                 |
//! | `nv`      | i32         | control-point count along v                 |
//! | `uorder`  | i32 (≥ 2)   | order along u (degree + 1)                  |
//! | `vorder`  | i32 (≥ 2)   | order along v (degree + 1)                  |
//! | `uknot`   | f32\[\]     | knot vector along u, len = `nu + uorder`    |
//! | `vknot`   | f32\[\]     | knot vector along v, len = `nv + vorder`    |
//! | `P`       | point\[\]   | `nu * nv` control points (xyz)              |
//! | `Pw`      | f32\[4\]\[\]| **alternative** to `P`: rational xyzw      |
//!
//! Provide either `P` or `Pw`; the latter enables rational NURBS.
//!
//! ## Trim curves — `trimcurves.*` (all optional, but provide them as
//! a set when used)
//!
//! Curves live in surface (u, v) parameter space and are themselves
//! NURBS curves. They are organised into *loops*; each loop is a
//! sequence of curves whose endpoints meet, forming a closed boundary.
//!
//! | Attribute              | Type      | Length                       |
//! |------------------------|-----------|------------------------------|
//! | `trimcurves.nloops`    | i32       | 1 (single value)             |
//! | `trimcurves.ncurves`   | i32\[\]   | one per loop                 |
//! | `trimcurves.n`         | i32\[\]   | one per curve (CV count)     |
//! | `trimcurves.order`     | i32\[\]   | one per curve                |
//! | `trimcurves.knot`      | f32\[\]   | concatenated, sum of `n[i] + order[i]` |
//! | `trimcurves.min`       | f32\[\]   | one per curve, parametric start |
//! | `trimcurves.max`       | f32\[\]   | one per curve, parametric end   |
//! | `trimcurves.u`         | f32\[\]   | concatenated, sum of `n[i]`  |
//! | `trimcurves.v`         | f32\[\]   | concatenated, sum of `n[i]`  |
//! | `trimcurves.w`         | f32\[\]   | concatenated, sum of `n[i]`  |
//! | `trimcurves.sense`     | i32\[\]   | one per loop (0 = keep inside, 1 = keep outside) |
//!
//! `u`, `v`, `w` together form rational 2D control points: `(u/w, v/w)`
//! is the parameter-space position. For non-rational trim curves,
//! pass `w = 1.0` for every CV.
//!
//! Run with: `cargo run --example nurbs`
use nsi_ffi_wrap as nsi;

fn main() {
    let ctx = nsi::Context::new(None).unwrap();

    // ─── camera + screen ─────────────────────────────────────────────
    ctx.create("cam_xform", nsi::TRANSFORM, None);
    ctx.connect("cam_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "cam_xform",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            // Pull the camera back along +z and tilt down slightly.
            &[
                1.0, 0.0, 0.0, 0.0, //
                0.0, 0.94, -0.34, 0.0, //
                0.0, 0.34, 0.94, 0.0, //
                0.0, 1.5, 4.0, 1.0,
            ]
        )],
    );

    ctx.create("cam", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("cam", None, "cam_xform", "objects", None);
    ctx.set_attribute("cam", &[nsi::f32!("fov", 35.0)]);

    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "cam", "screens", None);
    ctx.set_attribute(
        "screen",
        &[
            nsi::i32_slice!("resolution", &[512, 512]).array_len(2),
            nsi::i32!("oversampling", 32),
        ],
    );

    ctx.create("layer", nsi::OUTPUT_LAYER, None);
    ctx.connect("layer", None, "screen", "outputlayers", None);
    ctx.set_attribute(
        "layer",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "uint8"),
            nsi::string!("colorprofile", "srgb"),
        ],
    );

    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "layer", "outputdrivers", None);
    ctx.set_attribute("driver", &[nsi::string!("drivername", "idisplay")]);

    // ─── environment dome ────────────────────────────────────────────
    ctx.create("env_xform", nsi::TRANSFORM, None);
    ctx.connect("env_xform", None, nsi::ROOT, "objects", None);
    ctx.create("env", nsi::ENVIRONMENT, None);
    ctx.connect("env", None, "env_xform", "objects", None);
    ctx.create("env_attr", nsi::ATTRIBUTES, None);
    ctx.connect("env_attr", None, "env", "geometryattributes", None);
    ctx.create("env_shader", nsi::SHADER, None);
    ctx.connect("env_shader", None, "env_attr", "surfaceshader", None);
    ctx.set_attribute(
        "env_shader",
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/environmentLight"),
            nsi::f32!("intensity", 1.0),
        ],
    );

    // ─── the NURBS patch ─────────────────────────────────────────────
    // A bicubic patch (uorder = vorder = 4) over a 4×4 grid of control
    // points sitting at y = 0, ranging over x,z ∈ [-1, 1]. With orders
    // 4 and 4 control points per direction this becomes a single Bézier
    // patch — clamped open knot vectors give us that.
    const NU: i32 = 4;
    const NV: i32 = 4;
    const UORDER: i32 = 4;
    const VORDER: i32 = 4;

    // Knot vectors: clamped open, length = N + ORDER. For a 4×4
    // bicubic the canonical form is [0,0,0,0, 1,1,1,1].
    let uknot: [f32; 8] = [0., 0., 0., 0., 1., 1., 1., 1.];
    let vknot: [f32; 8] = [0., 0., 0., 0., 1., 1., 1., 1.];

    // 4×4 control points laid out row-major: P[i*nu + j] is column j of
    // row i. We add some bumps on the diagonal to make the trim hole
    // actually visible.
    #[rustfmt::skip]
    let p: [[f32; 3]; 16] = [
        [-1., 0.0, -1.], [-0.33, 0.0, -1.], [0.33, 0.0, -1.], [1., 0.0, -1.],
        [-1., 0.0, -0.33], [-0.33, 0.6, -0.33], [0.33, 0.6, -0.33], [1., 0.0, -0.33],
        [-1., 0.0,  0.33], [-0.33, 0.6,  0.33], [0.33, 0.6,  0.33], [1., 0.0,  0.33],
        [-1., 0.0,  1.], [-0.33, 0.0,  1.], [0.33, 0.0,  1.], [1., 0.0,  1.],
    ];

    // ── trim curve: a closed square hole in (u,v) parameter space ──
    // One loop made of one closed degree-1 (linear) NURBS curve with
    // five CVs (last == first). The curve traces a square inside
    // (u, v) ∈ [0.35, 0.65]², leaving a hole when rendered.
    const TRIM_N: i32 = 5;
    const TRIM_ORDER: i32 = 2; // linear (degree + 1 = 2)
    let trim_u: [f32; 5] = [0.35, 0.65, 0.65, 0.35, 0.35];
    let trim_v: [f32; 5] = [0.35, 0.35, 0.65, 0.65, 0.35];
    let trim_w: [f32; 5] = [1.0, 1.0, 1.0, 1.0, 1.0];

    // Knots for a length-5 linear closed polyline: clamped at both
    // ends, uniform interior. Length = TRIM_N + TRIM_ORDER = 7.
    let trim_knot: [f32; 7] = [0., 0., 0.25, 0.5, 0.75, 1., 1.];

    ctx.create("patch", nsi::NURBS, None);
    ctx.connect("patch", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "patch",
        &[
            // Surface intrinsics.
            nsi::i32!("nu", NU),
            nsi::i32!("nv", NV),
            nsi::i32!("uorder", UORDER),
            nsi::i32!("vorder", VORDER),
            nsi::f32_slice!("uknot", &uknot).array_len(uknot.len() as _),
            nsi::f32_slice!("vknot", &vknot).array_len(vknot.len() as _),
            nsi::point_slice!("P", &p),
            // Trim.
            nsi::i32!("trimcurves.nloops", 1),
            nsi::i32_slice!("trimcurves.ncurves", &[1]).array_len(1),
            nsi::i32_slice!("trimcurves.n", &[TRIM_N]).array_len(1),
            nsi::i32_slice!("trimcurves.order", &[TRIM_ORDER]).array_len(1),
            nsi::f32_slice!("trimcurves.knot", &trim_knot)
                .array_len(trim_knot.len() as _),
            nsi::f32_slice!("trimcurves.min", &[0.0_f32]).array_len(1),
            nsi::f32_slice!("trimcurves.max", &[1.0_f32]).array_len(1),
            nsi::f32_slice!("trimcurves.u", &trim_u)
                .array_len(trim_u.len() as _),
            nsi::f32_slice!("trimcurves.v", &trim_v)
                .array_len(trim_v.len() as _),
            nsi::f32_slice!("trimcurves.w", &trim_w)
                .array_len(trim_w.len() as _),
            // 0 = keep inside the loop, 1 = keep outside (i.e. the loop
            // is a hole). For a hole we want to *remove* the inside.
            nsi::i32_slice!("trimcurves.sense", &[1]).array_len(1),
        ],
    );

    // Surface shader.
    ctx.create("surf_attr", nsi::ATTRIBUTES, None);
    ctx.connect("surf_attr", None, "patch", "geometryattributes", None);
    ctx.create("surf_shader", nsi::SHADER, None);
    ctx.connect("surf_shader", None, "surf_attr", "surfaceshader", None);
    ctx.set_attribute(
        "surf_shader",
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
            nsi::color!("i_color", &[0.7, 0.55, 0.4]),
            nsi::f32!("roughness", 0.4),
        ],
    );

    // ─── render ──────────────────────────────────────────────────────
    ctx.set_attribute(nsi::GLOBAL, &[nsi::string!("bucketorder", "spiral")]);
    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
}
