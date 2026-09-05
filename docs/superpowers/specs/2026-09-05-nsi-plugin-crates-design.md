# `nsi-display` and `nsi-procedural` — safe Rust plugin crates

Date: 2026-09-05
Status: approved design, not yet implemented

## Problem

3Delight loads two kinds of plugin as dynamic libraries: **display
drivers** (ndspy `.dpy`) and **procedurals** (NSI). Both are C ABIs where
the renderer looks up exported symbols by name, hands the plugin raw
pointers it does not own, and calls it from threads it controls.

Writing one in Rust today means writing the `extern "C"` shims by hand.
Every author repeats the same four mistakes, all of which this repository
made and fixed in the last week:

- unwinding a panic into C,
- freeing memory the renderer owns,
- keeping a pointer past the call it was valid for,
- and, for procedurals, destroying the renderer's context by dropping a
  wrapper around it.

These two crates encode the answers once so plugin authors get them by
construction.

## The two ABIs

Verified against `$DELIGHT/include/{nsi_procedural.h,ndspy.h}`,
3Delight 2.9.208.

### Display driver (`ndspy.h`)

The renderer `dlopen`s the driver and resolves symbols **by name**:

| Symbol                                                            | Role                                                                                                                  |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `DspyImageOpen`                                                   | Allocate driver state, return it as `PtDspyImageHandle`; may edit the requested `PtDspyDevFormat[]` and `PtFlagStuff` |
| `DspyImageData`                                                   | One bucket of pixels                                                                                                  |
| `DspyImageClose`                                                  | Release the handle                                                                                                    |
| `DspyImageQuery`                                                  | Answer `PkProgressiveQuery`, `PkThreadQuery`, `PkRenderProgressQuery`, …                                              |
| `DspyImageReopen`, `DspyImageActiveRegion`, `DspyImageDelayClose` | Optional                                                                                                              |

`PtDspyImageHandle` is a `void*` the **driver** owns across the three
calls. `UserParameter[]` is the **renderer's**, valid only for the call.

### Procedural (`nsi_procedural.h`)

One exported symbol:

```c
struct NSIProcedural_t* NSIProceduralLoad(
    NSIContext_t ctx, NSIReport_t report,
    const char* nsi_library_path, const char* renderer_version);
```

returning a descriptor of `{nsi_version, unload, execute}`:

```c
void execute(NSIContext_t ctx, NSIReport_t report,
             struct NSIProcedural_t* proc,
             int nparams, const struct NSIParam_t* params);
void unload (NSIContext_t ctx, NSIReport_t report,
             struct NSIProcedural_t* proc);
```

Two facts drive the design. `nsi_version` must equal `NSI_VERSION`, so
version negotiation is the shim's job, not the author's. And `unload` is
documented as cleaning up "after the **last** execution" — so `execute`
runs **more than once**, and 3Delight parallelises geometry expansion, so
it may run concurrently.

## Design

### Author-facing shape

A plugin author writes a normal Rust type, implements a safe trait, and
invokes one macro. The macro exists because the renderer resolves symbols
by name: there is no point at which a plugin could register a
`Box<dyn Trait>` before `DspyImageOpen` is called.

```rust
// nsi-display; crate-type = ["cdylib"], artifact renamed to foo.dpy
struct MyDriver { file: File, width: usize, channels: usize }

impl nsi_display::DisplayDriver for MyDriver {
    /// The scalar the driver wants its pixels in. `open` rewrites the
    /// requested `PtDspyDevFormat[]` to match, so the driver -- not the
    /// renderer -- picks this, and `write` is typed accordingly.
    type Pixel = f32;

    fn open(
        params: nsi_display::Params<'_>,
        width: usize,
        height: usize,
        format: &mut PixelFormat,
    ) -> Result<Self, nsi_display::Error>;

    fn write(&mut self, bucket: nsi_display::Bucket<'_, Self::Pixel>)
        -> Result<(), nsi_display::Error>;

    fn close(self) -> Result<(), nsi_display::Error>;
}

nsi_display::declare_display_driver!(MyDriver);
```

