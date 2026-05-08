# nsi-trait

Core traits and types for the [Nodal Scene Interface](https://nsi.readthedocs.io/) (ɴsɪ).

This crate defines the renderer-agnostic abstractions used by the rest of
the [`nsi`](https://crates.io/crates/nsi) ecosystem:

- `Type` — `NSIType_t` mirror (`F32`, `F64`, `I32`, `I64`, `Color`,
  `Point`, `Vector`, `Normal`, `MatrixF32`, `MatrixF64`, …) with values
  binary-compatible with the C header.
- `FfiParam` — a `repr(C)` parameter struct, layout-compatible with
  `NSIParam_t`.
- `Flags` — `NSIParam_t.flags` bits.
- `Action` / `StoppingStatus` / `ErrorLevel` — render-control enums.
- `Parameter` trait — what a single typed parameter looks like.
- `Nsi` trait — the renderer surface (`create`, `delete`, `connect`,
  `set_attribute`, …).
- `NodeType` — every standard ɴsɪ node type, including `mesh`, `curves`,
  `nurbs`, `volume`, the cameras, `outputdriver`, `outputlayer`, etc.

This crate is `no_std`-friendly except for the optional `ustr` feature,
which switches the `Name` alias to `ustr::Ustr` for cheap interned
handles.

## License

Licensed under either of

- Apache License, Version 2.0
- MIT license
- zlib License

at your option.
