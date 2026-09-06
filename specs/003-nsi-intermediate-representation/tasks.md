# Tasks: ɴsɪ Intermediate Representation

Landed tasks are kept as the record of what shipped. Open tasks are the
`Partial` and `Open` contract rows, and nothing else.

Counts below are the modules as they stand: `owned` 7, `recorder` 13,
`resolve` 57, `scene` 33, `stream` 5 -- 115 in the library -- plus
`classifier` 10, `compression` 4, `lua_roundtrip` 2,
`stream_roundtrip` 1, and two doctests of which one is `ignore`d. 133
with every feature.

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
- [x] T1.13b `set_attribute` refuses an uncreated handle. See T6.7.
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
- [x] T1.16b `recursive` delete. See T6.11.
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
- [x] T3.2 `geometry_binding`. Evidence: `resolve::tests`.
- [x] T3.3 `render_outputs`. Evidence: `resolve::tests`.
- [x] T3.4 `instance_sources`. Evidence: `resolve::tests`.
- [x] T3.5a **Motion-sampled transforms fail loudly.** A motion-blurred
      scene no longer resolves to its static pose.
      Evidence: `resolve::tests::a_motion_sampled_transform_is_an_error`,
      `motion_samples_of_other_attributes_do_not_block_resolution`.
      Spec: R13.
- [x] T3.6 Two-screen fixture.
      Evidence: `resolve::tests::multiple_screens_yield_one_output_each`.
- [x] T3.7 Test that a `MatrixF32` transform is skipped deliberately.
      Evidence: `resolve::tests::a_non_f64_matrix_is_skipped_not_reinterpreted`.
- [x] T3.8 Multi-parent is a typed error, not the first parent's chain.
      Evidence: `resolve::tests::more_than_one_parent_is_an_error`.
      Spec: R11.
- [x] T3.9 A cycle is a typed error, and the test asserts it. The
      previous `a_cycle_terminates` asserted nothing.
      Evidence: `resolve::tests::a_cycle_is_an_error`,
      `resolve::tests::a_cycle_is_an_error_for_bindings_too`. Spec: R9.
- [x] T3.10 Inherit `geometryattributes` from ancestor transforms, with
      `priority`. Evidence:
      `resolve::tests::a_binding_on_an_ancestor_transform_is_inherited`,
      `the_nearest_binding_wins_at_equal_priority`,
      `priority_beats_proximity`;
      `recorder::tests::connect_records_the_priority_argument`.
      Spec: R12.
- [x] T3.5b `world_transform_at`, `motion_times` and
      `world_transform_samples` answer only at a recorded sample. An
      unsampled time is an error naming the times that exist. Evidence:
      `resolve::tests` motion cases. Spec: R13.

      This entry used to justify that with "element-wise interpolation
      of a matrix is wrong for anything with a rotation in it". That is
      false, and measurable: 3Delight's own rotation blur fits
      component-wise interpolation (rms 0.002) far better than slerp
      (0.021), and three samples fit piecewise-linear (0.003) not
      quadratic (0.026). The reason to keep these three exact is that
      "what did the caller record" is a different question from "where
      is it mid-shutter" -- which
      `world_transform_interpolated_at` now answers, on that measured
      model. See T9.17.
- [x] T3.11 Per-path transforms for an instanced node: `placements`.
      T3.8 refused the case; this answers it. The row asked only for a
      transform per path -- `Vec<[f64; 16]>` or a path-qualified handle
      -- but rendering showed the paths also gather **different
      attributes**: `visibility 1` on one parent and `visibility 0` on
      the other draws one copy. So a per-path transform with a shared
      binding would have been a new silent wrong answer, and
      `Placement` carries the binding too.
- [x] T3.12 `priority`'s direction and tie-break are in `nsi.pdf`, not
      merely plausible: highest wins, then closest to the geometry. The
      question was answered by reading the specification. See
      `research.md` D8.
- [x] T3.13 Every shader slot honours its connection's `priority`.
      Evidence: `resolve::tests::a_surfaceshader_connection_priority_wins`.

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
      Evidence: `resolve::tests::every_attributes_node_on_the_path_is_gathered`.
      Spec: R12.
- [x] T5.2 Include `.root` in the chain.
      Evidence: `resolve::tests::a_binding_on_the_root_is_gathered`.
