# Quickstart: ɴsɪ Intermediate Representation

## Build

```bash
cd ~/code/crates/nsi
cargo build --workspace
```

`nsi-intermediate` is a workspace member and depends on its siblings
`nsi-trait` and `nsi-ffi-wrap` by path. There is no git dependency and
no `[patch]`: a red test here originates here.

Building the crate needs no renderer. Only the stream gate does.

## Verification Commands

```bash
# Everything.
cargo test -p nsi-intermediate

# Per contract.
cargo test -p nsi-intermediate --lib owned         # recording.md
cargo test -p nsi-intermediate --lib recorder      # recording.md
cargo test -p nsi-intermediate --lib scene         # recording.md
cargo test -p nsi-intermediate --test classifier   # classification.md
cargo test -p nsi-intermediate --lib resolve       # resolution.md
cargo test -p nsi-intermediate --test stream_roundtrip  # stream.md
```

The suite must also be warning-free:

```bash
cargo clippy -p nsi-intermediate --all-targets -- -W warnings
```

## Manual QA Path

`stream_roundtrip` needs a working 3Delight; it creates a real
`apistream` context. `DELIGHT` must point at the install, and the
licence server must be reachable. Confirm the reference side really ran:

```bash
cargo test -p nsi-intermediate --test stream_roundtrip -- --nocapture 2>&1 \
  | rg '3Delight'
```

A banner such as `# 3Delight 2.9.207 linux64 ... "Re-Animator"` proves
it. **Without 3Delight this test cannot pass, and its absence is not a
licence to mark `stream.md` `Covered`.**

## Regenerating The Stream Oracle

There is no checked-in fixture: the reference is produced live by
3Delight in the same test run, from the same `build` function. If
3Delight's format changes, the test fails and `stream.rs` is corrected —
never the expectation.

## Extending The Stream Fixture

`build` is constrained by the preconditions in `contracts/stream.md`,
and violating one produces a failure that looks like an emitter bug but
is not. In particular: set a node's static attributes **before** its
motion samples, and do not `create` a handle twice. Both were hit while
adding the matrix and edge-class cases.
