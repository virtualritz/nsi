//! Typed attribute/parameter names -- compile-time witnesses for ɴsɪ
//! `NSIParam_t` identifiers.
//!
//! On the C side, ɴsɪ uses a single `NSIParam_t` struct everywhere. On the
//! Rust side that single C concept splits into two semantic roles, both of
//! which boil down to the *same* underlying type:
//!
//! - [`Attribute<T>`] -- properties **set on a node** (govern how the scene
//!   looks): `P`, `Pw`, `fov`, `transformationmatrix`, `uknot`, …
//! - [`Parameter<T>`] -- optional arguments **to a function** (govern how
//!   the call behaves): `streamformat`, `errorhandler`, `stoppedcallback`, …
//!
//! `Parameter<T>` is just a type alias for `Attribute<T>` -- the conceptual
//! split exists so the constants and docs read more naturally, but the
//! machinery is identical.
//!
//! End-users normally don't write these types out -- they just reference the
//! exported `const`s through the parameter macros:
//!
//! ```text
//! nsi::point_slice!(POSITION, &points)  // node attribute (was: P)
//! nsi::string!(STREAM_FORMAT, "nsi")    // function parameter to NSIBegin
//! ```
//!
//! Renderer- or app-specific entries are added in downstream crates without
//! touching this one -- `Attribute::new` is `const`:
//!
//! ```ignore
//! pub const MY_RENDERER_THING: Attribute<f32> = Attribute::new("custom_thing");
//! ```

use core::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

// ─── Geometric type aliases ─────────────────────────────────────────────────
//
// Scalar attribute data uses the Rust primitives directly (`f32`, `f64`,
// `i32`, `i64`) -- no aliases. Geometric types are aliases of fixed-size
// arrays so that slice length is a multiple of the component count at the
// type level.

/// 2D point with `f32` components -- typically a parametric (u, v) coordinate.
pub type Point2F32 = [f32; 2];
/// 3D point with `f32` components -- Cartesian position.
pub type Point3F32 = [f32; 3];
/// 4D point with `f32` components -- rational/weighted homogeneous (xyzw).
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

// Manual derives so generic bounds don't require T: Clone/Copy/Debug -- T is
// only ever used in PhantomData<fn() -> T> which is already Copy/Send/Sync.
impl<T: ?Sized> Clone for Attribute<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for Attribute<T> {}
impl<T: ?Sized> fmt::Debug for Attribute<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attribute")
            .field("name", &self.name)
            .finish()
    }
}
// Two attributes are the same attribute when they name the same one.
// `T` is a witness of the data shape, not part of the identity, and it
// appears only in `PhantomData`, so none of these need a bound on it.
// Without them an `Attribute` could not go in a `HashSet`, be compared
// in an `assert_eq!`, or key a map -- all of which a consumer building
// a table of attributes it has already set will want.
impl<T: ?Sized> PartialEq for Attribute<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl<T: ?Sized> Eq for Attribute<T> {}
impl<T: ?Sized> Hash for Attribute<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
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

// `Attribute<T>` is `Send + Sync` automatically: it holds a
// `&'static str` and a `PhantomData<fn() -> T>`, and a function
// pointer type is `Send + Sync` whatever `T` is. Two `unsafe impl`s
// used to say so explicitly, with a SAFETY comment that admitted the
// auto-impl already held -- `unsafe` that buys nothing is `unsafe` a
// reader still has to check. The test below asserts it instead.

/// Typed name of an ɴsɪ function-parameter -- alias of [`Attribute<T>`].
///
/// Used for the optional arguments passed to ɴsɪ calls (`NSIBegin`,
/// `NSIRenderControl`, `NSIEvaluate`, …) that govern *how the call behaves*,
/// as opposed to [`Attribute<T>`] which is set on nodes to govern *how the
/// scene looks*. The C side uses a single `NSIParam_t` struct for both, so
/// `Parameter<T>` and `Attribute<T>` are the same Rust type -- the alias
/// exists so consumers and docs can name the role precisely.
pub type Parameter<T> = Attribute<T>;

// ─── Standard ɴsɪ attribute names ───────────────────────────────────────────
//
// Rust constant identifiers are derived from the **new** wire-name convention
// (see the `naming-convention.md` chapter in the ɴsɪ spec). Mapping rule:
// take the new name, replace `-` and `.` with `_`, uppercase. Examples:
//
//   new wire        Rust const
//   --------        ----------
//   field-of-view   FIELD_OF_VIEW
//   u.count         U_COUNT
//   trim-curves.    TRIM_CURVES_*
//   callback.error  CALLBACK_ERROR
//
// The string literals below intentionally still hold the **old** wire names
// (e.g. `"fov"`, `"nu"`, `"errorhandler"`) so the constants work against a
// pre-rename 3DelightNSI. Once the renderer ships the new names, only the
// string literals here need to change -- the public Rust API stays stable.

