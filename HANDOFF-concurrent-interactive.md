# Handoff: concurrent interactive renders + full progressive support (nsi-rs / FERRIS DSPY driver)

## Goal

Make the in-process **FERRIS** output driver support **multiple concurrent
interactive renders** (one `nsi::Context` per render, all live at once) and
**full progressive rendering**. Today a single interactive render works (e.g. a
viewport overlay), but a _second_ simultaneous interactive context:

1. **produces no pixels** — its `image_write` callback never fires, and
2. **deadlocks on teardown** — `render_control(Stop)` + `Wait` never returns, so
   joining the render thread hangs the host app on exit.

A **batch** render (`Start` + `Wait`, no `interactive`/`progressive` flags) of
the exact same scene works in every context and tears down cleanly.

## Why this matters (the consumer)

Akatela (`/home/ritz/code/crates/akatela`, feature `nsi-render`) wants a live,
OIDN-denoised shader-ball preview per shader/material node in its graph, plus a
viewport preview — i.e. several interactive renders at once. Because concurrent
interactive contexts fail, akatela currently falls back to **one-shot batch
renders** for the node swatches (noisy, no live refine) and reserves the single
working interactive context for the viewport. Fixing this unblocks
interactive + denoised previews everywhere.

## Evidence (from akatela, instrumented)

Two shader nodes → two swatch contexts `NSI[1]`, `NSI[2]` (the viewport is
`NSI[0]`). Per-context diagnostics were added (first beauty bucket received;
drop join start/end). Observed:

```
swatch: node 4 -> NSI[1] created (interactive)
NSI: Starting render (interactive + progressive)      # render_control(Start,{interactive,progressive}) issued
swatch: node 5 -> NSI[2] created (interactive)
NSI: Starting render (interactive + progressive)
... ~40s of use, NO "NSI[1]/NSI[2]: first beauty bucket received" ever ...
NSI[2]: drop joining render thread (Stop+Wait)…        # <-- hangs here forever, app must be killed
```

So: `Start` is issued for the secondary contexts, but the write callback never
fires (no pixels) and `Stop`/`Wait` never returns. The single interactive
viewport context (`NSI[0]`) works fine in isolation, and the user confirms
interactive rendering itself is healthy (the viewport overlay is interactive and
denoised).

## What is already correct (do NOT re-do)

- **`Context::new` makes independent contexts** — it calls `NSIBegin` per call
  (see `crates/nsi-ffi-wrap/src/linked/mod.rs` / `context.rs`). Not a global
  context.
- **Callbacks route per-display, not by name** — `image_open` stashes a
  `DisplayData<T>` via `Box::into_raw` into the per-image handle; `image_write` /
  `image_close` recover it from that handle. No global registry keyed by
  `imagefilename` or driver handle. (So display-name collisions are not the
  cause; the akatela side shares `imagefilename "akatela_nsi_overlay"` across
  contexts and that is _not_ the problem.)
- **`DspyImageQuery` (`image_query`) already answers every interactive-relevant
  query**, matching 3Delight's reference driver `dlViewport.cpp` (see below):
  - `Progressive` → `acceptProgressive = 1`
  - `Thread` → `multithread = 1`
  - `Cooked` → `cooked = 1`
  - `Overwrite` → `overwrite = 1`
  - `Stop` → `Error::None` (continue)
  - `RenderProgress` → installs `image_progress`
  - everything else → `Error::Unsupported`

  File: `crates/nsi-ffi-wrap/src/output/mod.rs`, `pub(crate) extern "C" fn
image_query(...)`.

## 3Delight support's lead

> "Is it a custom display driver? If so, try both answers to the PkThreadQuery
> (ie: 0 and 1), in case this changes something. I don't think it should, but
> you never know... Look in `3dfm/src/dlViewport.cpp` for an example of a
> positive answer."

Reference driver (a known-good interactive viewport DSPY driver):
`/home/ritz/code/3DFM/src/dlViewport.cpp` — see `ViewportDspyImageQuery`
(handles `PkSizeQuery`, `PkOverwriteQuery`, `PkProgressiveQuery`,
`PkCookedQuery`, `PkStopQuery`, `PkThreadQuery` with `multithread = 1`), and the
driver table at the bottom (`PtDspyDriverFunctionTable`: `pOpen`, `pWrite`,
`pClose`, `pQuery`, and note any function table fields the FERRIS driver may be
leaving null — e.g. flags / `pActiveRegion` / delayed-read entries).

Our `image_query` already returns `multithread = 1`. Worth a quick experiment to
set it to `0` (per the lead), but the support team and the code both suggest the
query is not the root cause. **Treat the queries as a first 5-minute experiment,
then move to the driver's open/write/close + the function-table registration.**

## Where to work

