//! Owned mirrors of borrowed ɴsɪ arguments.
//!
//! The recorder outlives the calls that feed it, so it cannot hold a
//! borrowed `Arg`. [`OwnedArg`] copies the payload out.
//!
//! This mirrors the ɴsɪ C API's own contract: every argument except a
//! `NSIType` pointer is copied during the call, so a caller may free its
//! data the moment the call returns. Copying here is therefore not an
//! extra cost the recorder introduces — it is what a live renderer would
//! have done anyway.
//!
//! [`OwnedData::Reference`] is the exception, and holds a raw pointer
//! rather than a copy, because that is what ɴsɪ passes through.

use core::ffi::{CStr, c_char, c_void};
use nsi_ffi_wrap::Arg;
use nsi_trait::{ParamValue, Type};

/// A raw host address recorded from an ɴsɪ `Reference` argument.
///
/// # Safety
///
/// `Send` and `Sync` are asserted on two grounds, both structural
/// rather than hopeful:
///
/// 1. **The recorder never dereferences it.** A `HostPtr` is stored on
///    the way in and handed back on the way out, nothing else. No data
///    race is possible through a pointer that is never read.
/// 2. **The pointee outlives everything.** The recorder's `Nsi` impl
///    declares `Arg<'call, 'static>`, so the only `Reference` it can be
///    handed is one whose data is `'static`. That is the same rule
///    `nsi-ffi-wrap` applies -- `Reference`, `Callback` and
///    `ReferenceSlice` are `Send`/`Sync` at `'static` and nowhere else.
///
/// The assertion lives on this newtype rather than on `Recorder` so it
/// covers exactly the field that needs it. A blanket
/// `unsafe impl Send for Recorder` would silently keep covering any
/// non-`Send` field added later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct HostPtr(pub *const c_void);

// SAFETY: see the type's documentation.
unsafe impl Send for HostPtr {}
// SAFETY: see the type's documentation.
unsafe impl Sync for HostPtr {}

/// An ɴsɪ argument's payload, owned.
///
/// Variants are storage representations, not ɴsɪ types: colour, point,
/// vector, normal and 4x4 `f32` matrices all live in [`OwnedData::F32`]
/// and are told apart by [`OwnedArg::type_tag`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum OwnedData {
    /// `f32` scalars, flattened. Also holds colour, point, vector,
    /// normal and 4x4 `f32` matrices; [`OwnedArg::type_tag`] tells them
    /// apart.
    F32(Vec<f32>),
    /// `f64` scalars, flattened. Also holds 4x4 `f64` matrices.
    F64(Vec<f64>),
    /// 32-bit integers.
    I32(Vec<i32>),
    /// 64-bit integers.
    I64(Vec<i64>),
    /// Strings, copied out of their C representation.
    ///
    /// **Bytes, not `String`.** An ɴsɪ string is whatever the C API was
    /// handed. 3Delight writes a byte at or above `0x7f` raw and reads
    /// it back unchanged, and a file name on Linux is not required to be
    /// UTF-8 -- so an `imagefilename` naming `café.exr` in Latin-1 is a
    /// value this crate must carry, not one it may repair. Storing
    /// `String` here replaced the byte with U+FFFD at *recording* time,
    /// which no amount of care at replay could undo.
    ///
    /// Use `String::from_utf8_lossy` where text is wanted.
    ///
    /// # A path value may need expanding
    ///
    /// ɴsɪ streams carry `${VAR}` references so a scene can move between
    /// machines, and 3Delight expands them **when it uses the value**,
    /// not when it reads the stream. A backend that opens a path from
    /// here without expanding opens the wrong file -- and only on the
    /// machines where the variable mattered, which is when it is hardest
    /// to diagnose.
    ///
    /// The rules, measured rather than inferred, because each one
    /// changes what a backend must implement:
    ///
    /// - **`${NAME}` only.** A bare `$NAME` is left alone: a probe
    ///   naming `$VAR/x.exr` created a literal `$VAR` directory.
    /// - **Path-valued attributes only.** `imagefilename`,
    ///   `shaderfilename` and `Evaluate`'s `filename` expand, and nest
    ///   (`${A}/${B}`). `drivername` does **not** -- it fails with
    ///   `E6024 cannot find display driver '${...}'` -- nor does a node
    ///   handle (`E6087 unknown node handle`), nor an attribute *name*,
    ///   which is silently ignored.
    /// - **Any variable, not just `NSI_PATH_`.** `${HOME}` expands. That
    ///   prefix governs which variables 3Delight *writes* as references
    ///   under `streampathreplacement`; on read it means nothing.
    /// - **An unset variable stays literal**, rather than expanding to
    ///   empty or erroring: a probe created a literal `${MISSING}`
    ///   directory.
    ///
    /// One trap worth naming: `${...}` in an output layer's
    /// `variablename` **segfaults** 3Delight 2.9.207. Do not pass one
    /// through to a renderer expecting a diagnostic.
    ///
    /// See `specs/004` research Q1.
    String(Vec<Vec<u8>>),
    /// Raw host pointers. ɴsɪ calls this `Reference` (`Pointer` in the C
    /// API); it is not an object link and is never forwarded to a
    /// renderer as one. Stored so output-driver callbacks survive a
    /// replay. The recorder never dereferences these.
    Reference(Vec<HostPtr>),
}

