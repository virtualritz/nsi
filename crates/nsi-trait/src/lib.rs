//! Core traits and types for the Nodal Scene Interface -- ɴsɪ.
//!
//! This crate provides the fundamental abstractions for ɴsɪ without any FFI
//! dependencies. It defines:
//!
//! - [`Nsi`] -- the core trait that rendering contexts implement (`self` _is_ the context)
//! - [`ParamValue`] -- trait implemented by Rust values that can be expressed
//!   as a single ɴsɪ `NSIParam_t` at the FFI boundary
//! - [`Attribute<T>`](attribute::Attribute) and its alias [`Parameter<T>`](attribute::Parameter)
//!   -- typed compile-time names
//! - [`Type`] -- NSI data type discriminant (`#[repr(i32)]`, C-compatible)
//! - [`FfiParam`] -- C-compatible parameter struct (`#[repr(C)]`)
//! - [`Flags`] -- Parameter flags (PerFace, PerVertex, etc.)
//! - [`Name`] -- Feature-gated string type (`ustr::Ustr` or `String`)
//! - [`Action`] -- Render control actions
//! - [`NodeType`] -- Enum of all standard node types
//! - Node type constants ([`ROOT`], [`MESH`], etc.)
//! - [`Attribute<T>`](attribute::Attribute) -- Typed attribute names with
//!   per-name compile-time data-shape verification, plus standard attribute
//!   constants ([`attribute::POSITION`], [`attribute::FIELD_OF_VIEW`],
//!   etc.)
//!
//! # Crate Organization
//!
//! This is a pure Rust crate with no FFI. For FFI wrapper functionality,
//! see the `nsi-ffi-wrap` crate.

pub mod attribute;
pub mod node;
mod nsi_trait;

pub use attribute::*;
pub use node::*;
pub use nsi_trait::*;
