//! The names before the role-and-machine-type scheme still compile, and
//! mean what the new names mean.
#![allow(deprecated)]

use nsi_ffi_wrap::{self as nsi, ParamValue, Type};

/// An old macro builds the argument its replacement builds.
#[test]
fn old_macros_forward_to_the_new_ones() {
    let pairs = [
        (nsi::f32!("a", 1.0), nsi::real_f32!("a", 1.0)),
        (nsi::i32!("a", 1), nsi::integer_i32!("a", 1)),
        (nsi::i64!("a", 1), nsi::integer_i64!("a", 1)),
        (nsi::f64!("a", 1.0), nsi::real_f64!("a", 1.0)),
        (
            nsi::color!("a", &[1.0, 0.0, 0.0]),
            nsi::color3_f32!("a", &[1.0, 0.0, 0.0]),
        ),
    ];
    for (old, new) in pairs {
        assert_eq!(old.type_tag(), new.type_tag());
        assert_eq!(old.len(), new.len());
    }
}

/// An old wrapper name is an alias of the new one.
#[test]
fn old_wrapper_names_are_aliases() {
    let points = [[0.0f32, 1.0, 2.0]];
    let old =
        nsi::Arg::new("P", nsi::ArgData::from(nsi::PointSlice::new(&points)));
    assert_eq!(old.type_tag(), Type::Point3F32);
}

/// An old `Type` name works as a value and as a pattern.
#[test]
fn old_type_names_work_in_patterns() {
    assert_eq!(Type::F32, Type::RealF32);
    let described = match Type::Matrix4F64 {
        Type::MatrixF64 => "a double matrix",
        _ => "something else",
    };
    assert_eq!(described, "a double matrix");
}
