# Quickstart: Type Names And `hpoint`

```rust,ignore
use nsi_ffi_wrap as nsi;
// A rational NURBS surface: Pw as homogeneous points (w·x, w·y, w·z, w).
ctx.set_attribute("patch", &[nsi::point4_f32_slice!("Pw", &weighted_points)]);
```

Old names keep compiling with a deprecation warning that names the new
one: `nsi::f32!` → `nsi::real_f32!`, `nsi::Color` →
`nsi::argument::Color3F32`, `Type::F32` → `Type::RealF32`.

```sh
cargo test -p nsi-ffi-wrap --features output --test hpoint --test deprecated_names
cargo test -p nsi-parse --test roundtrip
```

`tests/hpoint.rs` needs 3Delight 2.9.210 or later.
