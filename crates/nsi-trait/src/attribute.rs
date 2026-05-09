//! Typed attribute / parameter names — compile-time witnesses for ɴsɪ
//! `NSIParam_t` identifiers.
//!
//! On the C side, ɴsɪ uses a single `NSIParam_t` struct everywhere. On the
//! Rust side that single C concept splits into two semantic roles, both of
//! which boil down to the *same* underlying type:
//!
//! - [`Attribute<T>`] — properties **set on a node** (govern how the scene
//!   looks): `P`, `Pw`, `fov`, `transformationmatrix`, `uknot`, …
//! - [`Parameter<T>`] — optional arguments **to a function** (govern how
//!   the call behaves): `streamformat`, `errorhandler`, `stoppedcallback`, …
//!
//! `Parameter<T>` is just a type alias for `Attribute<T>` — the conceptual
//! split exists so the constants and docs read more naturally, but the
//! machinery is identical.
//!
//! End-users normally don't write these types out — they just reference the
//! exported `const`s through the parameter macros:
//!
//! ```text
//! nsi::point_slice!(P, &points)     // node attribute
//! nsi::string!(STREAM_FORMAT, "nsi") // function parameter to NSIBegin
//! ```
//!
//! Renderer- or app-specific entries are added in downstream crates without
//! touching this one — `Attribute::new` is `const`:
//!
//! ```ignore
//! pub const MY_RENDERER_THING: Attribute<f32> = Attribute::new("custom_thing");
//! ```

use core::marker::PhantomData;

// ─── Geometric type aliases ─────────────────────────────────────────────────
//
// Scalar attribute data uses the Rust primitives directly (`f32`, `f64`,
// `i32`, `i64`) -- no aliases. Geometric types are aliases of fixed-size
// arrays so that slice length is a multiple of the component count at the
// type level.

/// 2D point with `f32` components — typically a parametric (u, v) coordinate.
pub type Point2F32 = [f32; 2];
/// 3D point with `f32` components — Cartesian position.
pub type Point3F32 = [f32; 3];
/// 4D point with `f32` components — rational/weighted homogeneous (xyzw).
pub type Point4F32 = [f32; 4];

/// 2D vector with `f32` components.
pub type Vector2F32 = [f32; 2];
/// 3D vector with `f32` components.
pub type Vector3F32 = [f32; 3];

/// 3D normal with `f32` components.
pub type Normal3F32 = [f32; 3];

/// RGB color with `f32` components.
pub type Color3F32 = [f32; 3];
/// RGBA color with `f32` components.
pub type Color4F32 = [f32; 4];

/// 3×3 matrix with `f32` components.
pub type Matrix3F32 = [[f32; 3]; 3];
/// 4×4 matrix with `f32` components.
pub type Matrix4F32 = [[f32; 4]; 4];
/// 4×4 matrix with `f64` components.
pub type Matrix4F64 = [[f64; 4]; 4];

// ─── Attribute<T> ───────────────────────────────────────────────────────────

/// Typed name of an ɴsɪ attribute.
///
/// `T` is the data shape the attribute accepts. Slice-typed `T = [U]` means
/// the attribute carries an array of `U`.
pub struct Attribute<T: ?Sized> {
    name: &'static str,
    _t: PhantomData<fn() -> T>,
}

// Manual derives so generic bounds don't require T: Clone/Copy/Debug — T is
// only ever used in PhantomData<fn() -> T> which is already Copy/Send/Sync.
impl<T: ?Sized> Clone for Attribute<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for Attribute<T> {}
impl<T: ?Sized> core::fmt::Debug for Attribute<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Attribute")
            .field("name", &self.name)
            .finish()
    }
}

impl<T: ?Sized> Attribute<T> {
    /// Create a new typed attribute name. `const`-callable so consumers can
    /// declare attribute constants in their own crates.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _t: PhantomData,
        }
    }

    /// The wire-side string identifier this attribute uses.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Interned, null-terminated identifier suitable for C-FFI use. The
    /// underlying [`Ustr`](ustr::Ustr) caches the interning so repeated calls
    /// for the same name reuse the same allocation -- there is no per-call
    /// `CString` dance.
    ///
    /// Available with the `ustr` feature.
    #[cfg(feature = "ustr")]
    #[inline]
    pub fn ustr(&self) -> ustr::Ustr {
        ustr::Ustr::from(self.name)
    }

    /// Raw C-FFI pointer to the null-terminated identifier. The lifetime is
    /// effectively `'static` because [`ustr::Ustr`] never frees its strings.
    /// Pass this directly to C functions expecting a `const char*`.
    ///
    /// Available with the `ustr` feature.
    #[cfg(feature = "ustr")]
    #[inline]
    pub fn as_c_ptr(&self) -> *const core::ffi::c_char {
        self.ustr().as_char_ptr()
    }
}

