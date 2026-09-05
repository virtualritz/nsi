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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum OwnedData {
    F32(Vec<f32>),
    F64(Vec<f64>),
    I32(Vec<i32>),
    I64(Vec<i64>),
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
    pub name: String,
    pub type_tag: Type,
    pub array_length: usize,
    pub flags: i32,
    pub data: OwnedData,
}

impl OwnedArg {
    /// Copy a borrowed parameter into owned storage.
    pub fn from_param<P: ParamValue>(param: &P) -> Self {
        let type_tag = param.type_tag();

        // `ParamValue::len()` is the raw element count, not the C
        // `count` field (which is `len / array_length`). Using the
        // divided value here would under-read an `array_len`-ed
        // argument and silently truncate it.
        let scalars = param.len() * components_per_element(type_tag);

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
                    core::slice::from_raw_parts(c.data as *const f32, scalars).to_vec(),
                ),
                Type::F64 | Type::MatrixF64 => OwnedData::F64(
                    core::slice::from_raw_parts(c.data as *const f64, scalars).to_vec(),
                ),
                Type::I32 => OwnedData::I32(
                    core::slice::from_raw_parts(c.data as *const i32, scalars).to_vec(),
                ),
                Type::I64 => OwnedData::I64(
                    core::slice::from_raw_parts(c.data as *const i64, scalars).to_vec(),
                ),
                Type::String => {
                    let ptrs = core::slice::from_raw_parts(c.data as *const *const c_char, scalars);
                    OwnedData::String(
                        ptrs.iter()
                            .map(|p| CStr::from_ptr(*p).to_string_lossy().into_owned())
                            .collect(),
                    )
                }
                Type::Reference => OwnedData::Reference(
                    core::slice::from_raw_parts(c.data as *const *const c_void, scalars)
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
mod tests {
    use super::*;
    use nsi_ffi_wrap as nsi;
    use nsi_trait::Type;

    #[test]
    fn owns_a_single_f32() {
        let arg = nsi::f32!("roughness", 0.3);
        let owned = OwnedArg::from_param(&arg);
        assert_eq!(owned.name, "roughness");
        assert_eq!(owned.type_tag, Type::F32);
        assert_eq!(owned.data, OwnedData::F32(vec![0.3]));
    }

    #[test]
    fn owns_a_point_slice_with_all_floats() {
        let points = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let arg = nsi::point_slice!("P", &points);
        let owned = OwnedArg::from_param(&arg);
        assert_eq!(owned.type_tag, Type::Point);
        // Two points, three floats each: the storage keeps all six,
        // flattened.
        assert_eq!(
            owned.data,
            OwnedData::F32(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0])
        );
    }

    /// An `array_len`-ed argument must keep every scalar. The C `count`
    /// is `len / array_length`, so deriving the scalar count from it
    /// would silently truncate.
    #[test]
    fn owns_every_scalar_of_an_array_len_argument() {
        use std::num::NonZeroUsize;
        let resolution = [1280i32, 720];
        let arg = nsi::i32_slice!("resolution", &resolution)
            .array_len(const { NonZeroUsize::new(2).unwrap() });
        let owned = OwnedArg::from_param(&arg);
        assert_eq!(owned.array_length, 2);
        assert_eq!(owned.data, OwnedData::I32(vec![1280, 720]));
    }

    /// The alignment check against `nsi`'s pointer-marshalling
    /// contract (fixed upstream in 20ae58c).
    ///
    /// `Reference::as_c_ptr` yields `&self.data`, not `self.data` --
    /// `data` addresses the array of values, and for a pointer-typed
    /// parameter the value *is* the pointer. So the recorder must
    /// dereference exactly one level to recover the host address.
    /// One level too few and it would store the pointee's first eight
    /// bytes; one too many and it would follow into the payload.
    #[test]
    fn records_a_reference_as_the_address_not_its_contents() {
        let payload = Box::new(0xdead_beef_cafe_f00d_u64);
        let expected = &*payload as *const u64 as usize;

        let arg = nsi::reference!("outputdriver", &payload);
        let owned = OwnedArg::from_param(&arg);

        assert_eq!(owned.type_tag, Type::Reference);
        match &owned.data {
            OwnedData::Reference(pointers) => {
                assert_eq!(pointers.len(), 1);
                assert_eq!(
                    pointers[0].0 as usize,
                    expected,
                    "recorder must store the payload's address"
                );
            }
            other => panic!("expected Reference, got {other:?}"),
        }
    }

    #[test]
    fn owns_a_string() {
        let arg = nsi::string!("shaderfilename", "dlPrincipled");
        let owned = OwnedArg::from_param(&arg);
        assert_eq!(owned.type_tag, Type::String);
        assert_eq!(
            owned.data,
            OwnedData::String(vec!["dlPrincipled".to_string()])
        );
    }
}
