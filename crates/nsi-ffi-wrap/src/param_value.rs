//! `ParamValue` bridge: exposes [`Arg`] through the renderer-agnostic
//! `nsi-trait` interface.
//!
//! Lives here rather than in `argument.rs` because it reaches into
//! [`Arg`]'s `pub(crate)` fields, and because keeping the trait bridge
//! in one small file makes it reviewable on its own.

use crate::argument::{Arg, ArgDataMethods, DataType};
use ::nsi_trait::{FfiParam, ParamValue, Type};

/// Map the crate-private [`DataType`] onto the public `nsi-trait`
/// [`Type`].
///
/// Both are `repr(i32)` over the same `NSIType` discriminants, so the
/// values already agree. The match is written out so that adding a
/// variant to either enum is a compile error rather than a silent
/// mismatch.
#[inline]
const fn to_trait_type(data_type: DataType) -> Type {
    match data_type {
        DataType::RealF32 => Type::RealF32,
        DataType::RealF64 => Type::RealF64,
        DataType::IntegerI32 => Type::IntegerI32,
        DataType::IntegerI64 => Type::IntegerI64,
        DataType::String => Type::String,
        DataType::Color3F32 => Type::Color3F32,
        DataType::Point3F32 => Type::Point3F32,
        DataType::Vector3F32 => Type::Vector3F32,
        DataType::Normal3F32 => Type::Normal3F32,
        DataType::Point4F32 => Type::Point4F32,
        DataType::Matrix4F32 => Type::Matrix4F32,
        DataType::Matrix4F64 => Type::Matrix4F64,
        DataType::Reference => Type::Reference,
    }
}

impl<'a, 'b> ParamValue for Arg<'a, 'b> {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    fn type_tag(&self) -> Type {
        to_trait_type(self.data.type_())
    }

    /// The raw element count, matching `ArgDataMethods::len()`.
    ///
    /// This is *not* the C `count` field: that is `len() /
    /// array_length`. For a `PointSlice` over `&[[f32; 3]]` this
    /// returns the number of points, and the payload holds three times
    /// as many `f32`s.
    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    fn array_length(&self) -> usize {
        self.array_length.get()
    }

    #[inline]
    fn flags(&self) -> i32 {
        self.flags
    }

    fn as_c_param(&self) -> Option<FfiParam> {
        Some(FfiParam {
            name: self.name.as_char_ptr(),
            data: self.data.as_c_ptr(),
            type_: self.data.type_() as core::ffi::c_int,
            arraylength: self.array_length.get() as core::ffi::c_int,
            // `count` is `data.len()/array_length`, exactly as
            // `to_c_param_vec` computes it. These two must stay
            // identical or the FFI fast path disagrees with the slow
            // one. `array_length` is `NonZeroUsize`, so this cannot
            // divide by zero.
            count: self.data.len() / self.array_length,
            flags: self.flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate as nsi;
    use crate::{ParamValue, Type};

    #[test]
    fn f32_arg_reports_name_type_and_len() {
        let arg = nsi::real_f32!("roughness", 0.3);
        assert_eq!(arg.name(), "roughness");
        assert_eq!(arg.type_tag(), Type::RealF32);
        assert_eq!(arg.len(), 1);
        assert_eq!(arg.array_length(), 1);
        assert_eq!(arg.flags(), 0);
    }

    #[test]
    fn point_slice_reports_point_count_not_float_count() {
        // PointSlice is nsi_tuple_data_array_def!(f32, .., 3), so it
        // takes `&[[f32; 3]]` -- a flat `&[f32]` will not compile.
        let points = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let arg = nsi::point3_f32_slice!("P", &points);
        assert_eq!(arg.type_tag(), Type::Point3F32);
        assert_eq!(arg.len(), 2);
        assert_eq!(arg.as_c_param().unwrap().count, 2);
    }

    /// `count` is `data.len()/array_length`; `array_len(2)` over two
    /// i32s is one element of length two, not two elements.
    #[test]
    fn array_len_divides_the_c_count() {
        use std::num::NonZeroUsize;
        let resolution = [1280i32, 720];
        let arg = nsi::integer_i32_slice!("resolution", &resolution)
            .array_len(const { NonZeroUsize::new(2).unwrap() });
        assert_eq!(arg.len(), 2);
        let c = arg.as_c_param().unwrap();
        assert_eq!(c.arraylength, 2);
        assert_eq!(c.count, 1);
    }

    #[test]
    fn as_c_param_matches_the_arg() {
        let arg = nsi::real_f32!("fov", 45.0);
        let c = arg.as_c_param().expect("Arg always has a C view");
        assert_eq!(c.type_, Type::RealF32 as i32);
        assert_eq!(c.count, 1);
        assert_eq!(c.arraylength, 1);
        assert!(!c.data.is_null());
    }
}
