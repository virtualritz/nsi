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
