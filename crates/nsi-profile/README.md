# `nsi-profile`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-profile/badge.svg)](https://docs.rs/nsi-profile)
[![Crate](https://img.shields.io/crates/v/nsi-profile.svg)](https://crates.io/crates/nsi-profile)

The ɴsɪ shading profile -- a fixed, versioned closure and node vocabulary
that a realtime backend can evaluate by *translating* shader networks to
portable GPU code, instead of executing arbitrary ᴏsʟ.

A profile node is addressed from a standard `shader` node:

```text
shaderfilename = "nsi-profile:diffuse_bsdf@1"
```

No new ɴsɪ node types, no new API calls. Every profile node ships two
implementations of record: an ᴏsʟ 1.12 reference in `osl/` -- what an
offline renderer executes -- and a GLSL 4.60 function in `glsl/` -- the GPU
source of record. Compiling the assembled module to SPIR-V is a backend step
behind the `GpuEmitter` trait, so this crate depends on no shader compiler.

The crate provides:

- `registry` -- profile versions, node and closure tables, scheme resolution.
- `validate` -- the loud validator: node handle, construct, version
  consulted, no silent stripping.
- `translate` -- network to GLSL module plus a deterministic, `std430`
  ParameterBlock layout.
- `edit` -- parameter update vs. re-translation classification.
- `emit` -- the `GpuEmitter` seam and a GLSL passthrough emitter.

See the crate documentation for the v1 node and closure tables, the
versioning policy and the normative exclusion list.

## Testing

```bash
cargo test -p nsi-profile
```

Never use `--release` (see `AGENTS.md`).

## License

MIT OR Apache-2.0 OR Zlib.
