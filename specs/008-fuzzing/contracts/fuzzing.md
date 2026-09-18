# Contract: Fuzzing The Untrusted Boundaries

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| Arbitrary bytes into `parse_stream` never panic | Covered | `crates/nsi-parse/fuzz/fuzz_targets/parse_stream.rs` | A 31-second run from the ten seeds: 1 988 922 executions, 1099 edges covered, no crash, timeout or sanitizer report. See `research.md`, "First Run" | Evidence of absence over what was explored, not a proof; the next run starts from the grown corpus |
| The target explores bytes, not strings | Covered | The target passes `data` straight through; no `String` or `&str` is built | Code review of the target; D2 | Building a string first would limit the fuzzer to valid UTF-8 |
| The target cannot rot unnoticed | Covered | `fuzz-build` in `.github/workflows/rust.yml` | CI builds it on every push | Built, never run, per D3 |
| A crash becomes an ordinary test | Covered | The policy in `plan.md`, gate 3 | No crash has been found, so none has been pinned | Nothing to pin yet |
| `nsi-ffi-wrap`'s C boundary is fuzzed | Open | Not implemented | None | D1: needs synthesised `NSIParam` arrays and a sanitizer run |

## Required Evidence Before Marking Complete

- `Open` (the C boundary): a target that builds `NSIParam` arrays from
  fuzzer bytes -- lengths, types, flags and counts all drawn from the
  input -- run under AddressSanitizer through `FfiApiAdapter`'s
  marshalling, with a run as long as the parser's and no report.
