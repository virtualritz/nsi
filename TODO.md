# `nsi` -- To Do

- **Crash at process exit on the OIDN path.** With `interactive = 1` 3Delight
  dispatches OIDN denoising to the GPU, and the process segfaults _after_ the
  render completes and tears down cleanly (`RESULT: PASS`, then exit 139).
  Reproduces with `examples/concurrent_interactive`; `NSI_DENOISE=0` is always
  clean. **Pre-existing and unrelated to the FFI marshalling fix** -- measured
  interleaved on the same machine, `master` and the fix both crash 8/8 with
  denoising on and 0/4 with it off. The rate also swings with machine state
  (the same binary was 1/4 earlier in the day and 12/12 later), so measure
  interleaved or not at all. Never reproduces under `gdb`, so it is a race.

- **Widen the Miri round-trip coverage.** `ffi_round_trip_tests` models the
  `FnOpen` journey only. `FnWrite` / `FnFinish` and a repeated
  `image_open`/`image_close` cycle are covered only behaviourally, by the
  example rendering correctly.

- **Miri in CI.** The unsafe FFI paths now have Miri-checkable tests
  (`argument::pointer_marshalling_tests`, `output::ffi_round_trip_tests`) but
  nothing runs them automatically. Note the renderer-dependent tests must be
  filtered out -- Miri cannot execute the real FFI.

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.
