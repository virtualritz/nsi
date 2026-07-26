# Implementation Plan: GPU-Resident Pixel Streaming

**Branch**: `claude/nsi-api-realtime-2okw3y` | **Date**: `2026-07-26` |
**Spec**: `specs/001-gpu-pixel-streaming/spec.md`.

## Summary

Define and implement a renderer-agnostic pixel streaming contract for ɴsɪ,
carried entirely by `stream.*` attributes on the standard `outputdriver`
node, so rendered pixels can stay GPU-resident. Ship it as a new
`nsi-stream` crate with a driver-owned publication ring, timeline-semaphore
synchronization, negotiated transports (gpu-shared/shm/callback), and a
3Delight bridge so the contract is testable against the only existing ɴsɪ
renderer.

## Technical Context

**Language/Version**: Rust, workspace edition/toolchain as pinned by the
root `Cargo.toml`.

**Primary Dependencies**: `nsi-trait` (only mandatory internal dep).
Feature-gated: Vulkan backend (`ash` or `wgpu` -- open clarification),
shared-memory transport (`rustix`/equivalent), 3Delight bridge
(`nsi-sys`/ndspy internals, feature `delight-bridge`).

**Storage**: none on disk. Wire surfaces: the attribute vocabulary and the
`stream.channel` message framing + shm layout, versioned by
`stream.version` (see `data-model.md`).

**Testing**: `cargo test -p nsi-stream` (never `--release`); bridge tests
require 3Delight and `DELIGHT`; GPU tests run with validation layers where
available.

**Target Platform**: Linux first (Vulkan external memory FD), Windows next
(Win32 handle variant); macOS deferred (Metal shared events/IOSurface).

**Performance Goals**: acquire ≤ 100 µs and non-blocking; zero client-side
CPU pixel copies on the GPU transport; bridge path at most one upload per
bucket.

**Constraints**: no changes to the ɴsɪ call/node set (R1); `nsi-ffi-wrap`
gains no mandatory GPU dependencies (R9); existing `output` module remains
untouched for file/CPU consumers.

## Constitution Check

- Source-of-truth: `.specify/feature.json` →
  `specs/001-gpu-pixel-streaming`.
- Required artifacts: all eight present in this directory.
- Evidence policy: both contract files include
  `Required Evidence Before Marking Complete`; all rows currently `Open`.
- Scope: implementation sessions work one user story or contract row at a
  time. All `[NEEDS CLARIFY]` markers were resolved 2026-07-26 (recorded
  inline in `spec.md`): Vulkan/`ash` driver side, bridge in v1,
  cross-process in v1, Linux → Windows → macOS.

## Project Structure

```text
specs/001-gpu-pixel-streaming/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── checklists/
│   └── requirements.md
└── contracts/
    ├── attribute-vocabulary.md
    └── publication-lifecycle.md

crates/nsi-stream/            (new, this feature)
├── src/
│   ├── lib.rs                vocabulary parser, errors, config
│   ├── ring.rs               publication ring, acquire/release
│   ├── transport/            gpu.rs, shm.rs, callback.rs
│   └── bridge/               3Delight bridge (feature delight-bridge)
└── tests/                    contract-derived tests
examples/stream_viewport/     manual-QA path (winit + GPU blit)
```

## Execution Rules

1. Resolve the three `[NEEDS CLARIFY]` markers before implementation.
2. Work one user story or one contract row at a time.
3. Add or update tests from the contract invariants before ticking rows.
4. Mark rows `Covered` only after the listed evidence commands ran.

## Artifact Checklist

- [x] Active feature pointer is updated (`.specify/feature.json`).
- [x] Required artifact set exists.
- [x] Each contract file has `Required Evidence Before Marking Complete`.