// Camera --------------------------------------------------------------------

/// `field-of-view` (currently `fov`) -- perspective camera FOV in degrees.
pub const FIELD_OF_VIEW: Attribute<f32> = Attribute::new("fov");

// Screen --------------------------------------------------------------------

/// `resolution` -- pixel resolution of a `screen` node, `[width, height]`.
pub const RESOLUTION: Attribute<[i32]> = Attribute::new("resolution");
/// `oversampling` -- pixel oversampling rate.
pub const OVERSAMPLING: Attribute<i32> = Attribute::new("oversampling");

// Transform/shading -------------------------------------------------------

/// `matrix` (currently `transformationmatrix`) -- 4×4 row-major matrix (`f64`).
pub const MATRIX: Attribute<Matrix4F64> =
    Attribute::new("transformationmatrix");
/// `filename` (currently `shaderfilename`) -- OSL shader filename.
pub const FILENAME: Attribute<&'static str> = Attribute::new("shaderfilename");

// Common geometry attrs -----------------------------------------------------

/// `position` (currently `P`) -- Cartesian control points/vertices.
///
/// Slice of 3-component f32 points; total component count is divisible by 3
/// at the type level.
pub const POSITION: Attribute<[Point3F32]> = Attribute::new("P");
/// `weighted-position` (currently `Pw`) -- rational (weighted homogeneous)
/// control points: xyzw.
pub const WEIGHTED_POSITION: Attribute<[Point4F32]> = Attribute::new("Pw");

// NURBS surface intrinsics --------------------------------------------------

/// `u.count` (currently `nu`) -- control-point count along *u*.
pub const U_COUNT: Attribute<i32> = Attribute::new("nu");
/// `v.count` (currently `nv`) -- control-point count along *v*.
pub const V_COUNT: Attribute<i32> = Attribute::new("nv");
/// `u.order` (currently `uorder`) -- order along *u* (degree + 1, ≥ 2).
pub const U_ORDER: Attribute<i32> = Attribute::new("uorder");
/// `v.order` (currently `vorder`) -- order along *v* (degree + 1, ≥ 2).
pub const V_ORDER: Attribute<i32> = Attribute::new("vorder");
/// `u.knot` (currently `uknot`) -- knot vector along *u*; length = `nu + uorder`.
pub const U_KNOT: Attribute<[f32]> = Attribute::new("uknot");
/// `v.knot` (currently `vknot`) -- knot vector along *v*; length = `nv + vorder`.
pub const V_KNOT: Attribute<[f32]> = Attribute::new("vknot");

// NURBS trim curves ---------------------------------------------------------

/// `trim-curves.loop-count` (currently `trimcurves.nloops`) -- number of
/// trim loops on a NURBS surface.
pub const TRIM_CURVES_LOOP_COUNT: Attribute<i32> =
    Attribute::new("trimcurves.nloops");
/// `trim-curves.curve-count` (currently `trimcurves.ncurves`) -- curves per loop.
pub const TRIM_CURVES_CURVE_COUNT: Attribute<[i32]> =
    Attribute::new("trimcurves.ncurves");
/// `trim-curves.cv-count` (currently `trimcurves.n`) -- control-point count
/// per trim curve.
pub const TRIM_CURVES_CV_COUNT: Attribute<[i32]> =
    Attribute::new("trimcurves.n");
/// `trim-curves.order` -- order per trim curve (degree + 1).
pub const TRIM_CURVES_ORDER: Attribute<[i32]> =
    Attribute::new("trimcurves.order");
/// `trim-curves.knot` -- concatenated knots; total length = Σ(`n[i] + order[i]`).
pub const TRIM_CURVES_KNOT: Attribute<[f32]> =
    Attribute::new("trimcurves.knot");
/// `trim-curves.min` -- parametric start per trim curve.
pub const TRIM_CURVES_MIN: Attribute<[f32]> = Attribute::new("trimcurves.min");
/// `trim-curves.max` -- parametric end per trim curve.
pub const TRIM_CURVES_MAX: Attribute<[f32]> = Attribute::new("trimcurves.max");
/// `trim-curves.u` -- concatenated *u* control values; length = Σ`n[i]`.
pub const TRIM_CURVES_U: Attribute<[f32]> = Attribute::new("trimcurves.u");
/// `trim-curves.v` -- concatenated *v* control values; length = Σ`n[i]`.
pub const TRIM_CURVES_V: Attribute<[f32]> = Attribute::new("trimcurves.v");
/// `trim-curves.w` -- concatenated weights; length = Σ`n[i]`.
pub const TRIM_CURVES_W: Attribute<[f32]> = Attribute::new("trimcurves.w");
/// `trim-curves.sense` -- one per loop. `0` = keep inside, `1` = keep outside (hole).
pub const TRIM_CURVES_SENSE: Attribute<[i32]> =
    Attribute::new("trimcurves.sense");

