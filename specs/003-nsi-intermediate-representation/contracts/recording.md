# Contract: Recording

## Scope

Covers `Recorder`'s implementation of the nine `nsi_trait::Nsi` methods,
and the copying of arguments into owned storage. Does not cover
connection classification (`classification.md`), graph resolution
(`resolution.md`), or replay (`stream.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `Recorder` satisfies the full `Nsi` bound | Covered | `recorder/mod.rs` `impl Nsi for Recorder` | `recorder::tests::recorder_implements_nsi` | -- |
| `Recorder` is `Send + Sync` | Covered | `owned/mod.rs` `HostPtr` + `unsafe impl Send/Sync` | `recorder::tests::recorder_is_send_and_sync`. Proves the auto-trait only; the soundness argument is the `'static` pin on the `Arg` GAT, which is an argument, not a test. | -- |
| `create` then `set_attribute` is retrievable | Covered | `recorder/mod.rs` `create`, `set_attribute` | `recorder::tests::records_a_node_and_its_attribute` | -- |
| Scalar payloads are copied during the call | Covered | `owned/mod.rs` `OwnedArg::from_param` | `owned::tests::owns_a_single_f32`, `owns_a_string` | -- |
| Multi-component types keep every scalar | Covered | `owned/mod.rs` `components_per_element` | `owned::tests::owns_a_point_slice_with_all_floats` | -- |
| `array_len` arguments keep exactly what the renderer reads | Covered | `owned/mod.rs` rounds the element count down as the C call does | `owned::tests::owns_every_scalar_of_an_array_len_argument` | -- |
| `Reference` marshalling derefs exactly one level | Covered | `owned/mod.rs` `Type::Reference` branch | `owned::tests::records_a_reference_as_the_address_not_its_contents`, calling `from_param` directly | -- |
| `Reference` through `Nsi::set_attribute` records the address | Covered | `recorder/mod.rs` `set_attribute` at `Arg<'call, 'static>` | `recorder::tests::a_reference_through_the_trait_records_the_host_address`, with a `'static` payload -- the only path a consumer has | -- |
| A `Callback` records its address and leaks its payload | Covered | `owned/mod.rs` has no `drop_fn`; `nsi-ffi-wrap` `Callback::drop_fn` is `pub(crate)`, `Callback::type_` reports `Reference` | `recorder::tests::a_callback_records_its_address_and_leaks_its_payload`, which asserts the reclaim count stays `0` | -- |
| Node order is insertion order | Covered | `scene/mod.rs` `IndexMap`, `shift_remove` | `scene::tests::node_order_is_insertion_order` | -- |
| `set_attribute` overwrites by name | Covered | `scene/mod.rs` `set_attribute` | `scene::tests::set_attribute_overwrites_by_name` | -- |
| Motion samples stay separate and time-sorted | Covered | `scene/mod.rs` `set_attribute_at_time` | `scene::tests::time_samples_are_kept_separately_and_sorted` | -- |
| Sample times match the renderer's | Covered | `scene/mod.rs` refuses a non-finite time and normalises `-0.0` | `scene::tests::a_non_finite_sample_time_is_refused`, `negative_zero_is_the_same_sample_time_as_zero`. 3Delight answers `E6026` for a `NaN` time and reads `-0` as `+0`; treating them otherwise handed a backend a zero-length motion segment | -- |
| `delete` removes the node and its edges | Covered | `scene/mod.rs` `delete`, `recorder/mod.rs` `delete` | `scene::tests::delete_removes_the_node_and_its_edges` and `recorder::tests::delete_through_the_trait_removes_the_node_and_its_edges` | -- |
| `delete_attribute` removes one key, statics and samples | Covered | `scene/mod.rs` `delete_attribute` walks `time_attrs` | `scene::tests::delete_attribute_removes_one_key`, `delete_attribute_removes_from_every_time_sample` | -- |
| `disconnect` removes a recorded edge | Covered | `scene/mod.rs` `disconnect`, `recorder/mod.rs` `disconnect` | `scene::tests::disconnect_removes_only_the_named_edge`, `disconnect_rejects_an_unmapped_destination`, `disconnect_ignores_priority`; `recorder::tests::disconnect_through_the_trait_removes_one_edge`, `an_unmapped_disconnect_is_an_error` | -- |
| `connect` records every argument, not just `"priority"` | Covered | `edge.rs` `Edge::args` / `Edge::priority`; `recorder/mod.rs` `connect` | `recorder::tests::connect_records_the_priority_argument` | -- |
| `render_control` drives the state machine | Covered | `recorder/mod.rs` `render_control` | `recorder::tests::render_control_drives_the_state_machine`, `wait_and_synchronize_do_not_change_state` | -- |
| `evaluate` is a recorded no-op | Covered | `recorder/mod.rs` `evaluate` returns `Ok(())`; the decision is a `spec.md` non-goal | `recorder::tests::evaluate_is_a_recorded_no_op`, asserting the scene is unchanged | -- |
| `"value"` and `"strength"` survive recording | Covered | `edge.rs` `Edge::args` keeps the arguments whole | `recorder::tests::connect_records_the_priority_argument` proves the vector is carried; `stream_roundtrip` replays a prioritised connection against 3Delight | -- |
| The strength rule holds transitively | Covered | `scene/mod.rs` `delete_recursive` checks strength on every edge into the doomed set, not only where a candidate is discovered | `scene::tests::strength_blocks_a_recursive_delete_through_a_second_path`; a node reached by a second, weak path was deleted despite holding a strong connection | -- |
| `delete` honours `recursive` | Covered | `scene/mod.rs` `delete_recursive`, `recorder/mod.rs` `delete` reads the argument, `edge.rs` `Edge::strength` | `scene::tests::a_recursive_delete_takes_the_network_with_it`, `a_recursive_delete_spares_a_node_used_elsewhere`, `strength_blocks_a_recursive_delete`, `a_plain_delete_is_not_recursive`, `a_recursive_delete_still_refuses_the_reserved_nodes` | -- |
| `create` drops its arguments | Covered | `recorder/mod.rs` `create` ignores `_args`; `scene/mod.rs` `create` keys identity on the node type | `recorder::tests::create_arguments_are_inert_but_the_type_is_not`; making the arguments part of identity, and dropping the type check, each redden it. ɴsɪ says "there are no optional parameters defined as of now", but also that a repeat "does nothing if all other parameters match the call which created that node. Otherwise, it emits an error" -- which reads as though the arguments were identity. Rendered, they are not: 3Delight accepts `Create "n" "attributes" "foo" "int" 1 [1]` followed by the same with `[2]` and the node still works, while a repeat with a different **type** is `E6002 error creating node 'extra' of type 'transform', already exists as type 'attributes'`. 3Delight *continues* after E6002, keeping the original node; this crate returns `RecordError::TypeMismatch` and is therefore stricter, which surfaces the mistake instead of rendering past it | -- |
| A connection to an uncreated handle is refused | Covered | `scene/mod.rs` `is_known`, `RecordError::UnknownHandle` | `scene::tests::connecting_an_uncreated_handle_is_an_error`, `the_reserved_handles_need_no_create` | -- |
| `disconnect` honours `.all` in all four positions | Covered | `scene/mod.rs` `disconnect`, `EdgeKind::to_attr`, `lib.rs` `ALL` | `scene::tests::disconnect_all_matches_every_source` (ɴsɪ's own documented example), `disconnect_all_matches_destinations_and_attributes`, `disconnect_all_matches_every_source_attribute` (the source-attribute position, which was a silent no-op), `disconnect_with_an_all_attribute_is_not_a_classify_error` | -- |
| The reserved nodes cannot be deleted | Covered | `scene/mod.rs` `delete`, `RecordError::Reserved` | `scene::tests::the_reserved_nodes_cannot_be_deleted`; deleting `.root` stripped every membership edge | -- |
| A repeated `connect` updates rather than duplicates | Covered | `scene/mod.rs` `connect_with_args` matches on `(from, to, kind)` | `scene::tests::a_repeated_connect_updates_rather_than_duplicates`; without it the node reads as having two parents and its whole subtree fails to resolve | -- |
| The reserved handles cannot be created | Covered | `scene/mod.rs` `create`, `RecordError::Reserved` | `scene::tests::the_reserved_handles_cannot_be_created`. 3Delight answers `E6002`; accepting it kept a node replay drops, so the scene changed on its own first round trip | -- |
| Re-`create` with a different type is refused | Covered | `scene/mod.rs` `create`, `RecordError::TypeMismatch` | `scene::tests::recreating_with_a_different_type_is_an_error`, `recreating_with_the_same_type_is_a_no_op` | -- |
| The two setters replace each other per name | Covered | `scene/mod.rs` `set_attribute` clears samples; `set_attribute_at_time` clears the static value | `scene::tests::a_static_set_clears_the_motion_samples_of_that_name`, `a_sampled_set_clears_the_static_value_of_that_name` | -- |
| Non-UTF-8 strings survive recording | Covered | `owned/mod.rs` stores `OwnedData::String(Vec<Vec<u8>>)` and copies `CStr::to_bytes` | `owned::tests::recording_keeps_a_non_utf8_byte`; restoring `to_string_lossy` reddens it. The row previously proposed making non-UTF-8 *unrepresentable* upstream instead. That would be the wrong invariant: `nsi-parse` must represent a stream 3Delight wrote, and `renderdl -cat` echoes a Latin-1 `café.exr` back **raw**, not as an escape as this row also claimed. A file name on Linux is not required to be UTF-8 | -- |
| A foreign `ParamValue` panics or falls back | Covered | `owned/mod.rs` `from_param` is `pub(crate)` | Both paths are unreachable *by construction*, not by argument: [`Recorder`]'s `Arg` GAT pins the parameter to `nsi_ffi_wrap::Arg`, whose `as_c_param` returns `Some` unconditionally and which never produces `Type::Invalid`; only a foreign implementor could reach either, and only through this being public. An external crate calling it now fails to compile with `E0624: associated function 'from_param' is private`, checked against a consumer outside the workspace. The `Invalid` arm returned an empty `f32` array -- recording a *different* argument rather than refusing -- and is now `unreachable!`, so if the pinned type ever changes it says so instead of quietly succeeding. Chosen over returning a `Result` that every internal caller would unwrap for a case that cannot arise. It also takes `Arg` rather than a generic `P: ParamValue`: while it stayed generic, "unreachable" rested on nobody in this crate passing something else, which is a habit rather than a guarantee | -- |

## Invariants

- Every argument except `Type::Reference` is copied before the call
  returns, matching the ɴsɪ C API's own contract.
- A `HostPtr` is never dereferenced by this crate.
- `ParamValue::len()` is the raw element count; the C `count` field is
  `len / array_length`. These are distinct and must not be conflated.
- `RenderState` transitions are total: an action with no transition from
  the current state leaves it unchanged. `Resume` from `Idle` stays
  `Idle`; `Start` from `Suspended` becomes `Running`.

## Failure Modes

- **Mutex poisoning** panics with `"scene mutex poisoned"`. A panic
  while recording leaves the scene unusable, which is preferable to
  continuing from an unknown state.
- **Deadlock.** `Recorder::scene` returns a guard over the lock every
  `Nsi` method takes. Recording through the same `Recorder` while a
  guard is alive deadlocks the calling thread, with no diagnostic. It is
  easier to hit than it looks: two calls in *one expression*, as in
  `r.scene().len() + r.scene().edges().count()`, hold two guards at
  once and block forever. Documented on the method; not designed away.
- **A malformed `Reference`** cannot be detected. A pointer is opaque;
  the recorder stores what it is given.
- **A leaked `Callback`.** Accepted, per R14.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --lib owned`
- `cargo test -p nsi-intermediate --lib recorder`
- `cargo test -p nsi-intermediate --lib scene`
- To close the `.all` row: a test disconnecting with `nsi::ALL` as the
  `to` handle and as the `to_attr`, asserting the documented outcome.
- To close the uncreated-handle row: a test asserting
  `set_attribute` on an unknown handle is an error, plus a `stream/mod.rs`
  test asserting no `Create` is emitted for one.