```rust
// nsi-procedural; crate-type = ["cdylib"]
struct MyProcedural;

impl nsi_procedural::Procedural for MyProcedural {
    /// `env` carries what `NSIProceduralLoad` is handed and the author
    /// may legitimately want: `report()` for messages and
    /// `renderer_version()`. The remaining argument, `nsi_library_path`,
    /// is consumed by the shim for `init_from_path` and is deliberately
    /// not exposed -- initialising the API is not the author's job.
    fn load(env: &nsi_procedural::LoadEnv<'_>)
        -> Result<Self, nsi_procedural::Error>;

    fn execute(
        &self,
        ctx: &mut nsi_procedural::BorrowedContext<'_>,
        params: nsi_procedural::Params<'_>,
    ) -> Result<(), nsi_procedural::Error>;
}

nsi_procedural::declare_procedural!(MyProcedural);
```

`execute` takes `&self` with a `Sync` bound, not `&mut self`: procedurals
**are** expanded concurrently, so `&mut self` is not an option. Interior
mutability stays available to plugins that need shared state.

### Threading, and why the two crates differ here

`PkThreadQuery` is **a 3Delight extension, not standard ndspy** -- the
header says so at `ndspy.h:167`. Pixar's API has no thread negotiation:
the renderer serialises calls into the driver. 3Delight added the query so
a driver may opt *in* to concurrent buckets by answering
`PtDspyThreadInfo.multithread = 1`.

So concurrency is a property the driver **declares**, and the safe API
ties the trait shape to that declaration:

```rust
trait DisplayDriver: Sized {
    type Pixel: PixelType;
    /// Answered to `PkThreadQuery`. Left `false`, the renderer serialises
    /// `write`, which is what makes `&mut self` sound.
    const MULTITHREAD: bool = false;
    …
}
```

- `MULTITHREAD = false` (the default): the renderer serialises, `write`
  takes `&mut self`, and the author needs no synchronisation.
- `MULTITHREAD = true`: the macro additionally requires `Self: Sync` and
  `write` takes `&self`. A driver cannot promise concurrency without the
  compiler holding it to it.

Getting this wrong is not hypothetical: `nsi-ffi-wrap` had exactly this
bug. It answered `multithread = 1` while taking `&mut` to a shared
`FnMut`, with no `Sync` bound anywhere. Fixed by making `FnWrite` be
`Fn + Sync` and both `image_write` borrows shared -- Miri's race detector
found a *second* race the first fix missed, on `DisplayData` itself.
Neither existing example needed changing: both already used interior
mutability. This design makes that class of bug unwritable.

`ctx` is `&mut` because each `execute` call is handed its own
`NSIContext_t`, so a fresh wrapper is constructed per call and nothing is
aliased. It also states the purpose at the signature: a procedural exists
to write into that context.

### The four rules the shims enforce

1. **Panic containment.** Every generated shim wraps the author's code in
   `catch_unwind`. A panic becomes an error code — `PkDspyErrorUndefined`
   for a driver, a reported error for a procedural — never an unwind into
   C.

2. **The handle is ours.** `DspyImageOpen` boxes the driver state and
   `Box::into_raw`s it into `PtDspyImageHandle`; `DspyImageClose`
   reclaims it with `Box::from_raw` exactly once. `close(self)` takes
   ownership so the type system enforces the once.

