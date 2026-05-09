//! Standard ɴsɪ node types — re-exported from `nsi-trait`.
//!
//! The string constants and the [`NodeType`] enum live in the `nsi-trait`
//! crate as the single source of truth. This module re-exports them so user
//! code can write `nsi_ffi_wrap::MESH`, `nsi_ffi_wrap::NURBS`, etc.

pub use ::nsi_trait::node::*;
