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
- No repeated `create` for one handle: 3Delight logs the second call and
  a recorder holds one node.
- Every `create` and `set_attribute` before every `connect`, because
  replay emits a scene's nodes before its edges.
- No `delete`, `delete_attribute` or `disconnect`.

Outside these the two differ by construction, and the gate says nothing
about such a scene. This is not a caveat on the gate; it is its domain.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A recorded scene replays as 3Delight's stream, within the preconditions | Covered | `stream/mod.rs` `write_stream` | `stream_roundtrip::recorder_replays_what_3delight_writes`, against live 3Delight 2.9.207 | -- |
| Scalar, string, colour, `int64` and `double` types emit correctly | Covered | `stream/mod.rs` `base_type_name` | same test; `build` includes each | -- |
| `array_len` emits as `type[n]` with count divided | Covered | `stream/mod.rs` `type_name`, `element_count` | same test; `resolution` with `array_len(2)` | -- |
| Exactly one scalar is bare; everything else is bracketed | Covered | `stream/mod.rs` `write_arg` | `stream_roundtrip` carries an empty slice, which 3Delight writes as `[ ]`; `stream::tests::an_empty_slice_still_brackets`. The rule was "more than one is bracketed", which wrote an empty slice as nothing at all. | -- |
| Motion samples emit as `SetAttributeAtTime` | Covered | `stream/mod.rs` `write_stream` | same test; `build` sets one at `t=0.5` | -- |
| Every connection class emits correctly | Covered | `edge.rs` `EdgeKind::to_attr` and the port branch | same test; the fixture drives all seven non-shader classes, a shader-network edge, and a `Some("")` source port | -- |
| Matrices emit as `matrix` / `doublematrix` | Covered | `stream/mod.rs` `base_type_name` | same test; `build` sets a `matrix_f64!` `transformationmatrix` and a `matrix_f32!` `othermatrix`, and 3Delight's own stream is the expectation | -- |
| Doubles format as 3Delight formats them | Covered | `stream/mod.rs` `format_f64`, C `%.17g` | `stream::tests::doubles_format_the_way_3delight_writes_them` pins the captured values; `stream_roundtrip` drives `0.1`, `1/3`, `1e-7`, `1e20` and `-0.0` through live 3Delight | -- |
| Sample *times* format the same way | Covered | `stream/mod.rs` writes the time through `format_f64` | `stream_roundtrip` sets a sample at `1.0 / 3.0`, which the two formatters render differently | -- |
| Floats format as 3Delight formats them, for the values driven | Partial | `stream/mod.rs` `format_f32` | The gate drives `0.1`, `45`, `1e5`, `1e-7`, `123456792` and the colour components, and 3Delight's own output is the expectation for each | It is not the renderer's algorithm. Probed divergences: `1/3` as `f32` is `0.33333335` there and `0.33333334` here; `f32::MAX` is `3.4028234e38` against `3.4028235e38`; the smallest denormal is `2e-45` against `1e-45`. All three re-parse to the same float, so the difference is textual -- but a byte comparison against a renderer-written stream carrying one would fail. Either match the printer or keep the domain stated. |
| Argument flags emit correctly | Covered | `stream/mod.rs` `flag_prefix`, letters inside the type name | `stream_roundtrip` sets `per_vertex`, `per_face` and `linear_interpolation`, which 3Delight writes as `"v point"`, `"f float"` and `"l float"` | -- |
| A `Reference` argument omits its parameter line | Covered | `stream/mod.rs` `write_arg` returns before the header | `stream_roundtrip` sets one; 3Delight keeps the `SetAttribute` statement and writes no parameter. The previous behaviour emitted a header with no value, which is malformed. | -- |
| A repeated `create` | Covered | `scene/mod.rs` `create` is a no-op for a matching type and an error otherwise | `scene::tests::recreating_with_the_same_type_is_a_no_op`. 3Delight logs the repeated call and the recorder does not, so this stays an R10 precondition. | -- |
| `connect` arguments emit | Covered | `stream/mod.rs` writes `edge.args` under the `Connect` | `stream_roundtrip` connects with `"priority"`, which 3Delight writes as an indented parameter line | -- |
| Strings are escaped | Covered | `stream/mod.rs` `quoted` | `stream::tests::a_string_cannot_inject_a_statement`, `a_recorded_scene_with_hostile_strings_stays_one_statement_a_line`; `stream_roundtrip` carries a value holding a quote and a newline | -- |
| Non-UTF-8 string bytes replay | Open | `owned/mod.rs` `to_string_lossy` | None | The byte is lost at recording, not at replay; see `recording.md`. |
| The reserved handles are never declared | Covered | `stream/mod.rs` skips `Create` for `crate::is_reserved` | `stream::tests::the_reserved_handles_are_never_declared`; `stream_roundtrip` sets `.global`, which 3Delight never declares | -- |

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
- To close the remaining `Open` rows: see their own Required Next
  Evidence cells.