3. **Parameters are the renderer's.** `Params<'_>` is a non-owning,
   lifetime-bound view over `UserParameter[]` / `NSIParam_t[]` with typed
   getters. It never takes ownership of anything it is handed. This is the
   rule `extract_callback` broke by `Box::from_raw`-ing the renderer's own
   pointer cell.

4. **The procedural's context is borrowed.** `BorrowedContext` **drops
   nothing**: no `NSIEnd`, no callback reclamation. Both would free memory
   the renderer owns. Today's `Context::drop` does both, so wrapping the
   procedural's context in it would destroy the renderer's context on the
   way out of `execute`.

### What is reused

`PixelFormat`, `PixelType`, the bucket-slicing arithmetic and the
`PkProgressiveQuery` / `PkThreadQuery` answers come from
`nsi_ffi_wrap::output` unchanged. That code is written, and its ownership
and aliasing behaviour is proven under Miri by
`output::ffi_round_trip_tests::the_driver_lifecycle_is_sound_end_to_end`.
Procedurals additionally reuse `Arg` / `ArgData` to emit NSI calls.

| Crate            | Depends on                                     | Initialisation         |
| ---------------- | ---------------------------------------------- | ---------------------- |
| `nsi-display`    | `ndspy-sys`, `nsi-ffi-wrap` (`output` feature) | none — never loads NSI |
| `nsi-procedural` | `nsi-sys`, `nsi-ffi-wrap`                      | `init_from_path`       |

A display driver never touches `Context`, and `NSI_API` is a
`lazy_static` initialised on first deref — so **a display driver never
`dlopen`s lib3delight**. The dependency on `nsi-ffi-wrap` costs compile
time, not runtime behaviour.

### Changes required in `nsi-ffi-wrap`

Two, both small, both forced by the ABIs:

- **A non-owning context.** A constructor yielding a `Context` whose
  `Drop` runs neither `NSIEnd` nor callback reclamation. Implemented as a
  flag on `InnerContext` rather than a parallel type, so every existing
  method is available without duplication.
- **`init_from_path`.** `NSIProceduralLoad` is handed
  `nsi_library_path`; the API global must be initialisable from it rather
  than from the current search of `DELIGHT_APP_PATH` / `lib3delight.so` /
  `$DELIGHT`. The crate already declares a `manual_init` feature for
  this, but it has no implementation — enabling it today deletes the
  `NSI_API` global and the crate stops compiling. This finishes it.

## Testing

**Miri, over the generated shims.** The shims are pure Rust: they never
call out through the C API. So Miri can execute them directly with
synthetic `UserParameter` / `NSIParam_t` arrays — the same technique that
makes the existing driver-lifecycle test a proof rather than a smoke
test. Cover: open/write/close round trip with handle reclamation, a
panicking author callback converted to an error code, and a `Params`
view outliving nothing.

**End to end, against the real renderer.** An example `.dpy` built as a
cdylib, rendered through, asserting buckets arrive and the file is
written. An example procedural `.so` referenced from a `procedural` node,
asserting the geometry it emits appears in the render. Both need a
licensed 3Delight (see the licence-server note in `AGENTS.md`).

## Risks

**Feature-gating the bound was considered and rejected.** Cargo features
must be additive; a `multithreaded_buckets` feature would add a *bound*,
so any crate in the graph enabling it would break every other crate's
non-`Sync` closures through feature unification. The bound is
unconditional.

**`crate-type = ["cdylib"]` and the `.dpy` extension.** The renderer
looks for a specific filename; Cargo produces `libfoo.so`. The examples
must rename or symlink, and the crates should document it rather than
leave each author to discover it.

**Symbol visibility.** `#[unsafe(no_mangle)]` plus default visibility is
enough on Linux and macOS. Windows needs `dllexport`, which the macro
must emit.

**Unload ordering.** `unload` receives the context, but by then the
renderer may already be tearing down. The shim must not assume the
context is still usable for anything but reporting.

## Phasing

Ordered so nothing blocks the imminent 0.10.0 release, which is ready
now:

```
Phase 0  Release 0.10.0 from the current tree.

Phase 1  nsi-ffi-wrap: non-owning context + init_from_path, with tests.
         verify: existing suite stays green; a borrowed context drops
                 neither the context nor its callbacks (Miri + a real
                 render)

Phase 2  nsi-display: trait, macro, Params view, query answers.
         verify: Miri over the shims; an example .dpy renders

Phase 3  nsi-procedural: trait, macro, BorrowedContext wiring.
         verify: Miri over the shims; an example procedural emits
                 geometry that appears in a render

Phase 4  Publish both at 0.1.0 alongside nsi-ffi-wrap 0.10.1.
```

`nsi-procedural` currently exists as a 14-line stub marked
`publish = false`; Phase 3 replaces its contents entirely.
