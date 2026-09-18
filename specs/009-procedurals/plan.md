# Plan: Procedurals In Rust

## Approach

The `nsi-display` pattern, applied to `nsi_procedural.h`:

- **A trait**, `Procedural`: `load` once per render, `execute` any number
  of times, `Drop` on unload. `execute` is generic over the `Nsi`
  implementation, bound the way `nsi-parse` binds it
  (`for<'call> N: Nsi<Arg<'call> = Arg<'call, 'static>>`), so one body
  serves both routes.
- **A macro**, `declare_procedural!`, exporting `NSIProceduralLoad`. The
  descriptor the renderer gets is the first field of a `#[repr(C)]`
  allocation that also holds the library path and the author's value --
  the over-allocation `nsi.pdf` §2.7.2 suggests.
- **A parameter view**, `Params`, over the `NSIParam_t` array,
  borrowed for the call. `nsi_ffi_wrap::c_api::marshal_params_to_args`
  already exists but handles only scalars and silently drops the rest,
  so it is not used.
- **A compiled-in route**, `execute(procedural, nsi, report, args)`,
  which builds the same view from the renderer's own `Arg`s through
  `ParamValue::as_c_param` -- a borrow, not a copy.
- **In `nsi-ffi-wrap`**, `Context::from_renderer_context(handle,
  library)`: a `Context` over a handle the renderer owns, which never
  runs `NSIEnd` and leaks, rather than frees, callbacks passed through
  it.

## Gates

1. Unit tests: every parameter type reads back what was written; a
   type mismatch is a miss; the descriptor's layout matches
   `NSIProcedural_t`; load errors, load panics and a missing library
   path are reported, not unwound.
2. The compiled-in route runs on `nsi-intermediate`'s `Recorder` and
   its calls land in the recorded scene.
3. 3Delight evaluates the built example: load and execute run, the
   parameter is read, and messages arrive through the context's error
   handler.
4. Falsification: each gate is shown to redden when the code it
   guards is broken.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/procedurals.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`
