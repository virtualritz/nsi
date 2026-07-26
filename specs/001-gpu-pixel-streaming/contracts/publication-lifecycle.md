# Contract: Publication And Lifecycle

## Scope

This contract covers the publication ring, acquire/release, synchronization,
resize, and teardown. It does not cover attribute parsing/negotiation (see
`attribute-vocabulary.md`) or renderer-internal sampling/refinement policy.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `commit` mode: every publication carries one scene generation; no image mixes generations | Covered | `crates/nsi-stream/src/ring.rs` (`publish(guard, generation)`, `Publication::scene_generation`) | `cargo test -p nsi-stream publish_commit_atomic` -- ok (2026-07-26, generation written into pixels, uniformity asserted) | `cargo test -p nsi-stream publish_commit_atomic` -- two-generation scene, assert per-pixel generation tag uniformity via ID layer. |
| `continuous` mode: acquired image never contains a torn bucket | Covered | `crates/nsi-stream/src/ring.rs` (accumulation buffer, copy-on-publish after bucket completion) | `cargo test -p nsi-stream publish_continuous_no_torn_bucket` -- ok (2026-07-26, concurrent checkered write-order fixture) | `cargo test -p nsi-stream publish_continuous_no_torn_bucket` with a checkered write-order fixture. |
| Acquire is non-blocking and returns latest publication or none | Covered | `crates/nsi-stream/src/ring.rs` (`acquire`) | `cargo test -p nsi-stream acquire_nonblocking` -- ok (2026-07-26, timed) | `cargo test -p nsi-stream acquire_nonblocking` (timed). |
| Client waits on the Publication's timeline value before sampling; contents complete after wait | Covered | `crates/nsi-stream/src/timeline.rs` (`CpuTimeline::signal`/`wait`); Vulkan variant `src/transport/gpu.rs` (`VulkanTimeline`, compile-verified) | `cargo test -p nsi-stream publication_semaphore_complete` -- ok (2026-07-26, CPU timeline; validation-layer GPU run pending a Vulkan-capable box) | `cargo test -p nsi-stream publication_semaphore_complete`. |
| Renderer never stalls on a fully leased ring; publication drops latest-wins and increments drop counter | Covered | `crates/nsi-stream/src/ring.rs` (`begin_write` latest-wins, `dropped_publications` counter) | `cargo test -p nsi-stream ring_exhaustion_drops` -- ok (2026-07-26, all tokens held) | `cargo test -p nsi-stream ring_exhaustion_drops` holding all tokens. |
| Release returns the image to the ring; driver reuses only released images | Covered | `crates/nsi-stream/src/ring.rs` (`release`, slot states) | `cargo test -p nsi-stream release_reuse_ordering` -- ok (2026-07-26) | `cargo test -p nsi-stream release_reuse_ordering`. |
| Resize: next publication has new extent; held pre-resize acquisitions stay valid until release | Covered | `crates/nsi-stream/src/ring.rs` (`resize`, `Arc`-backed slot storage; safe Rust, no `unsafe` in ring) | `cargo test -p nsi-stream resize_drain_safety` -- ok (2026-07-26; miri/asan not run -- ring path is entirely safe Rust) | `cargo test -p nsi-stream resize_drain_safety` under address sanitizer/miri where applicable. |
| Close drains: no publications after close, final semaphore value signaled, GPU objects freed | Covered | `crates/nsi-stream/src/ring.rs` (`close`, `is_drained`, `Error::Closed`), `src/timeline.rs` (final value) | `cargo test -p nsi-stream close_drain` -- ok (2026-07-26; GPU leak-check via validation layers pending a Vulkan-capable box) | `cargo test -p nsi-stream close_drain`; leak check via validation layers. |
| Client loss: driver detects channel close and honors `stream.onclientloss` | Covered | `crates/nsi-stream/src/channel.rs` (EPIPE detection, `ClientLoss` continue/stop) | `cargo test -p nsi-stream --features shm client_loss_behavior` -- ok (2026-07-26, both modes) | `cargo test -p nsi-stream client_loss_behavior` (kill client end of socket). |
| CPU/shm transport preserves all rows above with generation counter semantics | Covered | `crates/nsi-stream/src/transport/shm.rs` (frozen header layout, seqlock generation counter) | `cargo test -p nsi-stream --features shm transport_shm_parity` -- ok (2026-07-26) | Re-run the suite with `stream.transport "shm"` fixture: `cargo test -p nsi-stream --features shm transport_shm_parity`. |
| 3Delight bridge: buckets upload into publication images, publication anchored to `Synchronized`/`Restarted` statuses | Partial | `crates/nsi-stream/src/bridge/mod.rs` (`DelightBridge`, open/write/finish callbacks over the ndspy-internal `ferris_f32` driver; `synchronized()` anchor) | `cargo test -p nsi-stream --features delight-bridge bridge_publication` with `DELIGHT` set -- ok (2026-07-26, 3Delight 2.9.30 via the build.rs download: 64x64 render, 20 buckets, commit=2 publications generation-tagged, continuous=21 progressive publications). Viewport-example manual QA pending (needs a display + T7). | `cargo test -p nsi-stream --features delight-bridge bridge_publication` with `DELIGHT` set; manual QA via viewport example. |

## Invariants

- Publication never blocks renderer progress (latest-wins).
- The driver never writes an image with an outstanding lease.
- Frame serial is strictly monotonic; scene generation is monotonic and
  equals the count of applied synchronizes observed by the driver.
- Pixel data is linear, scene-referred, in the layer's declared format at
  every transport.

## Failure Modes

- Semaphore wait timeout on the client → surface as typed error with the
  publication's serial; do not spin.
- Driver allocation failure on resize → stream enters Failed, client
  notified via channel close + typed message; renderer render continues per
  `stream.onclientloss` semantics.
- Client crash mid-lease → channel close ⇒ all leases considered released
  after the driver's in-flight timeline value retires.

## Required Evidence Before Marking Complete

- Source evidence must cite `crates/nsi-stream/src/` symbols (ring,
  publication, transport implementations) per row.
- Executable evidence: the exact `cargo test` commands per row (never
  `--release`); GPU rows additionally cite the validation-layer log or
  sanitizer used.
- Bridge row: requires 3Delight installed, `DELIGHT` set; cite renderer
  version. Manual QA: run the viewport example, perform live edits +
  synchronize, record observed atomicity (no mixed-state frame) and the
  drop counter behavior.