// Globals/render-control --------------------------------------------------

/// `bucket-order` (currently `bucketorder`) -- bucket traversal pattern
/// (`"horizontal"`, `"spiral"`, …).
pub const BUCKET_ORDER: Attribute<&'static str> = Attribute::new("bucketorder");

// ─── Function-level parameters (Parameter<T>) ───────────────────────────────
//
// These govern how an ɴsɪ *call* behaves rather than how a node looks. The
// underlying type is the same -- `Parameter<T>` is just an alias for
// `Attribute<T>` -- the split is purely for readability at call sites.

/// `stream.format` (currently `streamformat`) -- output stream format for
/// `NSIBegin` (`"nsi"`, `"binarynsi"`, `"autonsi"`).
pub const STREAM_FORMAT: Parameter<&'static str> =
    Parameter::new("streamformat");
/// `stream.filename` (currently `streamfilename`) -- output file path when
/// `NSIBegin` is invoked in stream-to-file mode.
pub const STREAM_FILENAME: Parameter<&'static str> =
    Parameter::new("streamfilename");
/// `stream.path-replacement` (currently `streampathreplace`) -- substitution
/// pairs applied to paths in the output stream.
pub const STREAM_PATH_REPLACEMENT: Parameter<&'static str> =
    Parameter::new("streampathreplace");
/// `callback.error` (currently `errorhandler`) -- error-handler callback
/// registered through `NSIBegin`.
pub const CALLBACK_ERROR: Parameter<&'static str> =
    Parameter::new("errorhandler");
/// `callback.stop` (currently `stoppedcallback`) -- callback fired when an
/// interactive render stops.
pub const CALLBACK_STOP: Parameter<&'static str> =
    Parameter::new("stoppedcallback");

#[cfg(test)]
mod tests {
    /// `Send`, `Sync`, `Eq` and `Hash` hold for a `T` that is none of
    /// them, because `T` is only a witness: it appears in a
    /// `PhantomData<fn() -> T>` and never in a value. This is what the
    /// two `unsafe impl`s used to assert by hand.
    #[test]
    fn the_witness_type_constrains_nothing() {
        use std::collections::HashSet;

        // A raw pointer is neither `Send` nor `Sync` nor `Eq`, so it
        // is the witness this needs without inventing a type whose
        // fields nothing reads.
        type Awkward = *const u8;

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::Attribute<Awkward>>();

        let one = super::Attribute::<Awkward>::new("visibility");
        let same = super::Attribute::<Awkward>::new("visibility");
        let other = super::Attribute::<Awkward>::new("fov");
        assert_eq!(one, same);
        assert_ne!(one, other);

        let mut set = HashSet::new();
        assert!(set.insert(one));
        assert!(!set.insert(same), "the same attribute, hashed alike");
        assert!(set.insert(other));
    }

    use super::*;

    #[test]
    fn names_match_wire_strings() {
        assert_eq!(FIELD_OF_VIEW.name(), "fov");
        assert_eq!(POSITION.name(), "P");
        assert_eq!(WEIGHTED_POSITION.name(), "Pw");
        assert_eq!(U_COUNT.name(), "nu");
        assert_eq!(V_COUNT.name(), "nv");
        assert_eq!(U_ORDER.name(), "uorder");
        assert_eq!(U_KNOT.name(), "uknot");
        assert_eq!(TRIM_CURVES_LOOP_COUNT.name(), "trimcurves.nloops");
        assert_eq!(TRIM_CURVES_SENSE.name(), "trimcurves.sense");
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
        let p = POSITION;
        let p2 = p; // Copy
        assert_eq!(p.name(), p2.name());
    }

    #[cfg(feature = "ustr")]
    #[test]
    fn ustr_round_trip_and_null_terminated() {
        // Same Attribute → same Ustr (caching).
        let u1 = POSITION.ustr();
        let u2 = POSITION.ustr();
        assert_eq!(u1, u2);
        assert_eq!(u1.as_str(), "P");

        // Raw C pointer ends in NUL; readable with CStr.
        let cptr = POSITION.as_c_ptr();
        // SAFETY: Ustr always returns a valid null-terminated C string.
        let cstr = unsafe { core::ffi::CStr::from_ptr(cptr) };
        assert_eq!(cstr.to_str().unwrap(), "P");
    }
}
