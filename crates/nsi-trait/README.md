# `nsi-trait`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-trait/badge.svg)](https://docs.rs/nsi-trait)
[![Crate](https://img.shields.io/crates/v/nsi-trait.svg)](https://crates.io/crates/nsi-trait)

<!-- cargo-rdme start -->

Core traits and types for the Nodal Scene Interface -- ɴsɪ.

This crate provides the fundamental abstractions for ɴsɪ without any FFI
dependencies. It defines:

- `Nsi` -- the core trait that rendering contexts implement (`self` _is_ the context)
- `ParamValue` -- trait implemented by Rust values that can be expressed
  as a single ɴsɪ `NSIParam_t` at the FFI boundary
- [`Attribute<T>`](attribute::Attribute) and its alias [`Parameter<T>`](attribute::Parameter)
  -- typed compile-time names
- `Type` -- NSI data type discriminant (`#[repr(i32)]`, C-compatible)
- `FfiParam` -- C-compatible parameter struct (`#[repr(C)]`)
- `Flags` -- Parameter flags (PerFace, PerVertex, etc.)
- `Name` -- Feature-gated string type (`ustr::Ustr` or `String`)
- `Action` -- Render control actions
- `NodeType` -- Enum of all standard node types
- Node type constants (`ROOT`, `MESH`, etc.)
- [`Attribute<T>`](attribute::Attribute) -- Typed attribute names with
  per-name compile-time data-shape verification, plus standard attribute
  constants ([`P`](attribute::P), [`FOV`](attribute::FOV), etc.)

## Crate Organization

This is a pure Rust crate with no FFI. For FFI wrapper functionality,
see the `nsi-ffi-wrap` crate.

<!-- cargo-rdme end -->

## License

Licensed under any of

- Apache License, Version 2.0
- MIT License
- zlib License

at your option.
