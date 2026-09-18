# Contract: Procedurals In Rust

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| A built procedural is loaded and executed by 3Delight | Covered | `declare_procedural!`, `shim_load` | `tests/evaluate.rs::three_delight_loads_and_executes_the_procedural` | 3Delight 2.9.207; evaluation renders nothing, so no license |
| Its calls land in the renderer's scene | Covered | `shim_execute` builds a `Context` over the handed context | `the_procedurals_calls_land_in_the_renderers_scene`: after the procedural, re-creating `row` as a mesh is refused as "already exists as type 'particles'"; without it, the same call is silent | The control is what makes the refusal evidence |
| The wrapper never ends the renderer's context | Covered | `Context::from_renderer_context` sets `ends_context: false`; `Drop` checks it | Falsified: forcing `ends_context = true` crashes `tests/evaluate.rs` with SIGSEGV | Settable only by the constructor, so not a caller's duty |
| Callbacks through a borrowed context are not freed under the renderer | Covered | `InnerContext::drop` forgets them when `!ends_context` | Code review; D3 | A leak by design, documented on the constructor |
| Parameters are read losslessly | Covered | `params.rs` | `every_type_reads_back_what_was_written`: float, double, int, int64, string, color, doublematrix, string array, array type with `arraylength` 2. Falsified: dropping color's 3 components reddens it | `pointer` shares the scalar path and is untested by value |
| Asking for the wrong type is a miss | Covered | `Param::slice` checks the tag | `the_wrong_type_is_a_miss_not_a_reinterpretation` | |
| A compiled-in procedural runs on the host's `Nsi`, no C | Covered | `execute` | `a_compiled_in_procedural_runs_on_the_hosts_own_nsi` on `nsi-intermediate`'s `Recorder`; `a_hosts_error_reaches_the_caller` | |
| Load errors and panics are reported, not unwound | Covered | `shim_load` | `a_load_error_is_reported_and_loads_nothing`, `a_load_panic_is_reported_not_unwound_into_c`, `a_missing_library_path_is_reported` | Through a stand-in `NSIReport_t` |
| Execute errors are reported through the renderer | Covered | `shim_execute` | `an_execute_error_is_reported_through_the_renderer`, in 3Delight | |
| Execute and unload panics are reported, not unwound | Partial | `shim_execute`, `shim_unload` wrap in `catch_unwind` | None | Same pattern as the load path, which is tested |
| Calls go through `nsi_library_path`, not the default library | Partial | The path from load is what `from_renderer_context` loads | None that can tell the two apart | On a machine with one renderer, both are the same library. Needs a second implementation to falsify |
| The descriptor is `NSIProcedural_t` | Covered | `Descriptor`, `#[repr(C)]` | `the_descriptor_is_laid_out_as_nsi_procedural_t` pins size to `nsi-sys`'s blob | |
| C procedurals run in a Rust renderer | Open | Non-goal | None | Needs an exported ɴsɪ C library over `FfiApiAdapter` |

## Required Evidence Before Marking Complete

- `Partial` (execute/unload panics): a test procedural that panics in
  `execute`, evaluated in 3Delight, with the panic message arriving
  through the error handler and the process surviving.
- `Partial` (`nsi_library_path`): a run under a second ɴsɪ
  implementation -- or a stand-in library -- where the default and the
  named library differ, showing the calls reach the named one.
