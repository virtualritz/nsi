# Feature: Fuzzing The Untrusted Boundaries

## Problem

`TODO.md` has carried one line since before this repo had specs:
"Fuzz the FFI boundary. Malformed input and error paths are untested."
Every test in the workspace feeds these crates input *they* produced --
a fixture built by `build()`, a stream this crate wrote, a scene a
recorder recorded. Nothing feeds them input nobody designed.

That matters most where bytes arrive from outside:

- **`nsi-parse`** reads a `.nsi` stream, which is a file from anywhere:
  written by an older 3Delight, truncated by a failed copy, produced by
  a tool with its own idea of the grammar. A panic there is a crash in
  whatever embedded the parser.
- **`nsi-ffi-wrap`** marshals arguments across a C boundary, where the
  failure mode is worse than a panic.

The parser is where fuzzing pays first: it is pure Rust, takes a byte
slice, and has a sink that can do nothing, so a fuzz target is a dozen
lines and every finding is reproducible without a renderer.

## User Stories

1. **As a maintainer**, I run `cargo fuzz run parse_stream` and it
   explores the grammar without finding a panic, so a malformed file
   reports an error rather than aborting the host.
2. **As a consumer embedding the parser**, a truncated or hostile
   stream cannot take my process down.
3. **As a reviewer**, a crash found by fuzzing becomes a unit test with
   the bytes that caused it, so it stays fixed.

## Acceptance Criteria

- A fuzz target for `parse_stream` that accepts arbitrary bytes and
  asserts only that the call returns -- `Ok` or `Err`, never a panic.
- A seed corpus from the fixtures the crate already has, so the fuzzer
  starts inside the grammar rather than outside it.
- Any crash found is reduced to bytes and pinned as an ordinary test in
  the crate, not left in the fuzzing directory.
- The fuzz target builds in CI. It is not *run* there: fuzzing has no
  natural end, and a time-boxed run in CI is a flaky test wearing a
  fuzzer's clothes.

## Non-Goals

- **Fuzzing through to a renderer.** The interesting boundary is the
  parser's; a target that needs 3Delight installed would run nowhere.
- **`nsi-ffi-wrap`'s C boundary, for now.** Fuzzing it means
  synthesising `NSIParam` structures, which is a fuzz target that has
  to be as correct as the code it tests. Recorded as the next step.
- **Coverage targets.** A number that nobody acts on is not a gate.

## Risks

- A fuzz target that accidentally constrains its input -- by building a
  `String` from the bytes, say -- explores far less than it appears to.
  The target takes `&[u8]` and hands it over unchanged.
- `cargo-fuzz` needs a nightly toolchain, and this workspace's
  toolchain is nightly already, so that is a documented requirement
  rather than a blocker.
