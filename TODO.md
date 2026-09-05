# `nsi` -- To Do

- **Widen the Miri round-trip coverage.** `ffi_round_trip_tests` models the
  `FnOpen` journey only. `FnWrite` / `FnFinish` and a repeated
  `image_open`/`image_close` cycle are covered only behaviourally, by the
  example rendering correctly.

- **Miri in CI.** The unsafe FFI paths now have Miri-checkable tests
  (`argument::pointer_marshalling_tests`, `output::ffi_round_trip_tests`) but
  nothing runs them automatically. Note the renderer-dependent tests must be
  filtered out -- Miri cannot execute the real FFI.

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.
