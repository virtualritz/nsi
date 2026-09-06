# `nsi-ffi-wrap`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-ffi-wrap/badge.svg)](https://docs.rs/nsi-ffi-wrap)
[![Crate](https://img.shields.io/crates/v/nsi-ffi-wrap.svg)](https://crates.io/crates/nsi-ffi-wrap)

FFI wrapper layer for the [Nodal Scene Interface](https://nsi.readthedocs.io/)
(ɴsɪ) — turns the C API exposed by [`nsi-sys`](https://crates.io/crates/nsi-sys)
into idiomatic Rust.

This crate provides:

- `Context` — a safe wrapper around an ɴsɪ rendering context with the
  full set of methods (`create`, `delete`, `connect`, `disconnect`,
  `set_attribute`, `set_attribute_at_time`, `evaluate`,
  `render_control`).
- A loader that opens the renderer's shared library at runtime via
  [`dlopen2`](https://crates.io/crates/dlopen2). With the
  `link_lib3delight` feature the loader is replaced by a static link
  against `lib3delight`.
- Typed parameter macros for every ɴsɪ data type — `f32!`, `f32_slice!`,
  `f64!`, `f64_slice!`, `i32!`, `i32_slice!`, `i64!`, `i64_slice!`,
  `string!`, `string_slice!`, `color!`, `color_slice!`, `point!`,
  `point_slice!`, `point4_f32_slice!` (for `Pw`), `vector!`,
  `vector_slice!`, `normal!`, `normal_slice!`, `matrix_f32!`,
  `matrix_f32_slice!`, `matrix_f64!`, `matrix_f64_slice!`, `reference!`,
  `reference_slice!`, `callback!`.
- `FfiApiAdapter` — exposes any pure-Rust `Nsi` trait impl (from the
  [`nsi-trait`](https://crates.io/crates/nsi-trait) crate) through the
  C API via a factory closure.
- An optional `output` module (feature `output`) that streams pixel
  buckets out of the renderer through user-supplied callbacks, with
  typed pixel formats (`FERRIS_F32`, `FERRIS_U8`, …).

If you just want to render a scene, use the umbrella
[`nsi`](https://crates.io/crates/nsi) crate, which re-exports this
crate together with the toolbelt, 3Delight helpers, and Jupyter
support.

## Features

- `link_lib3delight` — link against `lib3delight` at build time instead
  of resolving it at runtime through `dlopen2`. Requires the `DELIGHT`
  environment variable to be set.
- `download_lib3delight` — fetch a dynamic library version of *3Delight*
  during the build (CI / fallback use).
- `output` — pixel-streaming support (callbacks from the renderer to
  Rust closures).
- `nightly` — enables nightly-only documentation features.
- `ustr_handles` — use [`ustr`](https://crates.io/crates/ustr) for node
  handles (interned, cheap to clone). Default is `CString`.

**`--all-features` is not a configuration this crate has.**
`link_lib3delight` and the default `dlopen2` are two answers to the
same question, and enabling both together links a library the runtime
loader was going to find for itself — with `download_lib3delight` also
on, that library is a *downloaded* one, so the renderer under test is
not the one installed. The golden-image tests then differ by tens of
percent for reasons that have nothing to do with this crate. Test with
`--features output`, which is what exercises the renderer, and see
`[package.metadata.docs.rs]` for the same reason stated to docs.rs.

## Testing

```bash
DELIGHT=/path/to/3delight cargo test -p nsi-ffi-wrap --features output
```

`DELIGHT` must be set: the render tests name shaders as
`${DELIGHT}/osl/...` and the renderer expands that from the
environment. Without it every shader fails to load and the images
differ wildly.

`tests/generate_test_image.rs` is `#[ignore]`d because it is a tool
rather than a test — it sets `RUST_TEST_UPDATE` and overwrites
`tests/expected_images/sphere.png`. Run it deliberately, look at what
it produced, and commit that on purpose:

```bash
cargo test -p nsi-ffi-wrap --features output --test generate_test_image -- --ignored
```

## License

Licensed under any of

- Apache License, Version 2.0
- MIT License
- zlib License

at your option.