// SAFETY: Attribute<T> contains only a `&'static str` and PhantomData.
// PhantomData<fn() -> T> is variant-correct (covariant in T) and Send + Sync
// regardless of T, so the auto-derive would also be Send + Sync — but we make
// it explicit so users don't worry about T's bounds.
unsafe impl<T: ?Sized> Send for Attribute<T> {}
unsafe impl<T: ?Sized> Sync for Attribute<T> {}

/// Typed name of an ɴsɪ function-parameter — alias of [`Attribute<T>`].
///
/// Used for the optional arguments passed to ɴsɪ calls (`NSIBegin`,
/// `NSIRenderControl`, `NSIEvaluate`, …) that govern *how the call behaves*,
/// as opposed to [`Attribute<T>`] which is set on nodes to govern *how the
/// scene looks*. The C side uses a single `NSIParam_t` struct for both, so
/// `Parameter<T>` and `Attribute<T>` are the same Rust type — the alias
/// exists so consumers and docs can name the role precisely.
pub type Parameter<T> = Attribute<T>;

// ─── Standard ɴsɪ attribute names ───────────────────────────────────────────
//
// Naming convention on the Rust side:
//   * Counts use `Count` postfix:    `nu` → `COUNT_U`, `nv` → `COUNT_V`
//   * Orders use `Order` postfix:    `uorder` → `ORDER_U`
//   * Knots use `Knot`:              `uknot` → `KNOT_U`
//   * Trim-curve attrs are nested:   `trimcurves.nloops` → `TRIM_CURVE_LOOP_COUNT`

// Camera --------------------------------------------------------------------

/// `fov` — perspective camera field of view (degrees).
pub const FOV: Attribute<f32> = Attribute::new("fov");

// Screen --------------------------------------------------------------------

/// `resolution` — pixel resolution of a `screen` node, `[width, height]`.
pub const RESOLUTION: Attribute<[i32]> = Attribute::new("resolution");
/// `oversampling` — pixel oversampling rate.
pub const OVERSAMPLING: Attribute<i32> = Attribute::new("oversampling");

// Transform / shading -------------------------------------------------------

/// `transformationmatrix` — 4×4 row-major matrix (`f64`).
pub const TRANSFORMATION_MATRIX: Attribute<Matrix4F64> =
    Attribute::new("transformationmatrix");
/// `shaderfilename` — OSL shader filename.
pub const SHADER_FILENAME: Attribute<&'static str> =
    Attribute::new("shaderfilename");

// Common geometry attrs -----------------------------------------------------

/// `P` — Cartesian control points / vertices.
///
/// Slice of 3-component f32 points; total component count is divisible by 3
/// at the type level.
pub const P: Attribute<[Point3F32]> = Attribute::new("P");
/// `Pw` — rational (weighted homogeneous) control points: xyzw.
pub const PW: Attribute<[Point4F32]> = Attribute::new("Pw");

// NURBS surface intrinsics --------------------------------------------------

/// `nu` — control-point count along *u*.
pub const COUNT_U: Attribute<i32> = Attribute::new("nu");
/// `nv` — control-point count along *v*.
pub const COUNT_V: Attribute<i32> = Attribute::new("nv");
/// `uorder` — order along *u* (degree + 1, ≥ 2).
pub const ORDER_U: Attribute<i32> = Attribute::new("uorder");
/// `vorder` — order along *v* (degree + 1, ≥ 2).
pub const ORDER_V: Attribute<i32> = Attribute::new("vorder");
/// `uknot` — knot vector along *u*; length = `nu + uorder`.
pub const KNOT_U: Attribute<[f32]> = Attribute::new("uknot");
/// `vknot` — knot vector along *v*; length = `nv + vorder`.
pub const KNOT_V: Attribute<[f32]> = Attribute::new("vknot");

// NURBS trim curves ---------------------------------------------------------

/// `trimcurves.nloops` — number of trim loops on a NURBS surface.
pub const TRIM_CURVE_LOOP_COUNT: Attribute<i32> =
    Attribute::new("trimcurves.nloops");
/// `trimcurves.ncurves` — curves per loop.
pub const TRIM_CURVE_COUNT: Attribute<[i32]> =
    Attribute::new("trimcurves.ncurves");
/// `trimcurves.n` — control-point count per trim curve.
pub const TRIM_CURVE_CV_COUNT: Attribute<[i32]> =
    Attribute::new("trimcurves.n");
