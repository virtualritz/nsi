# Feature Spec: GPU-Resident Pixel Streaming

Replace the ndspy-style CPU bucket callback path with a renderer-agnostic
pixel streaming contract that lets rendered pixels stay GPU-side. ndspy is a
historical RenderMan artifact of 3Delight, not part of ɴsɪ -- the ɴsɪ spec
only standardizes the `outputdriver` node and explicitly leaves the driver
API to the implementation, forwarding any extra attributes to it. This
feature defines that contract for the Rust ecosystem.

## User Stories

### User Story 1: DCC Viewport Without CPU Round-Trip (P1)

As a DCC/viewport integrator, I want progressive render output delivered
into a GPU texture I can sample directly, so that displaying an interactive
render costs no pixel copy through my application's CPU memory.

**Acceptance Criteria**

- Given an interactive render with the stream output driver in-process, when
  the renderer publishes an update, then the client can acquire a texture
  containing the published image without any client-side CPU pixel copy.
- Given a running stream, when the client calls acquire, then the call
  returns without blocking on renderer progress (latest published image or
  "nothing new").
- Given a publication, when the client samples the acquired texture after
  waiting on its fence/semaphore value, then the contents are complete (no
  partially written bucket).

### User Story 2: Atomic Frame Handoff For Engines (P1)

As a game-engine integrator, I want each `RenderControl` `synchronize`
commit to map to identifiable, atomic publications, so that no displayed
frame mixes pixels from two scene states.

**Acceptance Criteria**

- Given publish mode `commit`, when edits are synchronized, then every
  subsequently acquired image is tagged with the scene generation it was
  rendered from, and contains samples from exactly one generation.
- Given publish mode `continuous`, when buckets accumulate, then acquired
  images may show partial refinement of one generation but never contain a
  torn (partially written) bucket.
- Given the client holds all ring images, when the renderer wants to
  publish, then the renderer does not stall; the publication is dropped,
  latest-wins, and a drop counter is incremented.

### User Story 3: Degradation To CPU Transport (P2)

As a client of an out-of-process renderer, I want the same client code to
fall back to a shared-memory/CPU transport, so that GPU residency is an
optimization, not a protocol fork.

**Acceptance Criteria**

- Given `stream.transport` `"auto"` and no viable GPU path, when the driver
  opens, then pixels arrive via the shared-memory transport and the
  client-facing acquire API is unchanged.
- Given an explicitly requested transport that is not viable, when the
  driver opens, then opening fails with a typed error -- no silent fallback.

### User Story 4: AOV Mapping (P2)

As a client, I want each connected `outputlayer` addressable as its own
texture (or array layer), so that multi-AOV pipelines (beauty, ID picking,
normals) work without demultiplexing on the CPU.

**Acceptance Criteria**

- Given N output layers connected to one stream driver, when acquiring a
  publication, then each layer is individually addressable with its declared
  format and `variablename`.

### User Story 5: Resolution Change Safety (P3)

As a client, I want a `screen` resolution edit followed by `synchronize` to
resize the stream safely, so that interactive quality scaling (drop
resolution while dragging) never crashes or leaks.

**Acceptance Criteria**

- Given a resolution edit and synchronize, when the next publication occurs,
  then it uses the new extent.
- Given the client still holds a pre-resize acquisition, when the resize
  happens, then that acquisition stays valid until released; images are
  reclaimed only after release (no use-after-free).

## Non-Goals

- A realtime renderer backend (`Nsi` trait implementation) -- separate
  feature (see `specs/README.md` coverage order).
- Input-side GPU residency (geometry/attribute buffers via `Reference`-style
  handoff) -- separate feature.
- Frame pacing/deadline contracts and temporal-reuse semantics across
  `synchronize` -- renderer-side, separate feature.
- Tone mapping or display transforms -- the stream stays linear,
  scene-referred; display transform is the client's post stack.
- Cross-host (network) transport of GPU surfaces. The serializable-stream
  use of ɴsɪ degrades to the CPU transport; that is by design.
- ndspy as a public API. It may appear only inside a 3Delight bridge
  implementation detail.

## Requirements

- R1: The contract is expressed entirely as attributes on the standard
  `outputdriver` node (`drivername "nsi-stream"` plus a `stream.*`
  vocabulary). No additions to the ɴsɪ call set or node set.
- R2: GPU resources cross the client/renderer boundary only as exportable,
  named OS handles (external memory, timeline semaphores) -- never as raw
  pointers. Raw pointers/closures are permitted only for the in-process
  callback transport, matching existing `callback!`/`reference!` precedent.
- R3: The driver owns a ring of at least 2 publication images. Clients
  acquire and release; the driver never writes an image a client holds.
- R4: GPU synchronization uses one timeline semaphore (or platform
  equivalent); each publication carries the semaphore value to wait on. The
  CPU transport uses a generation counter/seqlock equivalent.
- R5: Publish modes: `commit` (atomic publication per applied synchronize)
  and `continuous` (progressive accumulation visible between commits).
- R6: Minimum formats: RGBA f16 and f32; per-layer format declared per
  `outputlayer`. Pixel data is linear and scene-referred.
- R7: Version negotiation: `stream.version` is mandatory; an unsupported
  version fails open() with a typed error.
- R8: Failure is loud (typed errors) for unviable transports, bad handles,
  and version mismatch. Fallback happens only under `"auto"`.
- R9: Rust surface lives in a new `nsi-stream` crate depending on
  `nsi-trait` only; GPU backends are feature-gated so `nsi-ffi-wrap` and
  clients without GPU needs take no graphics dependencies.
- R10: A 3Delight bridge implements the contract against today's only ɴsɪ
  renderer by uploading its display-driver buckets into the publication
  images, preserving publication semantics. [NEEDS CLARIFY: is the bridge
  v1 scope, or does v1 target only a future Rust backend? Bridge-in-v1
  makes the contract testable against a real renderer now.]

## Open Clarifications

- [NEEDS CLARIFY: first GPU backend -- raw Vulkan via `ash`, `wgpu`
  (external-memory import support varies per backend), or Vulkan-first with
  a `wgpu` interop layer?]
- [NEEDS CLARIFY: is cross-process/same-GPU (handle export over a local
  socket) in v1, or is v1 in-process only with the vocabulary designed so
  cross-process adds later without breakage?]
- [NEEDS CLARIFY: platform priority -- Linux/Windows (Vulkan external
  memory) first, macOS (Metal shared events/IOSurface) later?]

## Risks

- `wgpu` cannot import external memory on all backends. Mitigation: keep the
  transport pluggable; treat backend choice as a clarification, not an
  assumption.
- Handle lifetime across the FFI boundary (renderer thread writing after
  client teardown, or vice versa). Mitigation: explicit lifecycle state
  machine (`data-model.md`), close protocol that waits on the final
  semaphore value, channel-close detection.
- 3Delight's display-driver thread model delivers buckets from many threads.
  Mitigation: the bridge serializes uploads per image; covered by bridge
  contract rows.
- Semaphore deadlock. Mitigation: timeline (not binary) semaphores;
  publication never waits on client acquisition (latest-wins, R3/US2).
- Vocabulary drift between renderers. Mitigation: contract-first -- the
  vocabulary table in `data-model.md` is the wire format of record and is
  versioned (R7).
