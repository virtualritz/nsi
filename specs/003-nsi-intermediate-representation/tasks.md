# Tasks: ɴsɪ Intermediate Representation

Landed tasks are kept as the record of what shipped. Open tasks are the
`Partial` and `Open` contract rows, and nothing else.

Counts below are the modules as they stand: `owned` 5, `scene` 13,
`recorder` 13, `resolve` 26 (across `tests`, `binding_tests`,
`output_tests`, `instance_tests`), `classifier` 8, `stream_roundtrip` 1.

## User Story 1: Record An ɴsɪ Scene (P1)

- [x] T1.1 `impl ParamValue for Arg` in `nsi-ffi-wrap`.
      Evidence: `param_value::tests`, 4 cases.
- [x] T1.2 `impl Nsi for Context` in `nsi-ffi-wrap`.
      Evidence: `nsi_impl::tests`, 2 cases.
- [x] T1.3 `OwnedArg` / `OwnedData` / `HostPtr`.
      Evidence: `owned::tests`, 5 cases.
- [x] T1.4 `Scene` node and attribute tables.
      Evidence: `scene::tests`.
- [x] T1.5 `Recorder` implementing `Nsi`.
      Evidence: `recorder::tests`.
- [x] T1.6 Test `Nsi::delete` through the `Recorder`, not `Scene`.
      Evidence: `recorder::tests::delete_through_the_trait_removes_the_node_and_its_edges`.
      Closed `contracts/recording.md` `delete`.
- [x] T1.7 Test `delete_attribute` removing from a time sample.
      Evidence: `scene::tests::delete_attribute_removes_from_every_time_sample`.
      Closed `contracts/recording.md` `delete_attribute`.
- [x] T1.8 Test `disconnect`, including an unmapped `to_attr`.
      Evidence: `scene::tests::disconnect_removes_only_the_named_edge`,
      `disconnect_rejects_an_unmapped_destination`,
      `disconnect_ignores_priority`;
      `recorder::tests::disconnect_through_the_trait_removes_one_edge`,
      `an_unmapped_disconnect_is_an_error`.
      Closed `contracts/recording.md` `disconnect`.
- [x] T1.9 Decide `evaluate`. Decided: a no-op, per the `spec.md`
      non-goal. Evidence: `recorder::tests::evaluate_is_a_recorded_no_op`.
- [x] T1.10 Prove a `Reference` through `Nsi::set_attribute`, the only
      path a consumer has and the only one the `'static` pin governs.
      Evidence: `recorder::tests::a_reference_through_the_trait_records_the_host_address`.
- [x] T1.11 Pin the `Callback` leak as a known limitation.
      Evidence: `recorder::tests::a_callback_records_its_address_and_leaks_its_payload`,
      asserting the reclaim count stays `0`. Spec: R14.
- [x] T1.12 Key motion samples on a total order.
      Evidence: `scene::tests::a_nan_sample_time_matches_itself`,
      `negative_zero_is_a_distinct_sample_time`. Spec: R7.
- [ ] T1.13 `set_attribute` on an uncreated handle is a typed error.
      Gate: `contracts/recording.md` uncreated-handle row. It currently
      fabricates a node, and `stream.rs` then emits a `Create` 3Delight
      never wrote.
- [ ] T1.14 `disconnect` with `.all` for `to` and `to_attr`.
      Gate: `contracts/recording.md` `.all` row. A legal ɴsɪ call today
      fails `classify`.
- [ ] T1.15 Define edge identity as `(from, from_attr, to, to_attr)`.
      Gate: `contracts/recording.md` edge-identity row. A repeated
      `connect` doubles a layer in `render_outputs`.
- [ ] T1.16 Decide `recursive` delete and connection `strength`.
      Gate: the two `Open` ignored-argument rows.
- [ ] T1.17 Decide non-UTF-8 strings and `Type::Invalid`.
      Gate: the two `Open` `owned.rs` rows. Both are silent fallbacks.

## User Story 2: Know What A Connection Means (P1)

- [x] T2.1 Exhaustive `classify` over `to_attr`.
      Evidence: `tests/classifier.rs`.
- [x] T2.2 Extend the roundtrip fixture to every non-shader edge class.
      Evidence: `stream_roundtrip`, which now drives all seven plus a
      shader-network edge. Closed `contracts/classification.md` inverse.
