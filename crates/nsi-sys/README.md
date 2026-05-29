# `nsi-sys`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-sys/badge.svg)](https://docs.rs/nsi-sys)
[![Crate](https://img.shields.io/crates/v/nsi-sys.svg)](https://crates.io/crates/nsi-sys)
[![Chat](https://badges.gitter.im/n-s-i/community.svg)](https://gitter.im/n-s-i/community)
![Maintenance](https://img.shields.io/badge/maintenance-passively--maintained-yellowgreen.svg)

<!-- cargo-rdme start -->

Auto-generated Rust bindings for *Illumination Research*'s *Nodal
Scene Interface* -- ɴsɪ.

You should not need to use this crate directly except for two
reasons. You are likely either:

* a masochist who wants to use the C-API directly from Rust.

* Not happy with my high level Rust binding (see below) -- consider
  opening an issue [here](https://github.com/virtualritz/nsi/issues)
  instead.

* writing a renderer that exposes an ɴsɪ C-API.

## High Level Bindings

There are high level Rust bindings for this API in the
[ɴsɪ crate](https://crates.io/crates/nsi/).

### Differences From The C API

All `enum`s have been rustified -- they were mapped to actual Rust `enum`s.

Postfixes were stripped on `enum` and `struct` type names. E.g.:

[`NSIParam_t`](https://github.com/virtualritz/nsi/blob/master/crates/nsi-sys/include/nsi.h#L69-L77)
⟶ [`NSIParam`](https://docs.rs/nsi-sys/latest/nsi_sys/struct.NSIParam.html)

`enum` variants were renamed to mirror Rust primitive type names where
applicable (`Float` ⟶ `F32`, `Double` ⟶ `F64`, `Integer` ⟶ `I32`,
`Matrix` ⟶ `MatrixF32`, `DoubleMatrix` ⟶ `MatrixF64`) and to drop the
redundant `NSIType` / `NSI` prefix everywhere else. The full mapping for
[`NSIType`](https://docs.rs/nsi-sys/latest/nsi_sys/enum.NSIType.html):

| C-API name            | Rust variant |
|-----------------------|--------------|
| `NSITypeInvalid`      | `Invalid`    |
| `NSITypeFloat`        | `F32`        |
| `NSITypeDouble`       | `F64`        |
| `NSITypeInteger`      | `I32`        |
| `NSITypeInt64`        | `I64`        |
| `NSITypeString`       | `String`     |
| `NSITypeColor`        | `Color`      |
| `NSITypePoint`        | `Point`      |
| `NSITypeVector`       | `Vector`     |
| `NSITypeNormal`       | `Normal`     |
| `NSITypeMatrix`       | `MatrixF32`  |
| `NSITypeDoubleMatrix` | `MatrixF64`  |
| `NSITypePointer`      | `Pointer`    |

Rationale: make code using the bindings a bit less convoluted resp. easier
to read.

Finally,
[`NSIParamFlags`](https://docs.rs/nsi-sys/latest/nsi_sys/struct.NSIParamFlags.html)
is a [`bitflags`](https://docs.rs/bitflags) `struct` that wraps the
`NSIParam*` flags from the C-API for ergonomics.

## Compile- vs. Runtime

The crate builds as-is, with default features.

However, at runtime this crate requires a library/renderer that
implements the ɴsɪ C-API to link against. Currently the only
renderer that does is [*3Delight*](https://www.3delight.com/).

## Features

* `omit_functions` -- Omit generating bindings for the API's functions. This
  is for the case where you want to expose your own C-API hooks from your
  renderer.

<!-- cargo-rdme end -->

## License

Apache-2.0 OR BSD-3-Clause OR MIT OR Zlib

at your option.