/// `trimcurves.order` — order per trim curve (degree + 1).
pub const TRIM_CURVE_ORDER: Attribute<[i32]> =
    Attribute::new("trimcurves.order");
/// `trimcurves.knot` — concatenated knots; total length = Σ(`n[i] + order[i]`).
pub const TRIM_CURVE_KNOT: Attribute<[f32]> = Attribute::new("trimcurves.knot");
/// `trimcurves.min` — parametric start per trim curve.
pub const TRIM_CURVE_MIN: Attribute<[f32]> = Attribute::new("trimcurves.min");
/// `trimcurves.max` — parametric end per trim curve.
pub const TRIM_CURVE_MAX: Attribute<[f32]> = Attribute::new("trimcurves.max");
/// `trimcurves.u` — concatenated *u* control values; length = Σ`n[i]`.
pub const TRIM_CURVE_U: Attribute<[f32]> = Attribute::new("trimcurves.u");
/// `trimcurves.v` — concatenated *v* control values; length = Σ`n[i]`.
pub const TRIM_CURVE_V: Attribute<[f32]> = Attribute::new("trimcurves.v");
/// `trimcurves.w` — concatenated weights; length = Σ`n[i]`.
pub const TRIM_CURVE_W: Attribute<[f32]> = Attribute::new("trimcurves.w");
/// `trimcurves.sense` — one per loop. `0` = keep inside, `1` = keep outside (hole).
pub const TRIM_CURVE_SENSE: Attribute<[i32]> =
    Attribute::new("trimcurves.sense");

// Globals / render-control --------------------------------------------------

/// `bucketorder` — bucket traversal pattern (`"horizontal"`, `"spiral"`, …).
pub const BUCKET_ORDER: Attribute<&'static str> = Attribute::new("bucketorder");

// ─── Function-level parameters (Parameter<T>) ───────────────────────────────
//
// These govern how an ɴsɪ *call* behaves rather than how a node looks. The
// underlying type is the same -- `Parameter<T>` is just an alias for
// `Attribute<T>` -- the split is purely for readability at call sites.

/// `streamformat` — output stream format for `NSIBegin` (`"nsi"`, `"binarynsi"`,
/// `"autonsi"`).
pub const STREAM_FORMAT: Parameter<&'static str> =
    Parameter::new("streamformat");
/// `streamfilename` — output file path when `NSIBegin` is invoked in
/// stream-to-file mode.
pub const STREAM_FILE_NAME: Parameter<&'static str> =
    Parameter::new("streamfilename");
/// `streampathreplace` — substitution pairs applied to paths in the output
/// stream.
pub const STREAM_PATH_REPLACE: Parameter<&'static str> =
    Parameter::new("streampathreplace");
/// `errorhandler` — error-handler callback registered through `NSIBegin`.
pub const ERROR_HANDLER: Parameter<&'static str> =
    Parameter::new("errorhandler");
/// `stoppedcallback` — callback fired when an interactive render stops.
pub const STOPPED_CALLBACK: Parameter<&'static str> =
    Parameter::new("stoppedcallback");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_wire_strings() {
        assert_eq!(FOV.name(), "fov");
        assert_eq!(P.name(), "P");
        assert_eq!(PW.name(), "Pw");
        assert_eq!(COUNT_U.name(), "nu");
        assert_eq!(COUNT_V.name(), "nv");
        assert_eq!(ORDER_U.name(), "uorder");
        assert_eq!(KNOT_U.name(), "uknot");
        assert_eq!(TRIM_CURVE_LOOP_COUNT.name(), "trimcurves.nloops");
        assert_eq!(TRIM_CURVE_SENSE.name(), "trimcurves.sense");
    }

    #[test]
    fn attribute_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Attribute<f32>>();
        assert_send_sync::<Attribute<[Point3F32]>>();
        assert_send_sync::<Attribute<&'static str>>();
    }

    #[test]
    fn attribute_is_copy() {
        let p = P;
        let p2 = p; // Copy
        assert_eq!(p.name(), p2.name());
    }

    #[cfg(feature = "ustr")]
    #[test]
    fn ustr_round_trip_and_null_terminated() {
        // Same Attribute → same Ustr (caching).
        let u1 = P.ustr();
        let u2 = P.ustr();
        assert_eq!(u1, u2);
        assert_eq!(u1.as_str(), "P");

        // Raw C pointer ends in NUL; readable with CStr.
        let cptr = P.as_c_ptr();
        // SAFETY: Ustr always returns a valid null-terminated C string.
        let cstr = unsafe { core::ffi::CStr::from_ptr(cptr) };
        assert_eq!(cstr.to_str().unwrap(), "P");
    }
}
