#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
//! Auto-generated Rust bindings for *Illumination Research*'s *Nodal
//! Scene Interface* -- ɴsɪ.
//!
//! You should not need to use this crate directly except for two
//! reasons. You are likely either:
//!
//! * a masochist who wants to use the C-API directly from Rust.
//!
//! * Not happy with my high level Rust binding (see below) -- consider
//!   opening an issue [here](https://github.com/virtualritz/nsi/issues)
//!   instead.
//!
//! * writing a renderer that exposes an ɴsɪ C-API.
//!
//! # High Level Bindings
//!
//! There are high level Rust bindings for this API in the
//! [ɴsɪ crate](https://crates.io/crates/nsi/).
//!
//! ## Differences From The C API
//!
//! All `enum`s have been rustified -- they were mapped to actual Rust `enum`s.
//!
//! Postfixes were stripped on `enum` and `struct` type names. E.g.:
//!
//! [`NSIParam_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L69-L77)
//! ⟶ [`NSIParam`]
//!
//! Prefixes and postfixes were stripped on `enum` variants. E.g.:
//!
//! [`NSIType_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L27-L41)`::NSITypeInvalid`
//! ⟶ [`NSIType`]`::Invalid`
//!
//! Rationale: make code using the bindings a bit less convoluted resp. easier
//! to read.
//!
//! Finally, [`NSIParamFlags`] is a [`bitflags`](https://docs.rs/bitflags)
//! `struct` that wraps the `NSIParam*` flags from the C-API for ergonomics.
//!
//! # Compile- vs. Runtime
//!
//! The crate builds as-is, with default features.
//!
//! However, at runtime this crate requires a library/renderer that
//! implements the ɴsɪ C-API to link against. Currently the only
//! renderer that does is [*3Delight*](https://www.3delight.com/).
//!
//! # Features
//!
//! * `omit_functions` -- Omit generating bindings for the API's functions. This
//!   is for the case where you want to expose your own C-API hooks from your
//!   renderer.
use bitflags::bitflags;
use std::os::raw::c_int;

mod c_api {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use c_api::{
    NSI_ALL_ATTRIBUTES, NSI_ALL_NODES, NSI_SCENE_GLOBAL, NSI_SCENE_ROOT,
    NSI_VERSION, NSIContext, NSIErrorHandler, NSIErrorLevel, NSIHandle,
    NSIParam, NSIProcedural, NSIProceduralExecute, NSIProceduralLoad,
    NSIProceduralUnload, NSIRenderStopped, NSIReport, NSIStoppingStatus,
    NSIType,
};

#[cfg(not(feature = "omit_functions"))]
pub use c_api::{
    NSIBegin, NSIConnect, NSICreate, NSIDelete, NSIDeleteAttribute,
    NSIDisconnect, NSIEnd, NSIEvaluate, NSIRenderControl, NSISetAttribute,
    NSISetAttributeAtTime,
};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct NSIParamFlags: c_int {
        const InterpolateLinear = c_api::NSIParamInterpolateLinear as _;
        const IsArray = c_api::NSIParamIsArray as _;
        const PerFace = c_api::NSIParamPerFace as _;
        const PerVertex = c_api::NSIParamPerVertex as _;
    }
}

impl From<i32> for NSIErrorLevel {
    fn from(level: i32) -> Self {
        match level {
            0 => NSIErrorLevel::Message,
            1 => NSIErrorLevel::Info,
            2 => NSIErrorLevel::Warning,
            _ => NSIErrorLevel::Error,
        }
    }
}
