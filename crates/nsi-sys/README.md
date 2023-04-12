
# `nsi-sys`

[![Build](https://github.com/virtualritz/nsi-sys/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi-sys/actions)
[![Documentation](https://docs.rs/nsi-sys/badge.svg)](https://docs.rs/nsi-sys)
[![Crate](https://img.shields.io/crates/v/nsi-sys.svg)](https://crates.io/crates/nsi-sys)
[![Chat](https://badges.gitter.im/n-s-i/community.svg)](https://gitter.im/n-s-i/community)
![Maintenance](https://img.shields.io/badge/maintenance-passively--maintained-yellowgreen.svg)

Auto-generated Rust bindings for *Illumination Research*’s *Nodal
Scene Interface* – ɴsɪ.

You should not need to use this crate directly except for two
reasons. You are likely either:

* writing a renderer that exposes an ɴsɪ C-API.

* a masochist who wants to use the C-API directly from Rust.

## High Level Bindings

There are high level Rust bindings for this API in the
[ɴsɪ crate](https://crates.io/crates/nsi/).

### Differences From The C API

All `enum`s have been rustified. Meaning they were mapped to actual Rust `enum`s.

Postfixes were stripped on `enum` and `struct` type names. E.g.:

[`NSIParam_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L69-L77)
⟶ `NSIParam`

Prefixes and postfixes were stripped on `enum` variants. E.g.:

[`NSIType_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L27-L41)`::NSITypeInvalid`
⟶ `NSIType::Invalid`

Rationale: make code using the bindings a bit less convoluted resp. easier
to read.

Finally, `NSIParamFlags` is a [`bitflags`](https://docs.rs/bitflags)
struct that wraps the `NSIParam*` flags from the C-API for ergnomics.

## Compile- vs. Runtime

The crate builds as-is, with default features.

However, at runtime this crate requires a library/renderer that
implements the ɴsɪ C-API to link against. Currently the only
renderer that does is [*3Delight*](https://www.3delight.com/).

## Features

* `omit_functions` – Omit generating bindings for the API's
   functions. This is for the casewhere you want to expose your own
   C-API hooks from your renderer.

## License

Apache-2.0 OR BSD-3-Clause OR MIT OR Zlib at your option.