- [x] T5.3 The shader agrees with the gathered order.
      Evidence: `resolve::tests::the_shader_agrees_with_the_gathered_order`.
- [x] T5.4 Classify `displacementshader` and `volumeshader`.
      Evidence: `resolve::tests::displacement_and_volume_shaders_resolve_too`.
- [x] T5.5 The two setters replace each other per name.
      Evidence: `scene::tests::a_static_set_clears_the_motion_samples_of_that_name`,
      `a_sampled_set_clears_the_static_value_of_that_name`. Spec: R7.
- [x] T5.6 Re-`create` with a different type is an error.
      Evidence: `scene::tests::recreating_with_a_different_type_is_an_error`.
      Spec: R17.
- [x] T5.7 A detached node is an error, and a prototype is not detached.
      Evidence: `resolve::tests::a_detached_node_is_an_error_not_identity`,
      `resolve::tests::an_instancing_prototype_is_not_detached`. Spec: R20.
- [x] T5.8 Escape strings on replay.
      Evidence: `stream::tests::a_string_cannot_inject_a_statement`,
      `a_recorded_scene_with_hostile_strings_stays_one_statement_a_line`.
      Spec: R21.
- [x] T5.9 `RecordError`, `#[non_exhaustive]`, as the one recorder error.
- [ ] T5.10 Per-path transforms for an instanced node, and an
      `instances` node's `transformationmatrices` / `modelindices`.
      Gate: two `Open` rows in `contracts/resolution.md`.
- [ ] T5.11 Honour `INTERPOLATE_LINEAR` on a sampled transform.
- [x] T5.12 Sample times of an arbitrary attribute, for deforming
      geometry whose `P` is sampled under a static transform.
      `attribute_times` and `attribute_samples`. A mesh deforming under
      a static transform had no motion times at all, so a backend
      checking `motion_times` rendered it without deformation. An
      unknown handle is `ResolveError::UnknownHandle` rather than an
      empty list, which would have read as "static". Evidence: five
      tests in `resolve::tests`, each falsified.
- [ ] T5.13 Decide the attribute vocabulary: legacy or the documentation
      draft. `sourcemodels` to `objects` is the rename that fails
      silently. See `research.md` D10.

## Found By Review, Rounds 3 And 4

- [x] T6.1 A prototype has no single world transform; attributes still
      gather through the instancer.
      Evidence: `resolve::tests::a_prototype_has_no_single_world_transform`,
      `an_instancing_prototype_is_not_detached`. Spec: R20.
- [x] T6.2 A prototype that is *also* placed directly resolves by that
      path. The instancer was being reported as a second parent, which
      made a legal scene unresolvable.
      Evidence: `resolve::tests::a_prototype_placed_directly_resolves_by_that_path`.
- [x] T6.3 Reserved handles: never declared on replay, not deletable,
      attributes without a `create`.
      Evidence: `stream::tests::the_reserved_handles_are_never_declared`,
      `scene::tests::the_reserved_nodes_cannot_be_deleted`,
      `the_reserved_handles_take_attributes_without_a_create`. Spec: R23.
- [x] T6.4 Instance sources ordered by their `index` argument.
      Evidence: `resolve::tests::instance_sources_are_ordered_by_their_index_attribute`.
      Spec: R24.
- [x] T6.5 Index the graph; resolution is linear.
      Evidence: measured, 20k meshes 655 ms debug / 50 ms release.
      `Scene`'s fields are private and it is `#[non_exhaustive]`.
- [x] T6.6 `Recorder::into_scene`, and the `Scene` read accessors.
- [x] T6.7 `set_attribute` refuses an uncreated handle. It fabricated a
      typeless node that then satisfied the `connect` guard, silently
      undoing T1.13a.
      Evidence: `scene::tests::setting_an_attribute_on_an_uncreated_handle_is_an_error`.
- [x] T6.8 Classify every `<connection>` the specification declares.
      `lensshader`, `backgroundlayer`, `bounds`,
      `visibility.set.subsurface` and `exclusiveshading` were rejected,
      so an exporter using any of them could not record at all.
      Evidence: `classifier::every_connection_the_specification_declares_is_classified`.