- nsi-rs workspace root: `/home/ritz/code/crates/nsi/`
  (git remote `https://github.com/virtualritz/nsi.git`).
- DSPY driver: `crates/nsi-ffi-wrap/src/output/mod.rs` — functions
  `image_open`, `image_write<T>`, `image_close<T>`, `image_query`,
  `image_progress`, and the `PtDspyDriverFunctionTable` registration
  (search `register_output_drivers` / where `pOpen`/`pWrite`/`pClose`/`pQuery`
  are wired). Also check `crates/nsi-ffi-wrap/src/{linked,dynamic}/mod.rs` for
  how the driver is registered with 3Delight and whether registration is
  per-process (once) vs per-context.

## Hypotheses to investigate (in priority order)

1. **Driver function-table completeness.** Compare the FERRIS
   `PtDspyDriverFunctionTable` against `dlViewport.cpp`'s table field by field.
   A missing/incorrect flag (e.g. the driver's `flags`, or not advertising
   itself as supporting asynchronous/overlapped images) can make 3Delight
   refuse to drive a second concurrent interactive display, or serialize on a
   global the second render waits on forever (→ the Stop/Wait deadlock + no
   buckets).
2. **Per-process driver registration vs per-context.** If
   `register_output_drivers` registers the driver once globally but some state
   (statics, a single progress fn ptr, a shared image handle) is effectively
   single-render, the 2nd context can't open/write. Audit `image_query`'s
   `RenderProgress` arm (`*func_ptr = Some(image_progress)`) and `image_progress`
   for shared/global state.
3. **`image_open` / `DisplayData` for concurrent images.** Confirm two live
   `DisplayData<T>` instances (two open displays) don't share anything. Verify
   the boxed write/finish closures are owned per-display and not aliased.
4. **Upstream 3Delight limit.** If the driver is provably correct, build the
   minimal C-level repro (below) and take it to 3Delight: "two simultaneous
   interactive renders via a custom DSPY driver — 2nd delivers no buckets and
   `Stop` never returns." The `PkThreadQuery` 0/1 experiment is the support
   team's suggested probe here.
5. **Full progressive support.** Separately ensure the driver advertises and
   handles _everything progressive_: `acceptProgressive = 1` (done), correct
   handling of repeated full-frame re-delivery on each `Synchronize` (interactive
   re-renders from scratch and the renderer may re-`Open` or re-`Write` the whole
   frame, possibly denoised/full-size rather than bucketed). Verify
   `image_write` tolerates full-frame writes and out-of-order/overlapping
   regions, and that `image_open` being called again on the same display is
   handled.

## How to reproduce / test

**Standalone (preferred — isolates nsi from akatela):** add an example/test in
`crates/nsi/` that creates **two** `Context`s, each with a FERRIS f32 output
driver writing into its own `Arc<SharedImage>`-like buffer, sets up a trivial
scene (one sphere + one light + perspective camera), then on each:
`render_control(Start, {interactive:1, progressive:1})` + `Synchronize`. Assert
**both** buffers receive at least one bucket within a timeout, then
`render_control(Stop)` + `Wait` on both and assert both return (no hang). This
reproduces the failure without akatela and becomes the regression test.

**In akatela (integration):** point akatela at the local nsi via
`[patch.crates-io]` in `/home/ritz/code/crates/akatela/Cargo.toml`:

```toml
[patch.crates-io]
nsi = { path = "/home/ritz/code/crates/nsi" }
```

Build with `cargo build -p akatela --features nsi-render` (NEVER `--release`
unless asked; do not cap `--jobs`). The akatela side that drove per-swatch
interactive contexts was reverted to batch in commit-pending work, but the
diagnostics (`NSI[i]: first beauty bucket received`, `drop joining/joined`) and
the interactive `NsiRenderState` (`src/nsi_render/mod.rs`) remain as the
reference consumer pattern.

## Acceptance criteria

- Two (ideally N) concurrent interactive contexts each receive buckets
  (`image_write` fires for every context).
- `render_control(Stop)` + `Wait` returns promptly for every context; no
  teardown hang.
- Progressive interactive renders deliver repeated, full-frame (denoised)
  updates per `Synchronize`.
- A standalone regression test in `crates/nsi/` covers the two-context case.
- If the root cause is upstream 3Delight, the minimal C-level repro + findings
  are documented for a 3Delight bug report, and the `PkThreadQuery` 0-vs-1
  result is recorded.

## Notes / constraints

- Akatela always creates contexts with `renderthreads = -1` on `.global`
  (reserve a core for the UI). Keep that in mind when reasoning about thread
  scheduling across concurrent renders.
- 3Delight identifies displays by `imagefilename`; akatela currently reuses one
  name across contexts. If anything in 3Delight _does_ key on it, akatela can
  trivially make it unique — flag that back rather than assuming it can't.
