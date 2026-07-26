# Tasks: GPU-Resident Pixel Streaming

Dependency-ordered. Every task is small enough for a single commit and names
its evidence gate. Nothing may be ticked before its evidence ran (see
contracts).

## Setup

- [x] T1: Resolve `[NEEDS CLARIFY]` markers (GPU backend, bridge-in-v1,
  cross-process scope, platform priority) and record answers in `spec.md`.
  Evidence: `spec.md` contains no `[NEEDS CLARIFY]` markers; four RESOLVED
  entries dated 2026-07-26.
- [x] T2: Scaffold `crates/nsi-stream` (lib, error types, features:
  `vulkan`, `shm`; `delight-bridge` arrives with T16), wire into
  workspace. Evidence: `cargo build -p nsi-stream` -- ok (2026-07-26);
  also `--features shm`, `--features vulkan`, `--all-features`.
- [x] T3: Freeze the version-1 vocabulary table and channel/shm framing in
  `data-model.md` after clarifications. Evidence: `/analyze` pass
  (2026-07-26) shows no drift between spec, data-model, and contracts;
  the channel framing and shm layout are version-1-frozen in module
  rustdoc (`crates/nsi-stream/src/channel.rs`, `src/transport/shm.rs`)
  per data-model's "documented with the implementation" rule. (The pass
  found stale status text in quickstart/checklists only; fixed same
  session.)

## User Story 1 -- DCC Viewport Without CPU Round-Trip (P1)

- [x] T4: Vocabulary parser + typed errors (rows 1--3 of
  `attribute-vocabulary.md`). Evidence: `cargo test -p nsi-stream
  vocabulary_forwarding` / `vocabulary_version_reject` /
  `vocabulary_unknown_attr_warns` -- all ok (2026-07-26).
- [x] T5: Publication ring with acquire/release and latest-wins drop
  (`ring.rs`). Evidence: `cargo test -p nsi-stream acquire_nonblocking` /
  `ring_exhaustion_drops` / `release_reuse_ordering` -- all ok
  (2026-07-26).
- [ ] T6: In-process GPU transport (allocation, timeline semaphore,
  publication signaling). Evidence: `publication_semaphore_complete` with
  validation layers. Progress (2026-07-26): CPU-timeline equivalent
  covered (`publication_semaphore_complete` ok); Vulkan module
  (`transport/gpu.rs`: exportable ring images, `VulkanTimeline`)
  compile-verified via `cargo build -p nsi-stream --features vulkan`;
  validation-layer run needs a Vulkan-capable box.
- [ ] T7: `examples/stream_viewport` -- window, blit acquired texture,
  status overlay. Evidence: manual QA path in `quickstart.md`.

## User Story 2 -- Atomic Frame Handoff (P1)

- [x] T8: Scene-generation tagging anchored to synchronize
  (`Synchronized`/`Restarted` statuses). Evidence: `cargo test -p
  nsi-stream publish_commit_atomic` -- ok (2026-07-26).
- [x] T9: `continuous` mode with per-bucket write fencing. Evidence:
  `cargo test -p nsi-stream publish_continuous_no_torn_bucket` -- ok
  (2026-07-26).

## User Story 3 -- CPU/Shm Degradation (promoted to v1, 2026-07-26)

- [x] T10: Transport negotiation (`auto` order, explicit-no-fallback,
  device-UUID check). Evidence: `cargo test -p nsi-stream
  transport_auto_negotiation` / `transport_explicit_no_fallback` /
  `transport_device_mismatch` -- all ok (2026-07-26).
- [x] T11: Shm transport with generation-counter parity. Evidence:
  `cargo test -p nsi-stream --features shm transport_shm_parity` -- ok
  (2026-07-26).
- [x] T12: `stream.channel` rendezvous (handle export, publication
  messages, client-loss detection). Evidence: `cargo test -p nsi-stream
  --features shm client_loss_behavior` -- ok (2026-07-26, both
  `stream.onclientloss` modes).

## User Story 4 -- AOV Mapping (P2)

- [ ] T13: Per-`outputlayer` targets with declared formats. Evidence:
  `layer_formats` plus multi-AOV manual QA. Progress (2026-07-26):
  `cargo test -p nsi-stream layer_formats` ok; multi-AOV manual QA
  pending T7 example + T16 bridge (contract row marked Partial).

## User Story 5 -- Resize Safety (P3)

- [x] T14: Resize ring reallocation with drain-on-release. Evidence:
  `cargo test -p nsi-stream resize_drain_safety` -- ok (2026-07-26).
- [x] T15: Close/drain protocol. Evidence: `cargo test -p nsi-stream
  close_drain` -- ok (2026-07-26).

## Bridge (v1 scope per T1 resolution)

- [ ] T16: 3Delight bridge -- ndspy-internal driver uploading buckets into
  publication images. Evidence: `bridge_publication` with `DELIGHT` set;
  viewport-example manual QA against 3Delight. Progress (2026-07-26):
  bridge implemented (`crates/nsi-stream/src/bridge/mod.rs`, feature
  `delight-bridge` over `nsi-ffi-wrap/output`'s ndspy-internal driver);
  `cargo test -p nsi-stream --features delight-bridge bridge_publication`
  with `DELIGHT` set -- ok against 3Delight 2.9.30. Remaining for the
  tick: viewport-example manual QA (blocked on T7 + a display). Note:
  the staged 2.9.30 download is the bare library (no `osl/` shaders), so
  the fixture scene is an unshaded coverage quad; re-run with a full
  3Delight install for a shaded fixture.
