# Tasks: Fuzzing The Untrusted Boundaries

- [x] T1 A `parse_stream` fuzz target that takes bytes unchanged.
      *Evidence:* `fuzz/fuzz_targets/parse_stream.rs`.
- [x] T2 Seeds, one per statement kind. *Evidence:*
      `fuzz/seeds/parse_stream/`, ten files.
- [x] T3 A first run. *Evidence:* `research.md`, "First Run" --
      1 988 922 executions, no finding.
- [x] T4 Build it in CI. *Evidence:* `fuzz-build` in
      `.github/workflows/rust.yml`.
- [ ] T5 Fuzz `nsi-ffi-wrap`'s C boundary under AddressSanitizer.
      See `contracts/fuzzing.md`.
