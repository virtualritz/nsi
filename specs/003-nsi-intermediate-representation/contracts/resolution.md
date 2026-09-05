# Contract: Graph Resolution

## Scope

Covers turning ɴsɪ's scene-graph semantics into flat facts: world
transforms, gathered attributes, output chains and instance sources.
Does not cover mapping those facts onto a renderer.

## Why This Contract Exists

Every rule here is stated in `nsi.pdf` and was, at some point, guessed at
instead. Two review rounds found this surface answering a question ɴsɪ
had already answered -- and answering it plausibly, which is why nothing
failed. A wrong world transform renders. A wrong material renders.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| No transforms yields identity | Covered | `resolve.rs` `world_transform` | `resolve::tests::a_node_with_no_transforms_is_identity` | -- |
| A parent transform applies to its child | Covered | `resolve.rs` `world_transform` | `resolve::tests::a_single_transform_applies_to_its_child` | -- |
| Nested transforms accumulate | Covered | `resolve.rs` `mul` | `resolve::tests::nested_transforms_compose` | -- |
| A child's matrix applies before its parent's | Covered | `resolve.rs` `mul` doc + call order | `resolve::tests::child_transform_applies_before_parent` (non-commuting pair) | -- |
| A transform's own matrix is included | Covered | `resolve.rs` `local_transform` | `resolve::tests::a_transforms_own_matrix_is_included` | -- |
| A cycle is a typed error, not a hang and not an answer | Covered | `resolve.rs` `chain` visited set | `resolve::tests::a_cycle_is_an_error`; `binding_tests::a_cycle_is_an_error_for_bindings_too` | Only a two-node cycle entered from inside it is proven. A self-loop and a node hanging off a cycle are correct by reading, not by test. |
| More than one parent is a typed error | Covered | `resolve.rs` `chain` | `resolve::tests::more_than_one_parent_is_an_error`, naming both parents | -- |
| A detached node is an error, not identity | Covered | `resolve.rs` `chain` `Detached` | `resolve::tests::a_detached_node_is_an_error_not_identity`, `detachment_is_reported_at_the_node_that_fails_to_reach_root` | -- |
| An instancing prototype is not detached | Covered | `resolve.rs` `chain` follows `InstanceSource` | `binding_tests::an_instancing_prototype_is_not_detached` | -- |
| A non-`f64` matrix is ignored, not reinterpreted | Covered | `resolve.rs` `matrix_of` matches `OwnedData::F64` only | `resolve::tests::a_non_f64_matrix_is_skipped_not_reinterpreted` | -- |
| Motion-sampled transforms compose per sample | Covered | `resolve.rs` `world_transform_at`, `local_transform_at` | `resolve::tests::a_sampled_chain_resolves_per_sample`, `a_static_parent_composes_with_a_sampled_child` | -- |
| A static node contributes at every time | Covered | `resolve.rs` `local_transform_at` | `resolve::tests::a_static_parent_composes_with_a_sampled_child`, `a_static_chain_has_no_motion_times` | -- |
| Sample times are the chain's union, sorted and deduplicated | Covered | `resolve.rs` `motion_times` | `resolve::tests::motion_times_are_the_union_of_the_chain`, whose deeper node's time sorts last so an unsorted merge fails | -- |
| Only *transform* samples count as motion | Covered | `resolve.rs` `motion_times` filters on `transformationmatrix` | `resolve::tests::a_static_chain_has_no_motion_times`, which carries an unrelated sampled attribute | -- |
| A time between samples is an error, never an interpolation | Covered | `resolve.rs` `ResolveError::MissingSampleAtTime` | `resolve::tests::a_time_between_samples_is_an_error_not_an_interpolation`, `a_chain_sampled_at_different_times_is_an_error` | -- |
| `world_transform` still refuses a sampled chain | Covered | `resolve.rs` `has_motion_transform` | `resolve::tests::a_motion_sampled_transform_is_an_error`, `motion_samples_of_other_attributes_do_not_block_resolution` | -- |
| `attributes` dissolves to a shader | Covered | `resolve.rs` `geometry_binding` | `binding_tests::dissolves_attributes_to_a_shader` | -- |
| Unbound geometry has no binding | Covered | `resolve.rs` `geometry_binding` | `binding_tests::unbound_geometry_has_no_binding` | -- |
| `attributes` without a shader still binds | Covered | `resolve.rs` `geometry_binding` | `binding_tests::attributes_without_a_shader_still_bind` | -- |
| One `attributes` node fans out to many shapes | Covered | `resolve.rs` resolves per geometry | `binding_tests::one_attributes_node_fans_out_to_every_shape` | -- |
| A binding on an ancestor transform is inherited | Covered | `resolve.rs` `geometry_binding` searches the whole `chain` | `binding_tests::a_binding_on_an_ancestor_transform_is_inherited` | -- |
| A binding on `.root` is gathered | Covered | `resolve.rs` `chain` includes `ROOT` | `binding_tests::a_binding_on_the_root_is_gathered` | -- |
| **Every** `attributes` node on the path is kept | Covered | `resolve.rs` `geometry_binding` returns `Vec` | `binding_tests::every_attributes_node_on_the_path_is_gathered`, visibility on one node and the shader on another | -- |
| Highest priority wins, then proximity | Covered | `resolve.rs` `sort_by` | `binding_tests::priority_beats_proximity`, `the_nearest_binding_wins_at_equal_priority` | -- |
| A shader connection's own priority wins | Covered | `resolve.rs` `shader_on` | `binding_tests::a_surfaceshader_connection_priority_wins` | -- |
| The shader agrees with the gathered order | Covered | `resolve.rs` `shader_on` `min_by` on `(priority, rank)` | `binding_tests::the_shader_agrees_with_the_gathered_order`; a last-wins `max_by` keyed on depth returns the other node's shader | -- |
| All three shader slots resolve | Covered | `resolve.rs` `shader_on` per `EdgeKind` | `binding_tests::displacement_and_volume_shaders_resolve_too` | -- |
| The output chain resolves end to end | Covered | `resolve.rs` `render_outputs` | `output_tests::resolves_the_whole_output_chain` | -- |
| A screen with no layers still resolves | Covered | `resolve.rs` `render_outputs` | `output_tests::a_screen_without_layers_still_resolves` | -- |
| Layer order is connection order | Covered | `resolve.rs` iterates `edges` in order | `output_tests::multiple_layers_keep_connection_order` | -- |
| A layer may fan out to several drivers | Covered | `resolve.rs` `render_outputs` | `output_tests::a_layer_may_have_several_drivers` | -- |
| Multiple screens yield multiple outputs | Covered | `resolve.rs` iterates every `Screen` edge | `output_tests::multiple_screens_yield_one_output_each` | -- |
| Instance sources resolve in connection order | Covered | `resolve.rs` `instance_sources` | `instance_tests::resolves_instance_source_models` | -- |
| An instanced node resolves to one transform per path | Open | `resolve.rs` `chain` rejects multi-parent | None | ɴsɪ's lightweight instancing has one world transform per path. The error is the honest stop-gap; decide the shape -- `Vec<[f64; 16]>` or a path-qualified handle -- then test a two-parent fixture. |
| `transformationmatrices` / `modelindices` on an `instances` node | Open | Not resolved; `matrix_of` requires exactly 16 values | None | ɴsɪ gives an `instances` node "a transformation matrix for each instance" and an optional model selector. `instance_sources` returns prototypes only, so a backend cannot place the instances. |
| `INTERPOLATE_LINEAR` is honoured for a sampled transform | Open | `local_transform_at` requires an exact sample | None | ɴsɪ has a per-argument flag saying linear interpolation is intended. Where it is set, interpolating is the caller's stated wish rather than this crate's guess. |
| Deforming geometry (`P` sampled) is resolvable | Open | `motion_times` is transform-only | None | A mesh whose `P` is sampled under a static transform reports no motion times. A backend needs the sample times of an arbitrary attribute. |
| Node types are never consulted | Partial | `resolve.rs` reads attributes, not types | Implicit in every test | Under the ɴsɪ documentation draft, `sourcemodels` is renamed `objects`, which would make `objects` mean two different things depending on the destination node's type. See `research.md` D10. |

