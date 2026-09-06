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
    String(Vec<String>),
    /// Raw host pointers. ɴsɪ calls this `Reference` (`Pointer` in the C
    /// API); it is not an object link and is never forwarded to a
    /// renderer as one. Stored so output-driver callbacks survive a
    /// replay. The recorder never dereferences these.
    Reference(Vec<HostPtr>),
}

/// A recorded ɴsɪ argument.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedArg {
    /// The attribute name this argument sets.
    pub name: String,
    /// The ɴsɪ type, which is what tells one [`OwnedData`] layout from
    /// another sharing the same storage.
    pub type_tag: Type,
    /// ɴsɪ's `array_len`. The C `count` field is `len / array_length`.
    pub array_length: usize,
    /// ɴsɪ's argument flags: `per_vertex`, `per_face` and the like.
    /// Recorded but not yet replayed; see `contracts/stream.md`.
    pub flags: i32,
    /// The payload, copied unless it is a pointer.
    pub data: OwnedData,
}

impl OwnedArg {
    /// Copy a borrowed parameter into owned storage.
    pub fn from_param<P: ParamValue>(param: &P) -> Self {
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
                            .map(|p| {
                                CStr::from_ptr(*p)
                                    .to_string_lossy()
                                    .into_owned()
                            })
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
                Type::Invalid => OwnedData::F32(Vec::new()),
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