/// A recorded ɴsɪ argument.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OwnedArg {
    /// The attribute name this argument sets.
    pub name: String,
    /// The ɴsɪ type, which is what tells one [`OwnedData`] layout from
    /// another sharing the same storage.
    pub type_tag: Type,
    /// ɴsɪ's `array_len`. The C `count` field is `len / array_length`.
    pub array_length: usize,
    /// ɴsɪ's argument flags: `per_vertex`, `per_face` and the like.
    /// Replayed as the letter prefix 3Delight writes -- `"v point"`,
    /// `"f float"`, `"vl float"`; see `contracts/stream.md`.
    pub flags: i32,
    /// The payload, copied unless it is a pointer.
    pub data: OwnedData,
}

impl OwnedArg {
    /// The payload as `f32` scalars, or `None` for another layout.
    ///
    /// Colour, point, vector, normal and an `f32` matrix all share this
    /// storage flattened; [`OwnedArg::type_tag`] tells them apart.
    pub fn as_f32s(&self) -> Option<&[f32]> {
        match &self.data {
            OwnedData::F32(values) => Some(values),
            _ => None,
        }
    }

    /// The payload as `f64` scalars, or `None` for another layout.
    pub fn as_f64s(&self) -> Option<&[f64]> {
        match &self.data {
            OwnedData::F64(values) => Some(values),
            _ => None,
        }
    }

    /// The payload as 32-bit integers, or `None` for another layout.
    pub fn as_i32s(&self) -> Option<&[i32]> {
        match &self.data {
            OwnedData::I32(values) => Some(values),
            _ => None,
        }
    }

    /// The payload as 64-bit integers, or `None` for another layout.
    pub fn as_i64s(&self) -> Option<&[i64]> {
        match &self.data {
            OwnedData::I64(values) => Some(values),
            _ => None,
        }
    }

    /// The payload as strings, or `None` for another layout.
    ///
    /// Bytes, not `str`: see [`OwnedData::String`]. A path may need
    /// `${VAR}` expanding before use.
    pub fn as_strings(&self) -> Option<&[Vec<u8>]> {
        match &self.data {
            OwnedData::String(values) => Some(values),
            _ => None,
        }
    }

    /// A single `f32`, for the common scalar case.
    ///
    /// `None` unless the payload is exactly one `f32`, so a caller
    /// cannot silently read the first component of a colour as a float.
    pub fn as_f32(&self) -> Option<f32> {
        match self.as_f32s() {
            Some([value]) => Some(*value),
            _ => None,
        }
    }

    /// A single `i32`, on the same terms as [`OwnedArg::as_f32`].
    pub fn as_i32(&self) -> Option<i32> {
        match self.as_i32s() {
            Some([value]) => Some(*value),
            _ => None,
        }
    }

    /// A 4x4 `doublematrix`, row-major.
    ///
    /// `None` unless the declared type is [`nsi_trait::Type::MatrixF64`]
    /// with sixteen values: sixteen `double`s are not a `doublematrix`,
    /// and 3Delight refuses that too.
    pub fn as_matrix(&self) -> Option<[f64; 16]> {
        if self.type_tag != Type::MatrixF64 {
            return None;
        }
        match &self.data {
            OwnedData::F64(values) if values.len() == 16 => {
                Some(values[..16].try_into().expect("length checked"))
            }
            _ => None,
        }
    }