- [x] T2.3 `Some("")` is `None`, per ɴsɪ.
      Evidence: `classifier::an_empty_source_port_is_not_a_port`, and a
      live case in `stream_roundtrip`. Spec: R5.

## User Story 3: Resolved Facts (P1)

- [x] T3.1 `world_transform` with row-vector composition.
      Evidence: `resolve::tests`, 4 composition cases.
- [x] T3.2 `geometry_binding`. Evidence: `binding_tests`.
- [x] T3.3 `render_outputs`. Evidence: `output_tests`.
- [x] T3.4 `instance_sources`. Evidence: `instance_tests`.
- [x] T3.5a **Motion-sampled transforms fail loudly.** A motion-blurred
      scene no longer resolves to its static pose.
      Evidence: `resolve::tests::a_motion_sampled_transform_is_an_error`,
      `motion_samples_of_other_attributes_do_not_block_resolution`.
      Spec: R13.
- [x] T3.6 Two-screen fixture.
      Evidence: `output_tests::multiple_screens_yield_one_output_each`.
- [x] T3.7 Test that a `MatrixF32` transform is skipped deliberately.
      Evidence: `resolve::tests::a_non_f64_matrix_is_skipped_not_reinterpreted`.
- [x] T3.8 Multi-parent is a typed error, not the first parent's chain.
      Evidence: `resolve::tests::more_than_one_parent_is_an_error`.
      Spec: R11.
- [x] T3.9 A cycle is a typed error, and the test asserts it. The
      previous `a_cycle_terminates` asserted nothing.
      Evidence: `resolve::tests::a_cycle_is_an_error`,
      `binding_tests::a_cycle_is_an_error_for_bindings_too`. Spec: R9.
- [x] T3.10 Inherit `geometryattributes` from ancestor transforms, with
      `priority`. Evidence:
      `binding_tests::a_binding_on_an_ancestor_transform_is_inherited`,
      `the_nearest_binding_wins_at_equal_priority`,
      `priority_beats_proximity`;
      `recorder::tests::connect_records_the_priority_argument`.
      Spec: R12.
- [ ] T3.5b **`world_transform_at(handle, time)`.** The API decision,
      then a two-sample chain. Requires deciding interpolation between
      samples. Gate: `contracts/resolution.md` motion row. Still the
      largest known gap; it is now loud rather than wrong.
- [ ] T3.11 Per-path transforms for an instanced node.
      Gate: `contracts/resolution.md` instancing row. T3.8 refuses the
      case; this answers it.
- [ ] T3.12 Confirm `priority`'s direction and tie-break against
      3Delight. Gate: `contracts/resolution.md` `priority` row, which is
      `Partial` because the rule is chosen, not observed.
- [ ] T3.13 Decide whether `surfaceshader` honours `priority` too.

## User Story 4: Fidelity (P2)

- [x] T4.1 `write_stream`, format read from live 3Delight output.
      Evidence: `stream_roundtrip`, against 3Delight 2.9.207.
- [x] T4.2 Add matrices to the roundtrip fixture.
      Evidence: `stream_roundtrip` now sets a `matrix_f64!`
      `transformationmatrix` and a `matrix_f32!` `othermatrix`. Closed
      `contracts/stream.md` matrix row.
- [x] T4.4 State R10's preconditions. Two were discovered by violating
      them while extending the fixture: static attributes must precede
      motion samples, and a handle must not be `create`d twice.
      Spec: R10; `contracts/stream.md` Preconditions.
- [ ] T4.3 Determine whether 3Delight emits `Reference` arguments.
      Gate: `contracts/stream.md` `Reference` row. The current output
      writes a header with no value, which is malformed either way.
- [ ] T4.5 Discriminate Rust `Display` from C `printf` float formatting.
      Gate: the two `Partial` float rows. The fixture's values agree by
      luck, not by construction.
- [ ] T4.6 Emit argument flags (`per_vertex`, `per_face`,
      `linear_interpolation`). Gate: `contracts/stream.md` flags row.
      They are recorded and dropped.
- [ ] T4.7 Emit `connect` arguments, `"priority"` above all. It is now
      recorded but never replayed, so a prioritised scene diverges.
