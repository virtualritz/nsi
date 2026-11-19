# nsi-argument

Typed arguments for [ɴsɪ](https://nsi.readthedocs.io/) calls: `Arg`, its
value types and the `real_f32!`/`point3_f32_slice!`/... macros.

The crate depends on `nsi-trait` only, not on `nsi-sys`. An `Arg` converts
to `nsi_trait::FfiParam`, which has the `NSIParam` layout of `nsi.h`.
`nsi-ffi-wrap` does the conversion at the FFI boundary and re-exports this
crate under its old paths, so `nsi::Arg` and `nsi::real_f32!` keep working.
