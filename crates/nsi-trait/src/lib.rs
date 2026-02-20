//! Core traits and types for the Nodal Scene Interface -- ɴsɪ.
//!
//! This crate provides the fundamental abstractions for ɴsɪ without any FFI
//! dependencies. It defines:
//!
//! - [`Nsi`] -- The core trait that rendering contexts implement (Self IS the context)
//! - [`Parameter`] -- Trait for individual typed parameters
//! - [`Type`] -- NSI data type discriminant (`#[repr(i32)]`, C-compatible)
//! - [`FfiParam`] -- C-compatible parameter struct (`#[repr(C)]`)
//! - [`Flags`] -- Parameter flags (PerFace, PerVertex, etc.)
//! - [`Name`] -- Feature-gated string type (`ustr::Ustr` or `String`)
//! - [`Action`] -- Render control actions
//! - [`NodeType`] -- Enum of all standard node types
//! - Node type constants ([`ROOT`], [`MESH`], etc.)
//!
//! # Crate Organization
//!
//! This is a pure Rust crate with no FFI. For FFI wrapper functionality,
//! see the `nsi-ffi-wrap` crate.

mod node;
mod nsi_trait;

pub use node::*;
pub use nsi_trait::*;
