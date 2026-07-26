# Quickstart: GPU-Resident Pixel Streaming

**Current status: core implemented (2026-07-26).** `crates/nsi-stream`
exists; the vocabulary/ring/negotiation/shm rows are covered by the test
commands below (see the contract matrices for per-row evidence). Still
pending: GPU validation-layer runs (needs a Vulkan-capable box), the
viewport example (T7), and the 3Delight bridge (T16) -- the
`delight-bridge` feature is not declared until T16 lands. Nothing here is
evidence until it has actually been run.

## Build And Test

```bash
# Contract-derived tests (no GPU required for vocabulary/ring rows).
cargo test -p nsi-stream

# GPU transport rows (Vulkan validation layers recommended).
cargo test -p nsi-stream --features vulkan

# Shm transport parity.
cargo test -p nsi-stream --features shm

# 3Delight bridge rows (requires 3Delight installed and DELIGHT set).
cargo test -p nsi-stream --features delight-bridge
```

Never use `--release` for these (repo rule, see `AGENTS.md`).

## Manual QA Path

1. Run the viewport example against a renderer (bridge or native backend):

   ```bash
   cargo run --example stream_viewport --features vulkan,delight-bridge
   ```

2. Confirm the window shows the progressive render and the overlay reports
   transport `gpu`, publish mode, frame serial, scene generation, and drop
   counter.
3. Perform live edits (the example binds keys to transform/light edits that
   call `synchronize`). Confirm in `commit` mode that no displayed frame
   mixes old and new state, and the generation counter increments per
   synchronize.
4. Resize the window (triggers a `screen` resolution edit + synchronize).
   Confirm publication continues at the new extent with no crash.
5. Kill the viewer while rendering; confirm renderer behavior matches
   `stream.onclientloss` and no validation-layer errors are logged.

Record renderer name/version, OS, GPU, and observed results when citing this
path as contract evidence.
