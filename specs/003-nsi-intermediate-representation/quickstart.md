# Quickstart: ɴsɪ Intermediate Representation

## Build

```bash
cd ~/code/crates/nsi-mitsuba
cargo build --workspace
```

`nsi-intermediate` depends on `nsi` as a git dependency, pinned by
`Cargo.lock`. Under the `[patch]` override in `README.md` it instead
tracks a local working tree, uncommitted changes included, and a red
test here may then originate there.

## Verification Commands

```bash
# Everything.
cargo test --workspace

# Per contract.
cargo test -p nsi-intermediate --lib owned         # recording.md
cargo test -p nsi-intermediate --test classifier   # classification.md
cargo test -p nsi-intermediate --lib resolve       # resolution.md
cargo test -p nsi-intermediate --test stream_roundtrip  # stream.md
```

Expected at the time of writing: 43 passing, 0 failing.

## Manual QA Path

`stream_roundtrip` needs a working 3Delight; it creates a real
`apistream` context. Confirm with:

```bash
cargo test -p nsi-intermediate --test stream_roundtrip -- --nocapture 2>&1 \
  | rg '3Delight'
```

A banner such as `# 3Delight 2.9.207 linux64 ... "Re-Animator"` proves
the reference side really ran. **Without 3Delight this test cannot
pass, and its absence is not a licence to mark `stream.md` `Covered`.**

## Regenerating The Stream Oracle

There is no checked-in fixture: the reference is produced live by
3Delight in the same test run, from the same `build` function. If
3Delight's format changes, the test fails and `stream.rs` is corrected —
never the expectation.
