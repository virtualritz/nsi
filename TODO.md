# `nsi` -- To Do

- **Crash at process exit, root-caused.** `render_control(Stop)` + `Wait`
  and `NSIEnd` all return while 3Delight is still finishing on detached
  threads of its own. Ending the process out from under them is a SIGSEGV
  *after* a clean render. `examples/concurrent_interactive` now waits before
  returning (`NSI_EXIT_DELAY_MS`, default 500 ms; set it to `0` to reproduce)
  and returns `ExitCode` instead of calling `std::process::exit`, which had
  been skipping every destructor. Not a renderer bug: a pure-C harness of the
  same shape -- in-process `DspyRegisterDriver` driver, interactive +
  progressive, denoising, threaded driving, `exit()` with and without
  `NSIEnd` -- did not crash once in 44 runs while the Rust harness crashed
  in the same window. What remains is deciding whether `nsi` should offer a
  real drain (akatela's `drain_teardowns`) rather than a sleep.

- **Widen the Miri round-trip coverage.** `ffi_round_trip_tests` models the
  `FnOpen` journey only. `FnWrite` / `FnFinish` and a repeated
  `image_open`/`image_close` cycle are covered only behaviourally, by the
  example rendering correctly.

- **Miri in CI.** The unsafe FFI paths now have Miri-checkable tests
  (`argument::pointer_marshalling_tests`, `output::ffi_round_trip_tests`) but
  nothing runs them automatically. Note the renderer-dependent tests must be
  filtered out -- Miri cannot execute the real FFI.

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.
