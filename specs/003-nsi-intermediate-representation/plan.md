# Plan: ɴsɪ Intermediate Representation

## Status

Implemented. The remaining work is the `Partial` and `Open` rows in
`contracts/`, not new surface.

## Approach

Three layers, each testable without a renderer:

1. **Capture** — `OwnedArg` copies a borrowed `Arg`'s payload, mirroring
   the ɴsɪ C API's own copy contract.
2. **Structure** — `Scene` holds nodes, attributes and classified edges
   in insertion order.
3. **Resolution** — pure functions turn graph semantics into flat facts.

`Recorder` implements `Nsi` over the first two. `write_stream` replays
the second for verification.

## Gates

| Gate | Command | Met |
| --- | --- | --- |
| Trait seam exists upstream | `cargo test -p nsi-ffi-wrap --lib` | yes |
| Arguments copy losslessly | `cargo test -p nsi-intermediate --lib owned` | yes |
| Classification is exhaustive | `cargo test -p nsi-intermediate --test classifier` | yes |
| Resolution is correct | `cargo test -p nsi-intermediate --lib resolve` | yes |
| **Fidelity against 3Delight** | `cargo test -p nsi-intermediate --test stream_roundtrip` | yes |

The last gate is the meaningful one: it proves the recorder against a
production renderer rather than against itself.

## Artifact Checklist

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/recording.md`
- [x] `contracts/classification.md`
- [x] `contracts/resolution.md`
- [x] `contracts/stream.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`

## Upstream Changes This Required

Two commits in `virtualritz/nsi`, both pushed:

- `a9abbb0` — `impl ParamValue for Arg`.
- `b092555` — `impl Nsi for Context`, and dropping `where Self: 'call`
  from the `Arg` GAT. See `research.md` D2.