    /// Copy a borrowed parameter into owned storage.
    ///
    /// `pub(crate)` on purpose. It carried two failure paths that
    /// nothing in this crate can reach -- a panic if `as_c_param`
    /// returned `None`, and an empty `f32` array for `Type::Invalid` --
    /// because [`Recorder`](crate::Recorder)'s `Arg` GAT pins the
    /// parameter to `nsi_ffi_wrap::Arg`, whose `as_c_param` always
    /// returns `Some` and which never produces `Invalid`. Only a
    /// *foreign* `ParamValue` could reach either, and only through this
    /// being public.
    ///
    /// Narrowing it makes both unreachable by construction rather than
    /// by argument, which is cheaper than a `Result` every internal
    /// caller would have to unwrap for a case that cannot arise.
    /// Callers wanting an [`OwnedArg`] build one from its fields.
    ///
    /// It takes `Arg` rather than any `ParamValue` for the same reason:
    /// while it stayed generic, "unreachable" rested on nobody in this
    /// crate ever passing something else, which is a habit rather than
    /// a guarantee. Named, the compiler holds it.
    pub(crate) fn from_param(param: &Arg<'_, '_>) -> Self {
        let type_tag = param.type_tag();

        // The C call hands the renderer `count = len / array_length`
        // elements, so a run that does not divide is *dropped there*.
        // Copying `len` outright kept data 3Delight never saw -- and
        // made this crate's own stream fail the count `nsi-parse`
        // checks. Round the element count down the same way the C
        // boundary does.
        let array_length = param.array_length().max(1);
        let elements = param.len() / array_length * array_length;
        let scalars = elements * components_per_element(type_tag);

        // Unreachable while this is `pub(crate)`: the only implementor
        // reaching here is `nsi_ffi_wrap::Arg`, whose `as_c_param`
        // returns `Some` unconditionally.
        let c = param
            .as_c_param()
            .expect("nsi-ffi-wrap Arg always yields a C view");

        // SAFETY: `c.data` points at `scalars` values of the type named
        // by `type_tag`, valid while `param` lives, which is this call.
        let data = unsafe {
            match type_tag {
                Type::F32
                | Type::Color
                | Type::Point
                | Type::Vector
                | Type::Normal
                | Type::MatrixF32 => OwnedData::F32(
                    core::slice::from_raw_parts(c.data as *const f32, scalars)
                        .to_vec(),
                ),
                Type::F64 | Type::MatrixF64 => OwnedData::F64(
                    core::slice::from_raw_parts(c.data as *const f64, scalars)
                        .to_vec(),
                ),
                Type::I32 => OwnedData::I32(
                    core::slice::from_raw_parts(c.data as *const i32, scalars)
                        .to_vec(),
                ),
                Type::I64 => OwnedData::I64(
                    core::slice::from_raw_parts(c.data as *const i64, scalars)
                        .to_vec(),
                ),
                Type::String => {
                    let ptrs = core::slice::from_raw_parts(
                        c.data as *const *const c_char,
                        scalars,
                    );
                    OwnedData::String(
                        ptrs.iter()
                            .map(|p| CStr::from_ptr(*p).to_bytes().to_vec())
                            .collect(),
                    )
                }
                Type::Reference => OwnedData::Reference(
                    core::slice::from_raw_parts(
                        c.data as *const *const c_void,
                        scalars,
                    )
                    .iter()
                    .map(|p| HostPtr(*p))
                    .collect(),
                ),
                // Also unreachable: `Invalid` is ɴsɪ's C sentinel for
                // "no type" and nothing in this workspace produces it.
                // It used to yield an empty `f32` array, which would
                // have recorded a *different* argument rather than
                // refusing -- the silent wrong answer this crate exists
                // to avoid. If it ever fires, the pinned type has
                // changed and this must be revisited, not patched over.
                Type::Invalid => {
                    unreachable!("nsi-ffi-wrap never yields Type::Invalid")
                }
            }
        };

        Self {
            name: param.name().to_string(),
            type_tag,
            array_length: param.array_length(),
            flags: param.flags(),
            data,
        }
    }
}

/// Scalars per element for each ɴsɪ type.
#[inline]
const fn components_per_element(type_tag: Type) -> usize {
    match type_tag {
        Type::Color | Type::Point | Type::Vector | Type::Normal => 3,
        Type::MatrixF32 | Type::MatrixF64 => 16,
        _ => 1,
    }
}

#[cfg(test)]
mod tests;
