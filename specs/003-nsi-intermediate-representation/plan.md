# Plan: ɴsɪ Intermediate Representation

## Status

Implemented. The remaining work is the `Partial` and `Open` rows in
`contracts/`, not new surface.

The rows are not all small. `contracts/resolution.md` carries two that
change the public API -- per-sample motion transforms and per-path
instancing -- and `contracts/stream.md` carries three whose answer has
to be read off a live 3Delight rather than decided here.

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
| Trait seam exists | `cargo test -p nsi-ffi-wrap --lib` | yes |
| Arguments copy losslessly | `cargo test -p nsi-intermediate --lib owned` | yes |
| Classification is exhaustive | `cargo test -p nsi-intermediate --test classifier` | yes |
| Resolution is correct or refuses | `cargo test -p nsi-intermediate --lib resolve` | yes |
| No warnings | `cargo clippy -p nsi-intermediate --all-targets -- -W warnings` | yes |
| **Fidelity against 3Delight** | `cargo test -p nsi-intermediate --test stream_roundtrip` | yes |

The last gate is the meaningful one: it proves the recorder against a
production renderer rather than against itself. Its domain is narrower
than it looks -- see the preconditions in `contracts/stream.md`.

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

## Seam Changes This Required

Both landed in this repository, before the crate was merged in as a
subtree at `c1c1502`:

- `a9abbb0` — `impl ParamValue for Arg`.
- `b092555` — `impl Nsi for Context`, and dropping `where Self: 'call`
  from the `Arg` GAT. See `research.md` D2.

Nothing here depends on an unmerged change elsewhere.
