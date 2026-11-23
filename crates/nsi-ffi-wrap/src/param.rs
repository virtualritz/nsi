//! Converts [`Arg`]s to the C API's [`NSIParam`] layout.
//!
//! [`Arg`] lives in `nsi-argument`, which does not depend on `nsi-sys`.
//! It converts to `nsi-trait`'s [`FfiParam`], and this module turns that
//! into the `NSIParam` the renderer reads.
use crate::ArgSlice;
use nsi_sys::NSIParam;
use nsi_trait::FfiParam;

/// The C view of one parameter.
///
/// [`FfiParam`] is layout-identical to [`NSIParam`]. The fields are copied
/// one by one rather than transmuted, so a change to either struct is a
/// compile error here.
#[inline(always)]
pub(crate) fn to_nsi_param(param: FfiParam) -> NSIParam {
    NSIParam {
        name: param.name,
        data: param.data,
        type_: param.type_ as _,
        arraylength: param.arraylength as _,
        count: param.count as _,
        flags: param.flags as _,
    }
}

#[inline(always)]
pub(crate) fn to_c_param_vec(
    args: Option<&ArgSlice>,
) -> (i32, *const NSIParam, Vec<NSIParam>) {
    let args = match args {
        Some(args) => args
            .iter()
            .map(|arg| to_nsi_param(FfiParam::from(arg)))
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    (args.len() as _, args.as_ptr(), args)
}