## Invariants

- ɴsɪ is row-major, row-vector: `p * child * parent`. Composition is
  `mul(child, parent)`.
- Resolution is pure. It reads a `Scene` and allocates results; it never
  mutates.
- `geometry_binding` returns attributes **handles**, not their contents,
  because visibility encoding is renderer-specific.
- Every walk up the `objects` hierarchy goes through `Scene::chain`, so
  the scenes with no single answer are rejected in one place.
- Nothing is ever interpolated. A backend knows the right decomposition;
  this crate would have to guess one.

## Failure Modes

Every variant of `ResolveError` is a scene ɴsɪ permits and this crate
refuses to guess about. None is a hang, and none is a plausible wrong
matrix.

- **`Cycle`** -- the chain revisits a node. No correct answer exists.
- **`MultipleParents`** -- ɴsɪ's lightweight instancing, which has one
  world transform per path rather than one overall.
- **`Detached`** -- the node never reaches `.root`, so it "won't affect
  the render in any way".
- **`MissingSampleAtTime`** -- a sampled node has no sample there, and
  interpolating would bake one renderer's decomposition into all of them.
- **A missing transform attribute** contributes identity rather than
  failing. A transform node without a matrix is legal in ɴsɪ.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --lib resolve`
- To close the instancing rows: a two-parent fixture asserting one
  transform per path, and an `instances` node carrying
  `transformationmatrices`.
- To close the interpolation row: a fixture setting
  `INTERPOLATE_LINEAR` on a sampled `transformationmatrix`.
- To close the deforming row: an API for the sample times of a named
  attribute, then a sampled-`P` fixture.