- [x] T6.9 `Scene::relative_transform`, so a prototype's subtree
      resolves in the space the instance matrix applies to.
      Evidence: `resolve::tests::a_prototype_subtree_resolves_relative_to_the_prototype`,
      `relative_transform_rejects_a_node_off_the_chain`. Spec: R29.
- [x] T6.10 `Scene::instance_transforms`, pairing `transformationmatrices`
      with the prototype each draws through `modelindices`, and honouring
      `disabledinstances`.
      Evidence: `resolve::tests::instances_pair_their_matrix_with_their_prototype`,
      `a_negative_model_index_is_not_rendered`,
      `disabled_instances_are_omitted`. Spec: R30.
- [x] T6.11 Recursive `delete`, with ɴsɪ's two exceptions.
      Evidence: `scene::tests::a_recursive_delete_takes_the_network_with_it`,
      `a_recursive_delete_spares_a_node_used_elsewhere`,
      `strength_blocks_a_recursive_delete`, `a_plain_delete_is_not_recursive`.
      Spec: R31.
- [x] T6.12 Lua and compressed stream output, behind features.
      Evidence: `lua_roundtrip`, `compression`, and `contracts/output.md`.
      Spec: R25-R28.
- [ ] T6.13 Per-path world transforms for an instanced node. T6.9 and
      T6.10 make instancing usable without them; this would make it
      automatic.
- [ ] T6.14 `INTERPOLATE_LINEAR` on a sampled transform.
- [ ] T6.15 Sample times of an arbitrary attribute, for deforming
      geometry whose `P` is sampled under a static transform.
- [ ] T6.16 Decide the attribute vocabulary: legacy or documentation
      draft. See `research.md` D10.
- [x] T6.17 Test modules moved to their own files, per the workspace
      rule that source files do not grow inline `#[cfg(test)]` blocks.
      No source file is over 900 lines.
- [x] T6.18a The release chain is prepared: `nsi-trait` 0.4.0,
      `nsi-ffi-wrap` 0.10.0, every dependent repinned. `publish
      --dry-run` now fails on the missing upstream *version* rather than
      on a trait bound, which is the correct pre-publish state.
- [ ] T6.18b Publish, in order. Irreversible and needs credentials, so
      it is a person's action. See `plan.md`.

## Found By Review, Round 8

- [x] T8.1 `OwnedArg::from_param` copied every scalar, but the C call
      hands the renderer `count = len / array_length` elements -- so a
      run that does not divide was kept here and dropped there, and this
      crate's own stream then failed the count `nsi-parse` checks.
      Evidence: `owned::tests::an_array_len_run_is_rounded_down_as_the_c_call_does`,
      `a_tuple_array_len_run_is_rounded_down_too`.
- [x] T8.2 A shader-network edge's `to_attr` is its *port* name, so it
      shared an index bucket with the class of that name and resolved as
      one. Evidence:
      `resolve::tests::a_shader_network_port_does_not_resolve_as_its_namesake_class`,
      `a_port_named_like_a_binding_does_not_bind`.
- [x] T8.3 `ClassifyError` became unreachable when the destination set
      opened, but the type, the error variant and three spec files still
      described a rejection that no longer happens. `classify` is now
      total and returns `EdgeKind`.
- [x] T8.4 Contract rows cited test modules that the test-file split had
      renamed, so `cargo test -- binding_tests` ran nothing.
- [x] T8.5 `create` refuses a reserved handle, as 3Delight does. It kept
      a node replay then dropped, so the scene changed on its own first
      round trip. Evidence:
      `scene::tests::the_reserved_handles_cannot_be_created`.
- [x] T8.6 `nsi-trait` gains an `include` whitelist. `cargo package`
      was shipping a `.claude/audit` log a hook had written into the
      crate. All four crates now package clean.

## Attribute-Level Precedence

The one `Open` row that could hand a backend a *wrong answer* rather
than a missing feature, closed because `nsi-moonray` is being written
against this API now.

- [x] T9.1 `Scene::attribute_value(geometry, name)` applies ɴsɪ's two
      attribute-level rules that `Binding::attributes` cannot express:
      `ATTR.priority`, and a per-ray `visibility.<ray>` beating the
      default `visibility` at equal priority. `geometry_binding` orders
      *nodes*; this orders the definitions on them. Both now share
      `gathered_attributes`, so they cannot disagree about rank.
      Evidence: eleven tests in `resolve::tests`, each falsified by
      breaking the rule it guards.
