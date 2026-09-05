# Contract: Stream Replay

## Scope

Covers `write_stream`: replaying a recorded `Scene` as an ɴsɪ stream, and
holding that output against what 3Delight writes for the same calls.

## Preconditions

A recorder holds scene **state**; 3Delight's `apistream` is a **call
log**. They agree only for a scene that does not distinguish the two.
R10 names the preconditions and every row below inherits them:

- One attribute per `set_attribute` call.
- A node's static attributes set before its motion samples, because
  `write_stream` emits `attrs` before `time_attrs` per node.
- No repeated `create` for one handle.
- No `delete`, `delete_attribute` or `disconnect`.

Outside these the two differ by construction, and the gate says nothing
about such a scene. This is not a caveat on the gate; it is its domain.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A recorded scene replays as 3Delight's stream, within the preconditions | Covered | `stream.rs` `write_stream` | `stream_roundtrip::recorder_replays_what_3delight_writes`, against live 3Delight 2.9.207 | -- |
| Scalar, string, colour, `int64` and `double` types emit correctly | Covered | `stream.rs` `base_type_name` | same test; `build` includes each | -- |
| `array_len` emits as `type[n]` with count divided | Covered | `stream.rs` `type_name`, `element_count` | same test; `resolution` with `array_len(2)` | -- |
| A lone scalar is bare, more than one is bracketed | Covered | `stream.rs` `write_arg` | same test | -- |
| Motion samples emit as `SetAttributeAtTime` | Covered | `stream.rs` `write_stream` | same test; `build` sets one at `t=0.5` | -- |
| Every connection class emits correctly | Covered | `stream.rs` `to_attr_of` and the port branch | same test; the fixture drives all seven non-shader classes, a shader-network edge, and a `Some("")` source port | -- |
| Matrices emit as `matrix` / `doublematrix` | Covered | `stream.rs` `base_type_name` | same test; `build` sets a `matrix_f64!` `transformationmatrix` and a `matrix_f32!` `othermatrix`, and 3Delight's own stream is the expectation | -- |
| Floats format as 3Delight formats them | Partial | `stream.rs` `write_scalars` uses Rust `Display` | The fixture's values (`45`, `0.1`, `0.5`, `1280`, matrix entries) agree with 3Delight's `printf` | Rust's `Display` and C's `printf` are different algorithms that happen to agree on short decimals. Add a value that discriminates -- `1e-7`, `0.1f32` widened, a large `f64` -- and see which is right. |
| Sample *times* format as 3Delight formats them | Partial | `stream.rs` `SetAttributeAtTime` writes `{time}` via `Display` | Only `0.5` is exercised | Same as above, for the time field. |
| Argument flags emit correctly | Open | `owned.rs` records `flags`; `stream.rs` never writes them | None | `per_vertex`, `per_face` and `linear_interpolation` are recorded and dropped on replay. Determine how 3Delight writes them, then emit them. |
| A `Reference` argument | Open | `stream.rs` `OwnedData::Reference => {}` omits the payload but `write_arg` has already written the header | None | The current output is a `"name" "pointer" 1 ` line with no value, which is malformed. Whether 3Delight omits the whole statement or writes something is **an assumption either way**; capture a 3Delight stream containing a `Reference` and match it. |
| A repeated `create` | Open | `scene.rs` `create` updates in place | None | 3Delight logs a second `Create`; the recorder has one node. Observed while extending the fixture. Document as a precondition or reconcile. |
| `connect` arguments emit | Open | `stream.rs` writes no arguments on `Connect` | None | `"priority"` is now recorded but never replayed, so a prioritised scene diverges. Emit it. |

## Invariants

- The reference stream is produced **live**, in the same test run, by
  3Delight, from the same `build` function. There is no checked-in
  fixture to drift.
- Comparison is on canonicalised statements, not bytes: 3Delight wraps
  long values at an arbitrary width.
- **Grouping loss is accepted, not a defect.** One ɴsɪ call setting three
  attributes and three calls setting one each record identically, and
  both replay as three statements. A renderer only ever sees final
  values, so scene state is the right invariant for a backend. This is
  why the preconditions above exist rather than being a row to close.

## Failure Modes

- **No 3Delight** means the gate cannot run. A missing prerequisite is a
  failure, never a pass, and never a licence to mark a row `Covered`.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --test stream_roundtrip`, with
  `DELIGHT` set and a reachable licence server.
- To close the float rows: extend `build` with a value whose Rust
  `Display` and C `printf` renderings differ, and record which 3Delight
  writes.
- To close the flags row: a 3Delight stream containing a `per_vertex`
  argument.
- To close the `Reference` row: a 3Delight stream containing a
  `Reference` argument.
