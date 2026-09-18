# Tasks: Procedurals In Rust

- [x] T1 `Context::from_renderer_context` in `nsi-ffi-wrap`, never
      ending the context. *Evidence:* the SIGSEGV falsification in
      `contracts/procedurals.md`.
- [x] T2 `Params`, lossless. *Evidence:*
      `every_type_reads_back_what_was_written`, falsified.
- [x] T3 The `Procedural` trait and `declare_procedural!`. *Evidence:*
      `three_delight_loads_and_executes_the_procedural`.
- [x] T4 The compiled-in route, `execute`. *Evidence:*
      `a_compiled_in_procedural_runs_on_the_hosts_own_nsi`.
- [x] T5 Errors and load panics reported, not unwound. *Evidence:* the
      load tests and `an_execute_error_is_reported_through_the_renderer`.
- [x] T6 Proof the calls reach the renderer. *Evidence:*
      `the_procedurals_calls_land_in_the_renderers_scene`, with its
      control.
- [x] T7 The example and the README.
- [ ] T8 Execute and unload panics under 3Delight. See the contract.
- [ ] T9 `nsi_library_path` honoured, shown against a second
      implementation. See the contract.
