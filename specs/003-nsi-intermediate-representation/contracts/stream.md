# Contract: `.nsi` Stream Replay

## Scope

Covers replaying a recorded `Scene` as an ɴsɪ stream, and its equality
with what 3Delight writes for the same calls. This is the fidelity gate
for the whole recording surface.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A recorded scene replays as 3Delight's stream | Covered | `stream.rs` `write_stream` | `stream_roundtrip::recorder_replays_what_3delight_writes`, against live 3Delight 2.9.207 | -- |
| Scalar, string, colour, `int64` and `double` types emit correctly | Covered | `stream.rs` `base_type_name` | same test; `build` includes each | -- |
| `array_len` emits as `type[n]` with count divided | Covered | `stream.rs` `type_name`, `element_count` | same test; `resolution` with `array_len(2)` | -- |
| A lone scalar is bare, more than one is bracketed | Covered | `stream.rs` `write_arg` | same test | -- |
| Motion samples emit as `SetAttributeAtTime` | Covered | `stream.rs` `write_stream` | same test; `build` sets one at `t=0.5` | -- |
| Both connection forms emit correctly | Covered | `stream.rs` `to_attr_of` and the port branch | same test; `objects` and `outColor -> inColor` | -- |
| Matrices emit as `matrix` / `doublematrix` | Partial | `stream.rs` `base_type_name` | Verified by direct capture during development, but `build` sets no matrix | Add `matrix_f32!` and `matrix_f64!` to the roundtrip fixture. |
| `Reference` is omitted from the stream | Open | `stream.rs` `OwnedData::Reference => {}` | None | Capture a 3Delight stream containing a `Reference` argument and confirm it is omitted there too. **The current behaviour is an assumption.** |
| Grouping loss does not affect a renderer | Covered by design | `stream.rs` module doc | N/A -- a renderer sees final values only | Not testable here; documented as accepted in `spec.md`. |

## Invariants

- The reference stream is produced **live**, in the same test run, by
  the same generic `build` function. There is no checked-in fixture to
  drift.
- Comparison is on canonicalised statements: 3Delight wraps long values
  at an arbitrary width, so continuation lines fold back and whitespace
  collapses.
- If the streams diverge, `stream.rs` is wrong. The expectation is never
  adjusted to match.

## Failure Modes

- **No 3Delight installed:** `Context::new` returns `None` and the test
  panics with `"could not create an apistream ɴsɪ context"`. This is a
  missing prerequisite, not a passing test — see `quickstart.md`.
- **Format drift upstream:** the test fails with a statement-level diff
  naming the divergent call.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --test stream_roundtrip`
- Confirm 3Delight really ran: `-- --nocapture 2>&1 | rg '3Delight'`
  must show the banner.
- To close the `Reference` row: a captured 3Delight stream built with
  `nsi::reference!`, showing whether the argument appears.
