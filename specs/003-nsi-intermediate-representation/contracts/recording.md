# Contract: Recording

## Scope

Covers `Recorder`'s implementation of the nine `nsi_trait::Nsi` methods,
and the copying of arguments into owned storage. Does not cover
connection classification (`classification.md`), graph resolution
(`resolution.md`), or replay (`stream.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `Recorder` satisfies the full `Nsi` bound | Covered | `recorder.rs` `impl Nsi for Recorder` | `recorder::tests::recorder_implements_nsi` | -- |
| `Recorder` is `Send + Sync` | Covered | `owned.rs` `HostPtr` + `unsafe impl Send/Sync` | `recorder::tests::recorder_is_send_and_sync`. Proves the auto-trait only; the soundness argument is the `'static` pin on the `Arg` GAT, which is an argument, not a test. | -- |
| `create` then `set_attribute` is retrievable | Covered | `recorder.rs` `create`, `set_attribute` | `recorder::tests::records_a_node_and_its_attribute` | -- |
| Scalar payloads are copied during the call | Covered | `owned.rs` `OwnedArg::from_param` | `owned::tests::owns_a_single_f32`, `owns_a_string` | -- |
| Multi-component types keep every scalar | Covered | `owned.rs` `components_per_element` | `owned::tests::owns_a_point_slice_with_all_floats` | -- |
| `array_len` arguments are not truncated | Covered | `owned.rs` scalar count from `len()`, not the C `count` | `owned::tests::owns_every_scalar_of_an_array_len_argument` | -- |
| `Reference` marshalling derefs exactly one level | Covered | `owned.rs` `Type::Reference` branch | `owned::tests::records_a_reference_as_the_address_not_its_contents`, calling `from_param` directly | -- |
| `Reference` through `Nsi::set_attribute` records the address | Covered | `recorder.rs` `set_attribute` at `Arg<'call, 'static>` | `recorder::tests::a_reference_through_the_trait_records_the_host_address`, with a `'static` payload -- the only path a consumer has | -- |
| A `Callback` records its address and leaks its payload | Covered | `owned.rs` has no `drop_fn`; `nsi-ffi-wrap` `Callback::drop_fn` is `pub(crate)`, `Callback::type_` reports `Reference` | `recorder::tests::a_callback_records_its_address_and_leaks_its_payload`, which asserts the reclaim count stays `0` | -- |
| Node order is insertion order | Covered | `scene.rs` `IndexMap`, `shift_remove` | `scene::tests::node_order_is_insertion_order` | -- |
| `set_attribute` overwrites by name | Covered | `scene.rs` `set_attribute` | `scene::tests::set_attribute_overwrites_by_name` | -- |
| Motion samples stay separate and time-sorted | Covered | `scene.rs` `set_attribute_at_time` | `scene::tests::time_samples_are_kept_separately_and_sorted` | -- |
| Sample times key on a total order | Covered | `scene.rs` `total_cmp`, not `==` | `scene::tests::a_nan_sample_time_matches_itself`, `negative_zero_is_a_distinct_sample_time` | -- |
| `delete` removes the node and its edges | Covered | `scene.rs` `delete`, `recorder.rs` `delete` | `scene::tests::delete_removes_the_node_and_its_edges` and `recorder::tests::delete_through_the_trait_removes_the_node_and_its_edges` | -- |
| `delete_attribute` removes one key, statics and samples | Covered | `scene.rs` `delete_attribute` walks `time_attrs` | `scene::tests::delete_attribute_removes_one_key`, `delete_attribute_removes_from_every_time_sample` | -- |
| `disconnect` removes a recorded edge | Covered | `scene.rs` `disconnect`, `recorder.rs` `disconnect` | `scene::tests::disconnect_removes_only_the_named_edge`, `disconnect_rejects_an_unmapped_destination`, `disconnect_ignores_priority`; `recorder::tests::disconnect_through_the_trait_removes_one_edge`, `an_unmapped_disconnect_is_an_error` | -- |
| `connect` records every argument, not just `"priority"` | Covered | `edge.rs` `Edge::args` / `Edge::priority`; `recorder.rs` `connect` | `recorder::tests::connect_records_the_priority_argument` | -- |
| `render_control` drives the state machine | Covered | `recorder.rs` `render_control` | `recorder::tests::render_control_drives_the_state_machine`, `wait_and_synchronize_do_not_change_state` | -- |
| `evaluate` is a recorded no-op | Covered | `recorder.rs` `evaluate` returns `Ok(())`; the decision is a `spec.md` non-goal | `recorder::tests::evaluate_is_a_recorded_no_op`, asserting the scene is unchanged | -- |
| `"value"` and `"strength"` survive recording | Covered | `edge.rs` `Edge::args` keeps the arguments whole | `recorder::tests::connect_records_the_priority_argument` proves the vector is carried; `stream_roundtrip` replays a prioritised connection against 3Delight | -- |
| The strength rule holds transitively | Covered | `scene.rs` `delete_recursive` checks strength on every edge into the doomed set, not only where a candidate is discovered | `scene::tests::strength_blocks_a_recursive_delete_through_a_second_path`; a node reached by a second, weak path was deleted despite holding a strong connection | -- |
| `delete` honours `recursive` | Covered | `scene.rs` `delete_recursive`, `recorder.rs` `delete` reads the argument, `edge.rs` `Edge::strength` | `scene::tests::a_recursive_delete_takes_the_network_with_it`, `a_recursive_delete_spares_a_node_used_elsewhere`, `strength_blocks_a_recursive_delete`, `a_plain_delete_is_not_recursive`, `a_recursive_delete_still_refuses_the_reserved_nodes` | -- |
| `create` drops its arguments | Open | `recorder.rs` `create` ignores `_args` | None | Determine whether any ɴsɪ `create` argument is load-bearing. |
| A connection to an uncreated handle is refused | Covered | `scene.rs` `is_known`, `RecordError::UnknownHandle` | `scene::tests::connecting_an_uncreated_handle_is_an_error`, `the_reserved_handles_need_no_create` | -- |
| `set_attribute` on an uncreated handle still fabricates one | Open | `scene.rs` `entry().or_default()` | None | `connect` refuses unknown handles; `set_attribute` does not, so a typo still invents one. The reserved handles are handled -- they are never declared on replay -- so what remains is rejecting the rest. |
| `disconnect` honours `.all` in all four positions | Covered | `scene.rs` `disconnect`, `EdgeKind::to_attr`, `lib.rs` `ALL` | `scene::tests::disconnect_all_matches_every_source` (ɴsɪ's own documented example), `disconnect_all_matches_destinations_and_attributes`, `disconnect_all_matches_every_source_attribute` (the source-attribute position, which was a silent no-op), `disconnect_with_an_all_attribute_is_not_a_classify_error` | -- |
| The reserved nodes cannot be deleted | Covered | `scene.rs` `delete`, `RecordError::Reserved` | `scene::tests::the_reserved_nodes_cannot_be_deleted`; deleting `.root` stripped every membership edge | -- |
| A repeated `connect` updates rather than duplicates | Covered | `scene.rs` `connect_with_args` matches on `(from, to, kind)` | `scene::tests::a_repeated_connect_updates_rather_than_duplicates`; without it the node reads as having two parents and its whole subtree fails to resolve | -- |
| The reserved handles cannot be created | Covered | `scene.rs` `create`, `RecordError::Reserved` | `scene::tests::the_reserved_handles_cannot_be_created`. 3Delight answers `E6002`; accepting it kept a node replay drops, so the scene changed on its own first round trip | -- |
| Re-`create` with a different type is refused | Covered | `scene.rs` `create`, `RecordError::TypeMismatch` | `scene::tests::recreating_with_a_different_type_is_an_error`, `recreating_with_the_same_type_is_a_no_op` | -- |
| The two setters replace each other per name | Covered | `scene.rs` `set_attribute` clears samples; `set_attribute_at_time` clears the static value | `scene::tests::a_static_set_clears_the_motion_samples_of_that_name`, `a_sampled_set_clears_the_static_value_of_that_name` | -- |
| Non-UTF-8 strings survive recording | Open | `owned.rs` `to_string_lossy` | None | 3Delight round-trips the raw byte as an escape; this crate replaces it with U+FFFD at *recording* time, so it is gone before replay could escape it. The boundary is upstream: `nsi-ffi-wrap` `String::new` takes `Into<Vec<u8>>`. Making that `AsRef<str>` renders non-UTF-8 unrepresentable rather than checked, and this row becomes a note. |
| A foreign `ParamValue` panics or falls back | Open | `owned.rs` `.expect(...)`, `Type::Invalid => F32(vec![])` | None | Neither is reachable through `Recorder`: the `Arg` GAT pins it to `nsi_ffi_wrap::Arg`, whose `as_c_param` never returns `None` and which has no `Invalid`. Both need a *foreign* implementor of the `pub` `OwnedArg::from_param`. Narrow it to `pub(crate)` and drop this row, or return a `Result`. |

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
  `set_attribute` on an unknown handle is an error, plus a `stream.rs`
  test asserting no `Create` is emitted for one.
