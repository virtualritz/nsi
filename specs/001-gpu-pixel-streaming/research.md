# Research: GPU-Resident Pixel Streaming

Decisions, rejected alternatives, and references. Investigation summary from
the session that produced this spec: the ɴsɪ interface is already
interactive by design (buffered edits applied atomically at
`RenderControl(synchronize)`); the pixel return path is the most incidental
of its realtime gaps because the driver API is explicitly outside the spec.

## D1: Attribute-Only Contract On The `outputdriver` Node

**Decision.** Express the whole contract as `drivername "nsi-stream"` plus
`stream.*` attributes on the standard `outputdriver` node.

**Why.** The ɴsɪ spec states the destination "could be a file ... frame
buffer or a memory address", that the driver API "is implementation specific
and is not covered by this documentation", and that "any extra attributes
are also forwarded to the output driver which may interpret them however it
wishes". A conforming renderer that does not know the vocabulary simply has
no driver named `nsi-stream` and errors accordingly.

**Rejected.** New ɴsɪ calls or node types -- breaks the twelve-call surface
and portability for zero gain. Extending ndspy -- ndspy is 3Delight's
RenderMan-era artifact, pull-based, CPU-bucket-shaped; not a contract worth
inheriting.

## D2: Driver-Owned Publication Ring ("Reverse Swapchain")

**Decision.** The driver allocates and owns 2--3 publication images; the
client acquires the latest published one and releases it. Publication is
latest-wins and never blocks the renderer.

**Why.** Renderer-side ownership makes resize a driver-internal reallocation
(old images drain via release), keeps write-safety trivial (never write a
held image), and mirrors how progressive accumulation already works
renderer-side.

**Rejected.** Client-allocated surfaces the renderer imports -- complicates
resize and forces the client to reason about renderer write timing. A
single shared surface -- guarantees tearing or forces renderer stalls.

## D3: OS Handles, Not Pointers

**Decision.** GPU objects cross the boundary as exportable handles (POSIX FD
/Win32 handle for memory, timeline semaphore handles) plus a device UUID for
adapter matching. In-process transport may shortcut with closures, matching
the existing `callback!` and `reference!` precedent in `nsi-ffi-wrap`.

**Why.** Handles survive process boundaries on the same host and GPU; raw
pointers do not. Pointer attributes also destroy the serializability design
goal wherever they appear, so they are confined to the transport that is
unserializable anyway.

**Rejected.** Raw pointer attributes as the primary contract.

## D4: Publication Anchored To `synchronize`

**Decision.** `commit` mode publishes atomically per applied synchronize,
tagged with a scene generation; `continuous` mode additionally publishes
during refinement. Default: `commit` for engines, `continuous` for DCC IPR.

**Why.** `RenderControl` is the API's single transaction point -- edits are
buffered and applied atomically at synchronize (statuses `Synchronized`/
`Restarted`). Anchoring publication to that boundary gives an engine a
frame-state/image correlation for free.

**Rejected.** Renderer-cadence-only pushing -- the client cannot correlate
an image with the scene state that produced it.

## D5: New Crate `nsi-stream`

**Decision.** The contract types, vocabulary parser, transports, and the
3Delight bridge (feature `delight-bridge`) live in a new crate depending
only on `nsi-trait`. GPU backends are feature-gated.

**Why.** `nsi-ffi-wrap/src/output/` is bound to 3Delight's ndspy driver and
CPU bucket delivery; it stays for file output and CPU consumers. The stream
contract must be implementable by any renderer, including a future
`Nsi`-trait backend exposed via `FfiApiAdapter` -- which would implement the
sink natively and never touch ndspy.

**Rejected.** Growing the existing `output` module -- entangles a
renderer-agnostic contract with an ndspy-specific one.

## D6: Loud Failure, Negotiated Fallback

**Decision.** `stream.transport "auto"` negotiates gpu-shared, then shared
memory, then callback. Any explicitly named transport that cannot open fails
with a typed error.

**Why.** Blueprints persistence rule: silent fallback on required
identifiers is forbidden. Auto-negotiation is opt-in, so a client that
requires GPU residency finds out at open(), not from a profiler.

## References

- ɴsɪ spec, `outputdriver` node: <https://nsi.readthedocs.io/en/latest/nodes/outputdriver.html>
- ɴsɪ spec, rendering/`RenderControl`: <https://nsi.readthedocs.io/en/latest/c-api.html>
- ɴsɪ design goals: <https://nsi.readthedocs.io/en/latest/background.html>
- Existing CPU driver wrapper: `crates/nsi-ffi-wrap/src/output/mod.rs`
- Zero-copy precedent: `crates/nsi-ffi-wrap/src/argument.rs` (`Reference`,
  `ReferenceSlice`)
- Backend-independence machinery: `crates/nsi-trait/src/nsi_trait.rs`,
  `crates/nsi-ffi-wrap/src/c_api.rs` (`FfiApiAdapter`)
