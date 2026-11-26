//! Typed arguments for ɴsɪ calls, without FFI.
//!
//! [`Arg`] and its value types are the argument currency of every ɴsɪ
//! backend in this workspace. This crate depends on `nsi-trait` only, so
//! a crate that builds, records or parses arguments (`nsi-intermediate`,
//! `nsi-parse`) does not pull in `nsi-sys` and its `bindgen` build.
//!
//! An [`Arg`] converts to [`FfiParam`], which has the `NSIParam` layout
//! of `nsi.h`. `nsi-ffi-wrap` hands that to the renderer and re-exports
//! everything here under its old paths (`nsi::Arg`, `nsi::real_f32!`,
//! ...).

#[macro_use]
pub mod argument;
// The wrappers that borrow argument data share their names with the
// data shapes `nsi-trait` exports (`Color3F32` is both `[f32; 3]` and
// the wrapper around a `&[f32; 3]`), so they stay in `argument` and only
// the names without a twin are re-exported here.
pub use argument::{
    AdoptedCallback, Arg, ArgData, ArgDataMethods, ArgSlice, ArgVec, Callback,
    CallbackPtr, DataType, Reference, ReferenceSlice, StableDeref, String,
    StringSlice,
};

// The macros name `$crate::Attribute` and the shape aliases
// (`$crate::Point3F32`, ...), so `nsi-trait` is re-exported at the root,
// as `nsi-ffi-wrap` does.
pub use ::nsi_trait::*;

// Bridges `Arg` onto `nsi_trait::ParamValue` and `FfiParam`.
mod param_value;
