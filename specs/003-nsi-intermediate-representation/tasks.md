# Tasks: ɴsɪ Intermediate Representation

Landed tasks are kept as the record of what shipped. Open tasks are the
`Partial` and `Open` contract rows, and nothing else.

Counts below are the modules as they stand: `owned` 5, `scene` 23,
`recorder` 13, `stream` 3, `resolve` 41 (19 `tests`, 14 `binding_tests`,
6 `output_tests`, 2 `instance_tests`), `classifier` 8,
`stream_roundtrip` 1. Lib total 85; 94 with the integration tests.

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
- [x] T1.13a `connect` refuses an uncreated handle.
      Evidence: `scene::tests::connecting_an_uncreated_handle_is_an_error`,
      `the_reserved_handles_need_no_create`. Spec: R18.
- [ ] T1.13b `set_attribute` on an uncreated handle still fabricates
      one. Gate: `contracts/recording.md` uncreated-handle row.
- [x] T1.14 `disconnect` honours `.all` in all four positions.
      Evidence: `scene::tests::disconnect_all_matches_every_source`,
      `disconnect_all_matches_destinations_and_attributes`,
      `disconnect_with_an_all_attribute_is_not_a_classify_error`.
      Spec: R19.
- [x] T1.15 Edge identity is `(from, from_attr, to, to_attr)`; a repeat
      updates rather than duplicates.
      Evidence: `scene::tests::a_repeated_connect_updates_rather_than_duplicates`.
      Spec: R18.
- [x] T1.16a Connection arguments survive whole, `"strength"` and
      `"value"` included. Evidence: `edge.rs` `Edge::args`;
      `recorder::tests::connect_records_the_priority_argument`.
      Spec: R16.
- [ ] T1.16b `recursive` delete is still dropped.
- [ ] T1.17 Non-UTF-8 strings. The loss is at recording, not replay:
      the boundary is `nsi-ffi-wrap` `String::new`, which takes
      `Into<Vec<u8>>`. Making it `AsRef<str>` renders the bad case
      unrepresentable. Gate: `contracts/recording.md`.

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
- [x] T3.5b `world_transform_at`, `motion_times` and
      `world_transform_samples`. Decided: never interpolate, because
      element-wise interpolation of a matrix is wrong for anything with
      a rotation in it. An unsampled time is an error naming the times
      that exist. Evidence: `resolve::tests` motion cases, 7. Spec: R13.
- [ ] T3.11 Per-path transforms for an instanced node.
      Gate: `contracts/resolution.md` instancing row. T3.8 refuses the
      case; this answers it.
- [x] T3.12 `priority`'s direction and tie-break are in `nsi.pdf`, not
      merely plausible: highest wins, then closest to the geometry. The
      question was answered by reading the specification. See
      `research.md` D8.
- [x] T3.13 Every shader slot honours its connection's `priority`.
      Evidence: `binding_tests::a_surfaceshader_connection_priority_wins`.

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
- [x] T4.3 3Delight keeps the statement and omits the parameter line.
      Evidence: `stream_roundtrip` carries a `Reference`.
- [x] T4.5 Doubles are C `%.17g`; Rust `Display` differs on four of
      five probe values. Evidence:
      `stream::tests::doubles_format_the_way_3delight_writes_them`, and
      the roundtrip fixture's discriminating values.
- [x] T4.6 Flags are letter prefixes inside the type name.
      Evidence: `stream_roundtrip` sets all three.
- [x] T4.7 Connection arguments emit as indented lines under `Connect`.
      Evidence: `stream_roundtrip` connects with `"priority"`.

## Found By Review, Round 2

Every item here is quoted from `nsi.pdf`. Each was implemented, not
merely specced; see the commit `follow ɴsɪ's own rules`.

- [x] T5.1 Gather **every** `attributes` node on the path, not one.
      Evidence: `binding_tests::every_attributes_node_on_the_path_is_gathered`.
      Spec: R12.
- [x] T5.2 Include `.root` in the chain.
      Evidence: `binding_tests::a_binding_on_the_root_is_gathered`.
- [x] T5.3 The shader agrees with the gathered order.
      Evidence: `binding_tests::the_shader_agrees_with_the_gathered_order`.
- [x] T5.4 Classify `displacementshader` and `volumeshader`.
      Evidence: `binding_tests::displacement_and_volume_shaders_resolve_too`.
- [x] T5.5 The two setters replace each other per name.
      Evidence: `scene::tests::a_static_set_clears_the_motion_samples_of_that_name`,
      `a_sampled_set_clears_the_static_value_of_that_name`. Spec: R7.
- [x] T5.6 Re-`create` with a different type is an error.
      Evidence: `scene::tests::recreating_with_a_different_type_is_an_error`.
      Spec: R17.
- [x] T5.7 A detached node is an error, and a prototype is not detached.
      Evidence: `resolve::tests::a_detached_node_is_an_error_not_identity`,
      `binding_tests::an_instancing_prototype_is_not_detached`. Spec: R20.
- [x] T5.8 Escape strings on replay.
      Evidence: `stream::tests::a_string_cannot_inject_a_statement`,
      `a_recorded_scene_with_hostile_strings_stays_one_statement_a_line`.
      Spec: R21.
- [x] T5.9 `RecordError`, `#[non_exhaustive]`, as the one recorder error.
- [ ] T5.10 Per-path transforms for an instanced node, and an
      `instances` node's `transformationmatrices` / `modelindices`.
      Gate: two `Open` rows in `contracts/resolution.md`.
- [ ] T5.11 Honour `INTERPOLATE_LINEAR` on a sampled transform.
- [ ] T5.12 Sample times of an arbitrary attribute, for deforming
      geometry whose `P` is sampled under a static transform.
- [ ] T5.13 Decide the attribute vocabulary: legacy or the documentation
      draft. `sourcemodels` to `objects` is the rename that fails
      silently. See `research.md` D10.