- [x] T9.2 `RAY_TYPES` is the eight suffixes the specification lists,
      not any `visibility.*`. `visibility.set.subsurface` is a
      *connection* to a `set` node, and treating it as a more specific
      `visibility` would have ranked a connection against a flag.
      Evidence: `resolve::tests::visibility_set_subsurface_is_not_a_ray_type`.
- [x] T9.3 `priority_value` reads `int` only. Reinterpreting another
      layout would let a stray float silently reorder the scene.
      Evidence: `resolve::tests::a_non_integer_priority_is_ignored`.
- [x] T9.4 Specificity-before-proximity confirmed against 3Delight.
      Probes `E` and `E2`: a distant `visibility.camera` beats a nearer
      `visibility`. The row is `Covered`, not an assumption.
- [x] T9.5 **The connection `priority` on a `geometryattributes` edge is
      inert in 3Delight, and this crate had sorted by it since round 2.**
      Round 10 rendered it: `visibility 0` near, `visibility 1` far with
      `"priority" 10` on the connection, and 3Delight leaves the object
      invisible -- proximity wins. Six scenes agree, and a priority on a
      *shader* connection still works, which is the distinction ɴsɪ's
      "(for shaders, essentially)" draws and its `priority` entry at
      line 552 does not. `gathered_attributes` no longer reads it;
      `shader_on` still does. The test that had pinned the opposite was
      corrected to what the renderer does, and is now
      `a_geometryattributes_connection_priority_does_not_reorder`.
- [x] T9.6 `RAY_TYPES` is public: a backend building a visibility mask
      needs exactly those eight, and a second copy would drift.
- [x] T9.7 `priority_value` reads `int` only. The `I64` arm shipped in
      `e8839cc` with no test covering it; 3Delight ignores an `int64`
      priority (while accepting `int64` for the value itself), so the
      arm would have ranked a node the renderer does not. Evidence:
      `resolve::tests::an_int64_priority_is_ignored`.
- [x] T9.8 A node setting `ATTR.priority` without `ATTR` defines
      `ATTR` at its default, and ranks on that priority: rendered, such
      a node two levels up beats a `visibility 0` on the primitive's
      own node, so the crate was returning a **wrong** winner and not
      merely a missing one. Closed by widening rather than by carrying
      ɴsɪ's per-attribute defaults, which are a renderer's to know:
      `AttributeValue::arg` is now `Option`, `None` meaning *the
      default of `AttributeValue::name`*, and `name` carries the
      attribute that won -- which `arg.name` used to. Evidence: seven
      tests in `resolve::tests`, three mutations, seven probe scenes.
- [x] T9.9 `shaderattributes` nodes resolve, under their own rule.
      They had classified since the classifier landed and nothing read
      them. `shader_attribute_value` takes the nearest and stops:
      proximity only, no `ATTR.priority`, no per-ray fallback -- ɴsɪ
      gives this container none of those, and reusing the
      `geometryattributes` rule would have invented them. The two
      containers are gathered by their own `EdgeKind` and proven not to
      cross. Evidence: five tests in `resolve::tests`, each falsified.
- [x] T9.11 The primitive's own attributes are the top-ranked source of
      a shader attribute. ɴsɪ gives "the highest priority ... to
      attributes set directly on the geometric primitive", and T9.9
      dropped that clause from its own `Open` row while marking the row
      `Covered` -- so the crate answered with a container's value where
      3Delight answers with the mesh's. Rendered in both directions, and
      against a `tint.priority`. Evidence:
      `resolve::tests::the_geometrys_own_shader_attribute_outranks_every_container`.
- [x] T9.12 Three shader-attribute orderings were unpinned -- a reversed
      source list, a reversed within-level order, and dropping `.root`
      edges each left the suite green. Evidence:
      `resolve::tests::shader_attribute_sources_are_in_precedence_order`,
      `the_first_connected_shader_attribute_wins_at_one_level`.
