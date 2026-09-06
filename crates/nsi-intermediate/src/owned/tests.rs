//! Tests for [`super`].
//!
//! Separate file per the workspace rule: source files do not grow
//! inline `#[cfg(test)]` modules.

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
                pointers[0].0 as usize, expected,
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
        OwnedData::String(vec![b"dlPrincipled".to_vec()])
    );
}

/// The C boundary hands the renderer `count = len / array_length`
/// elements, so a run that does not divide is dropped *there*. Keeping
/// it here made the recorder disagree with what 3Delight saw, and made
/// this crate's own stream fail the count a reader checks.
#[test]
fn an_array_len_run_is_rounded_down_as_the_c_call_does() {
    use std::num::NonZeroUsize;

    let arg = nsi::f32_slice!("x", &[1.0f32, 2.0, 3.0])
        .array_len(const { NonZeroUsize::new(2).unwrap() });
    let owned = OwnedArg::from_param(&arg);

    assert_eq!(
        owned.data,
        OwnedData::F32(vec![1.0, 2.0]),
        "the renderer reads one element of float[2]; the third is not sent"
    );
}

/// The same for a tuple type, where each element is three floats.
#[test]
fn a_tuple_array_len_run_is_rounded_down_too() {
    use std::num::NonZeroUsize;

    let points = [[0.0f32, 0.0, 0.0], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]];
    let arg = nsi::point_slice!("P", &points)
        .array_len(const { NonZeroUsize::new(2).unwrap() });
    let owned = OwnedArg::from_param(&arg);

    assert_eq!(
        owned.data,
        OwnedData::F32(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0])
    );
}

/// Recording must not repair a non-UTF-8 string.
///
/// `nsi::String::new` takes `Into<Vec<u8>>`, so a caller -- or
/// `nsi-parse` reading a stream 3Delight wrote -- can hand across a
/// Latin-1 file name. Converting it here with `to_string_lossy`
/// replaced the byte with U+FFFD before replay could ever see it, and
/// the loss was silent: the stream still parsed, it just named a
/// different file.
///
/// This is the recording end specifically. Reverting the conversion
/// reddened only `nsi-parse`'s round-trip tests, so this crate did not
/// guard its own boundary.
#[test]
fn recording_keeps_a_non_utf8_byte() {
    let arg = nsi::string!("imagefilename", b"caf\xE9.exr".to_vec());
    let owned = OwnedArg::from_param(&arg);

    assert_eq!(owned.type_tag, Type::String);
    assert_eq!(
        owned.data,
        OwnedData::String(vec![b"caf\xE9.exr".to_vec()]),
        "the byte survives; U+FFFD would be `ef bf bd`",
    );
}

/// The typed accessors read the payload they name and refuse the rest.
///
/// Every consumer wrote the same `match &arg.data` by hand, and the
/// interesting part is what they must *not* do: read the first
/// component of a colour as a scalar, or sixteen `double`s as a matrix.
#[test]
fn typed_accessors_refuse_the_wrong_layout() {
    let fov = OwnedArg::from_param(&nsi::f32!("fov", 45.0));
    assert_eq!(fov.as_f32(), Some(45.0));
    assert_eq!(fov.as_f32s(), Some(&[45.0f32][..]));
    assert!(fov.as_i32s().is_none());
    assert!(fov.as_matrix().is_none());

    // A colour is three `f32`s, so the scalar accessor must decline.
    let c = OwnedArg::from_param(&nsi::color!("c", &[0.1, 0.2, 0.3]));
    assert_eq!(c.as_f32s().map(<[f32]>::len), Some(3));
    assert_eq!(c.as_f32(), None, "not the first component of a colour");

    #[rustfmt::skip]
    let m = [
        1.0f64, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        7.0, 0.0, 0.0, 1.0,
    ];
    let matrix = OwnedArg::from_param(&nsi::matrix_f64!("m", &m));
    assert_eq!(matrix.as_matrix().map(|v| v[12]), Some(7.0));

    // Sixteen `double`s are not a `doublematrix`; 3Delight refuses it.
    let sixteen = OwnedArg {
        name: "m".to_string(),
        type_tag: Type::F64,
        array_length: 1,
        flags: 0,
        data: OwnedData::F64(m.to_vec()),
    };
    assert_eq!(sixteen.as_matrix(), None);
    assert_eq!(sixteen.as_f64s().map(<[f64]>::len), Some(16));

    let s = OwnedArg::from_param(&nsi::string!("f", b"caf\xE9".to_vec()));
    assert_eq!(s.as_strings(), Some(&[b"caf\xE9".to_vec()][..]));
    assert!(s.as_f32s().is_none());
}
