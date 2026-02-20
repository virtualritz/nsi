# `nsi-sys`

[![Build](https://github.com/virtualritz/nsi-sys/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi-sys/actions)
[![Documentation](https://docs.rs/nsi-sys/badge.svg)](https://docs.rs/nsi-sys)
[![Crate](https://img.shields.io/crates/v/nsi-sys.svg)](https://crates.io/crates/nsi-sys)
[![Chat](https://badges.gitter.im/n-s-i/community.svg)](https://gitter.im/n-s-i/community)
![Maintenance](https://img.shields.io/badge/maintenance-passively--maintained-yellowgreen.svg)

Auto-generated Rust bindings for _Illumination Research_’s _Nodal
Scene Interface_ – ɴsɪ.

You should not need to use this crate directly except for two
reasons. You are likely either:

- a masochist who wants to use the C-API directly from Rust.
- Not happy with my high level Rust binding (see below) – consider
  opening an issue [here](https://github.com/virtualritz/nsi/issues)
  instead.
- writing a renderer that exposes an ɴsɪ C-API.

## High Level Bindings

There are high level Rust bindings for this API in the
[ɴsɪ crate](https://crates.io/crates/nsi/).

### Differences From The C API

All `enum`s have been rustified – they were mapped to actual Rust `enum`s.

Postfixes were stripped on `enum` and `struct` type names. E.g.:

[`NSIParam_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L69-L77)
⟶ [`NSIParam`]

Prefixes and postfixes were stripped on `enum` variants. E.g.:

[`NSIType_t`](https://github.com/virtualritz/nsi-sys/blob/f1f05da59b558f9dd18f7afd37aa82d72b73b7da/include/nsi.h#L27-L41)`::NSITypeInvalid`
⟶ [`NSIType`]`::Invalid`

Rationale: make code using the bindings a bit less convoluted resp. easier
to read.

Finally, [`NSIParamFlags`] is a [`bitflags`](https://docs.rs/bitflags)
`struct` that wraps the `NSIParam*` flags from the C-API for ergonomics.

## Compile- vs. Runtime

The crate builds as-is, with default features.

However, at runtime this crate requires a library/renderer that
implements the ɴsɪ C-API to link against. Currently the only
renderer that does is [_3Delight_](https://www.3delight.com/).

## Features

- `download_lib3delight` – Fetches the dynamic library version of _3Delight
  2.1.2_ for _Linux_, _macOS_ or _Windows_.

  This can be used as a fallback, to build against, if you do not have the
  renderer installed on your system. But it is an old version of 3Delight
  and foremost a CI feature.

  It is instead suggested that you [download a _3Delight_
  package](https://www.3delight.com/download) for your platform & install
  it. This will set the `DELIGHT` environment variable that the build
  script is looking for to find a locally installed library to link
  against. Free version renders with up to 12 cores.

  This will also install _3Delight Display_ which you can render to,
  progressively – useful for debugging.

- `link_lib3delight` – Links against the dynamic library version of
  3Delight. Requires the `DELIGHT` environment variable to be set.

- `omit_functions` – Omit generating bindings for the API's functions. This
  is for the case where you want to expose your own C-API hooks from your
  renderer.

## License

Apache-2.0 OR BSD-3-Clause OR MIT OR Zlib

at your option.