- [x] T9.10 Attributes provided through a `set`, for **both**
      containers. 3Delight honours `geometryattributes` on a set as well
      as `shaderattributes`, so `geometry_binding` and `attribute_value`
      shared the blind spot and it was untracked until round 12. Rather
      than leave it open on "the specification does not say", the
      ranking was rendered: a set sits between the geometry and its
      first transform, direct membership only, first membership wins,
      and `ATTR.priority` still outranks all of it. Evidence: eleven
      tests in `resolve::tests`; see `research.md` D13.
- [x] T9.13 A set whose member is a *transform on the chain* is gathered
      too. T9.10 walked `SetMember` from the geometry alone, so a scene
      putting the set on the transform -- the ordinary way to assign a
      whole group -- resolved to nothing. The walk is per chain node,
      deduplicated to the nearest occurrence, because one set may hold
      several nodes of the chain. Evidence:
      `resolve::tests::a_set_on_a_transform_in_the_chain_is_gathered`,
      `a_set_holding_two_chain_nodes_is_one_source`.
- [x] T9.14 `attribute_times` and `attribute_samples` answer `Ok([])`
      for `.root` and `.global`. They are reserved and exist without
      being created, so `UnknownHandle` was both wrong and unstable:
      the answer flipped to `Ok` once any unrelated attribute was set on
      `.root`. Evidence:
      `resolve::tests::a_reserved_handle_has_no_samples_rather_than_being_unknown`.
- [x] T9.15 `attribute_samples_are_time_ordered` never called
      `attribute_samples`; reversing that function left it green. It now
      asserts on both.

## Create Arguments

- [x] T9.16 `create`'s arguments are inert, and dropping them is right.
      The specification reads as though they were part of a node's
      identity -- "does nothing if all other parameters match the call
      which created that node" -- but it also says none are defined, and
      rendering settles it: 3Delight accepts a repeat with a *different*
      create argument and refuses one with a different **type**
      (`E6002`). The crate already refused the type; the row was open on
      a question that a single render answered. Evidence:
      `recorder::tests::create_arguments_are_inert_but_the_type_is_not`.

## Interpolating A Transform

- [x] T9.17 `world_transform_interpolated_at` holds the end sample
      outside the sampled range, rather than refusing. The first
      version refused, and its doc argued that clamping "would answer
      for a moment the caller never described" -- as though the renderer
      agreed. It does not: samples at t=0 and t=1 under a shutter of
      [-1, 2] leave **zero** alpha beyond the sampled positions, with a
      peak at each end 2.7x the swept middle. The caller did describe
      that moment; they opened the shutter there. Refusing would have
      failed a backend on a scene 3Delight renders.
- [x] T9.18 A chain mixing static and sampled nodes, and a node with a
      single sample. Dropping static nodes from the interpolated walk
      left all 180 tests green, and a single-sample node found no
      bracketing pair and was refused while `world_transform_at`
      answered -- the two accessors disagreed.
- [x] T9.19 A `-0.0` normalising `+ 0.0` was added to the interpolated
      path and removed again: the clamp subsumes it, and removing it
      left the suite green. A line that guards nothing is worse than no
      line, because it reads as protection.

## Foreign Parameters

- [x] T9.20 `OwnedArg::from_param` is `pub(crate)`. It was `pub` and
      carried two paths nothing could reach -- a panic when
      `as_c_param` returns `None`, and an empty `f32` array for
      `Type::Invalid` -- both reachable only by a *foreign*
      `ParamValue`, and only because the function was public. Narrowing
      makes them unreachable by construction rather than by argument,
      which is cheaper than a `Result` every internal caller would
      unwrap for a case that cannot arise. The `Invalid` arm is now
      `unreachable!` rather than a silent empty array: it would have
      recorded a different argument instead of refusing. Verified from
      outside the workspace: `E0624`.

## Resolving Along A Placement

- [x] T9.21 `placements_at`, `attribute_value_along` and
      `shader_attribute_value_along`. Between them, `placements`
      refusing a sampled node and `world_transform_interpolated_at`
      refusing a multi-parent one left a *moving instanced* geometry
      with no answer anywhere -- and instanced moving geometry is
      ordinary content, not a corner. The path-based internals already
      existed (`gathered_along`, `compose_along`); this exposes them.
      `placements` and `placements_at` share `placements_with`, and
      `attribute_value` shares `resolve_attribute` with its along-path
      form, so neither pair can drift.
