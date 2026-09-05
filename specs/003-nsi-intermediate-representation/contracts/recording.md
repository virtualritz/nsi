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
| `connect` records `"priority"` | Covered | `recorder.rs` `priority_of`, `scene.rs` `connect_with_priority` | `recorder::tests::connect_records_the_priority_argument` | -- |
| `render_control` drives the state machine | Covered | `recorder.rs` `render_control` | `recorder::tests::render_control_drives_the_state_machine`, `wait_and_synchronize_do_not_change_state` | -- |
| `evaluate` is a recorded no-op | Covered | `recorder.rs` `evaluate` returns `Ok(())`; the decision is a `spec.md` non-goal | `recorder::tests::evaluate_is_a_recorded_no_op`, asserting the scene is unchanged | -- |
| `connect` drops `"value"` and `"strength"` | Open | `recorder.rs` `priority_of` reads one name | None | `strength > 0` blocks a recursive `delete` in ɴsɪ. Decide whether that matters before a backend relies on it, then test or document. |
| `delete` drops its arguments, `recursive` included | Open | `recorder.rs` `delete` ignores `_args` | None | ɴsɪ's `recursive` delete removes a subgraph. Implement it or state the limitation as a non-goal. |
| `create` drops its arguments | Open | `recorder.rs` `create` ignores `_args` | None | Determine whether any ɴsɪ `create` argument is load-bearing. |
| `set_attribute` on an uncreated handle fabricates a node | Open | `scene.rs` `entry().or_default()` | None | ɴsɪ requires the node to exist. Silent fabrication is the fallback the blueprint forbids; make it a typed error, then test that `stream.rs` no longer emits a `Create` for it. |
| `disconnect` with `.all` wildcards | Open | `scene.rs` `disconnect` classifies `to_attr` first | None | `NSIDisconnect` accepts `.all` for `to` and `to_attr`; today that fails `classify`, so a legal call errors. Support it or declare it a non-goal with its own error. |
| Edge identity and duplicate connections | Open | `scene.rs` `connect` pushes unconditionally | None | ɴsɪ says re-creating a connection is not an error. A repeat currently doubles the layer in `render_outputs`. Define `(from, from_attr, to, to_attr)` as a set key and test the repeat. |
| Non-UTF-8 strings survive recording | Open | `owned.rs` `to_string_lossy` | None | `to_string_lossy` replaces invalid bytes, so R3 "copied" is false for them and the stream would differ from 3Delight's. Decide: `Vec<u8>` storage, or a documented ASCII/UTF-8 precondition. |
| `Type::Invalid` is not silently an empty `F32` | Open | `owned.rs` `Type::Invalid => OwnedData::F32(Vec::new())` | None | A silent fallback. Reject it, or add an `OwnedData::Invalid`. |
| A `ParamValue` whose `as_c_param` is `None` | Open | `owned.rs` `.expect("nsi-ffi-wrap Arg always yields a C view")` | None | `from_param` is generic and `nsi-trait` documents `None` as legal, so a non-`Arg` implementor panics. Narrow the bound or return a `Result`. |

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
  guard is alive deadlocks the calling thread. Documented on the method;
  not designed away.
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
