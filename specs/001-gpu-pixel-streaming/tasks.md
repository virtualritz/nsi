# Tasks: GPU-Resident Pixel Streaming

Dependency-ordered. Every task is small enough for a single commit and names
its evidence gate. Nothing may be ticked before its evidence ran (see
contracts). Tasks T1--T3 are blocked on the `/clarify` pass resolving the
three `[NEEDS CLARIFY]` markers in `spec.md`.

## Setup

- [ ] T1: Resolve `[NEEDS CLARIFY]` markers (GPU backend, bridge-in-v1,
  platform priority) and record answers in `spec.md`. Evidence: updated
  spec with markers removed.
- [ ] T2: Scaffold `crates/nsi-stream` (lib, error types, features:
  `vulkan`, `shm`, `delight-bridge`), wire into workspace. Evidence:
  `cargo build -p nsi-stream`.
- [ ] T3: Freeze the version-1 vocabulary table and channel/shm framing in
  `data-model.md` after clarifications. Evidence: `/analyze` pass shows no
  drift between spec, data-model, and contracts.

## User Story 1 -- DCC Viewport Without CPU Round-Trip (P1)

- [ ] T4: Vocabulary parser + typed errors (rows 1--3 of
  `attribute-vocabulary.md`). Evidence: `vocabulary_forwarding`,
  `vocabulary_version_reject`, `vocabulary_unknown_attr_warns`.
- [ ] T5: Publication ring with acquire/release and latest-wins drop
  (`ring.rs`). Evidence: `acquire_nonblocking`, `ring_exhaustion_drops`,
  `release_reuse_ordering`.
- [ ] T6: In-process GPU transport (allocation, timeline semaphore,
  publication signaling). Evidence: `publication_semaphore_complete` with
  validation layers.
- [ ] T7: `examples/stream_viewport` -- window, blit acquired texture,
  status overlay. Evidence: manual QA path in `quickstart.md`.

## User Story 2 -- Atomic Frame Handoff (P1)

- [ ] T8: Scene-generation tagging anchored to synchronize
  (`Synchronized`/`Restarted` statuses). Evidence:
  `publish_commit_atomic`.
- [ ] T9: `continuous` mode with per-bucket write fencing. Evidence:
  `publish_continuous_no_torn_bucket`.

## User Story 3 -- CPU/Shm Degradation (P2)

- [ ] T10: Transport negotiation (`auto` order, explicit-no-fallback,
  device-UUID check). Evidence: `transport_auto_negotiation`,
  `transport_explicit_no_fallback`, `transport_device_mismatch`.
- [ ] T11: Shm transport with generation-counter parity. Evidence:
  `transport_shm_parity`.
- [ ] T12: `stream.channel` rendezvous (handle export, publication
  messages, client-loss detection). Evidence: `client_loss_behavior`.

## User Story 4 -- AOV Mapping (P2)

- [ ] T13: Per-`outputlayer` targets with declared formats. Evidence:
  `layer_formats` plus multi-AOV manual QA.

## User Story 5 -- Resize Safety (P3)

- [ ] T14: Resize ring reallocation with drain-on-release. Evidence:
  `resize_drain_safety`.
- [ ] T15: Close/drain protocol. Evidence: `close_drain`.

## Bridge (gated on T1 outcome)

- [ ] T16: 3Delight bridge -- ndspy-internal driver uploading buckets into
  publication images. Evidence: `bridge_publication` with `DELIGHT` set;
  viewport-example manual QA against 3Delight.
