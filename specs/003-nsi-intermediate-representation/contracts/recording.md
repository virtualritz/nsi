# Contract: Recording

## Scope

Covers `Recorder`'s implementation of the nine `nsi_trait::Nsi` methods,
and the copying of arguments into owned storage. Does not cover
connection classification (`classification.md`), graph resolution
(`resolution.md`), or replay (`stream.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `Recorder` satisfies the full `Nsi` bound | Covered | `crates/nsi-intermediate/src/recorder.rs` `impl Nsi for Recorder` | `recorder::tests::recorder_implements_nsi` | -- |
| `Recorder` is `Send + Sync` despite storing raw pointers | Covered | `owned.rs` `HostPtr` + `unsafe impl Send/Sync` | `recorder::tests::recorder_is_send_and_sync` | -- |
| `create` then `set_attribute` is retrievable | Covered | `recorder.rs` `create`, `set_attribute` | `recorder::tests::records_a_node_and_its_attribute` | -- |
| Scalar payloads are copied during the call | Covered | `owned.rs` `OwnedArg::from_param` | `owned::tests::owns_a_single_f32`, `owns_a_string` | -- |
| Multi-component types keep every scalar | Covered | `owned.rs` `components_per_element` | `owned::tests::owns_a_point_slice_with_all_floats` | -- |
| `array_len` arguments are not truncated | Covered | `owned.rs` scalar count from `len()`, not the C `count` | `owned::tests::owns_every_scalar_of_an_array_len_argument` | -- |
| `Reference` records the address, not the pointee | Covered | `owned.rs` `Type::Reference` branch, one deref | `owned::tests::records_a_reference_as_the_address_not_its_contents` | -- |
| Node order is insertion order | Covered | `scene.rs` `IndexMap`, `shift_remove` | `scene::tests::node_order_is_insertion_order` | -- |
| `set_attribute` overwrites by name | Covered | `scene.rs` `set_attribute` | `scene::tests::set_attribute_overwrites_by_name` | -- |
| Motion samples stay separate and time-sorted | Covered | `scene.rs` `set_attribute_at_time` | `scene::tests::time_samples_are_kept_separately_and_sorted` | -- |
| `delete` removes the node and its edges | Partial | `scene.rs` `delete` | `scene::tests::delete_removes_the_node_and_its_edges` proves `Scene::delete`; nothing drives it through `Recorder::delete` | Add a `recorder::tests` case calling `Nsi::delete` and asserting the node and its edges are gone. |
| `delete_attribute` removes one key | Partial | `scene.rs` `delete_attribute` | `scene::tests::delete_attribute_removes_one_key` covers static attrs only | Add a case asserting removal from a time sample too; the implementation walks `time_attrs` but nothing proves it. |
| `disconnect` removes a recorded edge | Open | `scene.rs` `disconnect` | None | Add a test connecting then disconnecting, asserting `edges` is empty, and one asserting an unmapped `to_attr` errors. |
| `render_control` drives the state machine | Covered | `recorder.rs` `render_control` | `recorder::tests::render_control_drives_the_state_machine`, `wait_and_synchronize_do_not_change_state` | -- |
| `evaluate` is a recorded no-op | Open | `recorder.rs` `evaluate` returns `Ok(())` | None | Out of scope per `spec.md` non-goals. Either add a test asserting the no-op, or record the decision to leave procedurals unimplemented until a backend needs them. |

## Invariants

- Every argument except `Type::Reference` is copied before the call
  returns, matching the ɴsɪ C API's own contract.
- A `HostPtr` is never dereferenced by this crate.
- `ParamValue::len()` is the raw element count; the C `count` field is
  `len / array_length`. These are distinct and must not be conflated.

## Failure Modes

- **Mutex poisoning** panics with `"scene mutex poisoned"`. A panic
  while recording leaves the scene unusable, which is preferable to
  continuing from an unknown state.
- **A malformed `Reference`** cannot be detected. A pointer is opaque;
  the recorder stores what it is given.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --lib owned`
- `cargo test -p nsi-intermediate --lib recorder`
- `cargo test -p nsi-intermediate --lib scene`
- To close the `disconnect` row: a test that connects, disconnects, and
  asserts `scene.edges.is_empty()`.
- To close the `delete` row: a test driving `Nsi::delete` rather than
  `Scene::delete`.
