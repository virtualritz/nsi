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
| No transforms yields identity | Covered | `resolve/mod.rs` `world_transform` | `resolve::tests::a_node_with_no_transforms_is_identity` | -- |
| A parent transform applies to its child | Covered | `resolve/mod.rs` `world_transform` | `resolve::tests::a_single_transform_applies_to_its_child` | -- |
| Nested transforms accumulate | Covered | `resolve/mod.rs` `mul` | `resolve::tests::nested_transforms_compose` | -- |
| A child's matrix applies before its parent's | Covered | `resolve/mod.rs` `mul` doc + call order | `resolve::tests::child_transform_applies_before_parent` (non-commuting pair) | -- |
| A transform's own matrix is included | Covered | `resolve/mod.rs` `local_transform` | `resolve::tests::a_transforms_own_matrix_is_included` | -- |
| A cycle is a typed error, not a hang and not an answer | Covered | `resolve/mod.rs` `chain` visited set | `resolve::tests::a_cycle_is_an_error`; `resolve::tests::a_cycle_is_an_error_for_bindings_too` | Only a two-node cycle entered from inside it is proven. A self-loop and a node hanging off a cycle are correct by reading, not by test. |
| More than one parent is a typed error | Covered | `resolve/mod.rs` `chain` | `resolve::tests::more_than_one_parent_is_an_error`, naming both parents | -- |
| A detached node is an error, not identity | Covered | `resolve/mod.rs` `chain` `Detached` | `resolve::tests::a_detached_node_is_an_error_not_identity`, `detachment_is_reported_at_the_node_that_fails_to_reach_root` | -- |
| An instancing prototype gathers attributes through its instancer | Covered | `resolve/mod.rs` `chain` follows `InstanceSource` | `resolve::tests::an_instancing_prototype_is_not_detached` | -- |
| A prototype has no single world transform | Covered | `resolve/mod.rs` `transform_chain` refuses to pass an `instances` node; `ResolveError::Instanced` | `resolve::tests::a_prototype_has_no_single_world_transform`. Answering with the instancer's own matrix put every instance in the same wrong place. | -- |
| A prototype of two instancers is ambiguous | Covered | `resolve/mod.rs` `linked_chain` counts instancers as parents | `resolve::tests::a_prototype_of_two_instancers_is_ambiguous` | -- |
| A non-`f64` matrix is ignored, not reinterpreted | Covered | `resolve/mod.rs` `matrix_of` matches `OwnedData::F64` only | `resolve::tests::a_non_f64_matrix_is_skipped_not_reinterpreted` | -- |
| Motion-sampled transforms compose per sample | Covered | `resolve/mod.rs` `world_transform_at`, `local_transform_at` | `resolve::tests::a_sampled_chain_resolves_per_sample`, `a_static_parent_composes_with_a_sampled_child` | -- |
| A static node contributes at every time | Covered | `resolve/mod.rs` `local_transform_at` | `resolve::tests::a_static_parent_composes_with_a_sampled_child`, `a_static_chain_has_no_motion_times` | -- |
| Sample times are the chain's union, sorted and deduplicated | Covered | `resolve/mod.rs` `motion_times` | `resolve::tests::motion_times_are_the_union_of_the_chain`, whose deeper node's time sorts last so an unsorted merge fails | -- |
| Only *transform* samples count as motion | Covered | `resolve/mod.rs` `motion_times` filters on `transformationmatrix` | `resolve::tests::a_static_chain_has_no_motion_times`, which carries an unrelated sampled attribute | -- |
| A time between samples is an error, never an interpolation | Covered | `resolve/mod.rs` `ResolveError::MissingSampleAtTime` | `resolve::tests::a_time_between_samples_is_an_error_not_an_interpolation`, `a_chain_sampled_at_different_times_is_an_error` | -- |
| `world_transform` still refuses a sampled chain | Covered | `resolve/mod.rs` `has_motion_transform` | `resolve::tests::a_motion_sampled_transform_is_an_error`, `motion_samples_of_other_attributes_do_not_block_resolution` | -- |
| `attributes` dissolves to a shader | Covered | `resolve/mod.rs` `geometry_binding` | `resolve::tests::dissolves_attributes_to_a_shader` | -- |
| Unbound geometry has no binding | Covered | `resolve/mod.rs` `geometry_binding` | `resolve::tests::unbound_geometry_has_no_binding` | -- |
| `attributes` without a shader still binds | Covered | `resolve/mod.rs` `geometry_binding` | `resolve::tests::attributes_without_a_shader_still_bind` | -- |
| One `attributes` node fans out to many shapes | Covered | `resolve/mod.rs` resolves per geometry | `resolve::tests::one_attributes_node_fans_out_to_every_shape` | -- |
| A binding on an ancestor transform is inherited | Covered | `resolve/mod.rs` `geometry_binding` searches the whole `chain` | `resolve::tests::a_binding_on_an_ancestor_transform_is_inherited` | -- |
| A binding on `.root` is gathered | Covered | `resolve/mod.rs` `chain` includes `ROOT` | `resolve::tests::a_binding_on_the_root_is_gathered` | -- |
| **Every** `attributes` node on the path is kept | Covered | `resolve/mod.rs` `geometry_binding` returns `Vec` | `resolve::tests::every_attributes_node_on_the_path_is_gathered`, visibility on one node and the shader on another | -- |
| Nearest the geometry wins | Covered | `resolve/mod.rs` `gathered_attributes` sorts on depth, then connection order | `resolve::tests::the_nearest_binding_wins_at_equal_priority` | -- |
| A `geometryattributes` connection's `priority` is **inert** | Covered | `resolve/mod.rs` `gathered_attributes` does not read it | `resolve::tests::a_geometryattributes_connection_priority_does_not_reorder`; restoring the priority to the sort key reddens it. Observed in 3Delight 2.9: six probe scenes (`D`, `D2`, `D3`, `V1`, `S1`, `S3`) put `"priority" 10` on the far node's connection and the **near** node still wins, for visibility and for the shader alike. Moving the same 10 onto the node as `visibility.priority` flips it (`B`) | -- |
| A shader connection's own priority wins | Covered | `resolve/mod.rs` `shader_on` | `resolve::tests::a_surfaceshader_connection_priority_wins` | -- |
| The shader agrees with the gathered order | Covered | `resolve/mod.rs` `shader_on` `min_by` on `(priority, rank)` | `resolve::tests::the_shader_agrees_with_the_gathered_order`; a last-wins `max_by` keyed on depth returns the other node's shader | -- |
| All three shader slots resolve | Covered | `resolve/mod.rs` `shader_on` per `EdgeKind` | `resolve::tests::displacement_and_volume_shaders_resolve_too` | -- |
| The output chain resolves end to end | Covered | `resolve/mod.rs` `render_outputs` | `resolve::tests::resolves_the_whole_output_chain` | -- |
| A screen with no layers still resolves | Covered | `resolve/mod.rs` `render_outputs` | `resolve::tests::a_screen_without_layers_still_resolves` | -- |
| Layer order is connection order | Covered | `resolve/mod.rs` iterates `edges` in order | `resolve::tests::multiple_layers_keep_connection_order` | -- |
| A layer may fan out to several drivers | Covered | `resolve/mod.rs` `render_outputs` | `resolve::tests::a_layer_may_have_several_drivers` | -- |
| Multiple screens yield multiple outputs | Covered | `resolve/mod.rs` iterates every `Screen` edge | `resolve::tests::multiple_screens_yield_one_output_each` | -- |
| Instance sources are ordered by their `index` argument | Covered | `resolve/mod.rs` `instance_sources` sorts on `Edge::index` | `resolve::tests::instance_sources_are_ordered_by_their_index_attribute`, whose connection order differs from its index order | -- |
| Instance sources without an index keep connection order | Covered | `resolve/mod.rs` sort key is `(index, order)` | `resolve::tests::resolves_instance_source_models` | -- |
| A prototype's subtree resolves relative to an ancestor | Covered | `resolve/mod.rs` `relative_transform` | `tests::a_prototype_subtree_resolves_relative_to_the_prototype`, `relative_transform_rejects_a_node_off_the_chain` | -- |
| Composing *past* an instancer is refused | Covered | `resolve/mod.rs` `linked_chain` records the instancer hop; `relative_transform` refuses to cross one | `tests::relative_transform_refuses_to_cross_an_instancer`. Composing through folded in the instancer's own matrix and left out the per-instance one -- a plausible wrong answer for the query the method exists to serve | -- |
| A ragged matrix buffer is refused | Covered | `resolve/mod.rs` `instance_transforms`, `ResolveError::MalformedInstanceMatrices` | `resolve::tests::a_ragged_matrix_buffer_is_an_error`; it used to keep the whole matrices and drop the remainder | -- |
| A model index matching no prototype is refused | Covered | `resolve/mod.rs` `ResolveError::UnknownModelIndex` | `resolve::tests::a_model_index_matching_no_prototype_is_an_error`; it used to drop the instance | -- |
| A prototype placed directly resolves by that path | Covered | `resolve/mod.rs` `linked_chain` counts an instancer as a parent only when there is no direct one | `resolve::tests::a_prototype_placed_directly_resolves_by_that_path`; reporting the instancer as a second parent made a legal scene unresolvable | -- |
| Instances pair their matrix with their prototype | Covered | `resolve/mod.rs` `instance_transforms`, matching `modelindices` against the connection `index` | `resolve::tests::instances_pair_their_matrix_with_their_prototype`, whose indices are deliberately not their positions; `a_negative_model_index_is_not_rendered`; `disabled_instances_are_omitted` | -- |
| An instanced node resolves to one transform per path | Open | `resolve/mod.rs` `chain` rejects multi-parent | None | ɴsɪ's lightweight instancing has one world transform per path. The error is the honest stop-gap; decide the shape -- `Vec<[f64; 16]>` or a path-qualified handle -- then test a two-parent fixture. |

| `INTERPOLATE_LINEAR` governs a sampled transform | Covered | Nothing reads the flag for this, deliberately | The row's premise was wrong, and rendering shows it. Transform samples interpolate **by default**: a quad translated `-3 -> +3` over the shutter smears (peak alpha `0.183` static, `0.117` moving, higher average), and setting `l` on the `transformationmatrix` changes the image not at all -- identical to six digits. The flag sits in ɴsɪ's list beside `PerFace` and `PerVertex`, which are primitive-variable flags, and says "interpolated linearly instead of using some other default method": for a primvar there are other methods, for motion samples there is no other default to override. **What it *does* govern was not established**: no observable effect was found on a subdiv vertex primvar or on deforming `P` either, so the flag is recorded and emitted (`stream/mod.rs` `flag_prefix`) and otherwise carried, not interpreted. Reading it as motion interpolation would have invented a rule | Find a case where `l` changes a render, or record that 3Delight 2.9.207 ignores it. |
| A transform interpolates at an arbitrary time | Covered | `resolve/mod.rs` `world_transform_interpolated_at` | `resolve::tests::a_transform_interpolates_between_its_samples`, `interpolating_at_a_sample_returns_the_sample`, `interpolating_outside_the_sampled_range_holds_the_end_sample`, `each_node_interpolates_from_its_own_samples`, `interpolation_keeps_the_static_nodes_of_a_chain`, `a_single_sample_node_is_constant`. Taking the left sample, refusing outside the range, extrapolating instead of holding, and dropping static nodes each redden a different one. **Outside the sampled range the end sample is held**, because 3Delight holds it: samples at `t=0`/`t=1` under a shutter of `[-1, 2]` leave zero alpha beyond the sampled positions and a peak at each end 2.7x the swept middle. The first version refused, which would have failed a backend on a scene the renderer renders. Component-wise is not an approximation: interpolating a transformed point gives `((1-a)M₀ + aM₁)p`, so element-wise interpolation of the matrix *is* interpolation of the moving point. Each node interpolates from its own samples and the results compose, which the fourth test separates from interpolating the composed matrices (4.0 against 6.0) | -- |
| Deforming geometry (`P` sampled) is resolvable | Covered | `resolve/mod.rs` `attribute_times` and `attribute_samples` | `resolve::tests::a_deforming_mesh_reports_its_sample_times` asserts both that the transform is static *and* that `P` has two sample times, which is the shape of the bug: `motion_times` answered "static" for a mesh that plainly deforms. Also `attribute_times_are_per_attribute`, `a_static_attribute_has_no_sample_times`, `attribute_samples_are_time_ordered`. Making the filter per-node rather than per-attribute, and folding the static value in as a sample, each redden a different one | -- |
| An unknown handle is refused, not answered | Covered | `resolve/mod.rs` `ResolveError::UnknownHandle` | `resolve::tests::attribute_times_refuses_an_unknown_handle`; returning an empty `Vec` instead reddens it. "Not sampled" for a handle that names nothing reads as a fact about the scene rather than about the question | -- |
| Resolution is linear in the scene | Covered | `scene/mod.rs` `by_from` / `by_to` / `by_to_attr`, maintained on `connect` and rebuilt on removal; every walk in `resolve/mod.rs` goes through them | Measured on a 20 000-mesh scene under a shared transform: 655 ms debug, 50 ms release, scaling linearly across 2k/8k/20k. Before the index the same scene took roughly 200 s -- `by_to` alone was not enough, because a transform with 20 000 children has 20 000 incoming `objects` edges and gathering scanned them once per child. | -- |
| Node types are never consulted | Partial | `resolve/mod.rs` reads attributes, not types | Implicit in every test | Under the ɴsɪ documentation draft, `sourcemodels` is renamed `objects`, which would make `objects` mean two different things depending on the destination node's type. See `research.md` D10. |

| `ATTR.priority` selects between definitions | Covered | `resolve/mod.rs` `attribute_value`, reading the `ATTR.priority` int beside the attribute on the same node | `resolve::tests::attr_priority_beats_proximity`, `attr_priority_lifts_only_its_own_attribute`; forcing the priority to `0` reddens both | -- |
| A per-ray `visibility.<ray>` beats the default at equal priority | Covered | `resolve/mod.rs` `attribute_value` ranks specificity after priority | `resolve::tests::a_per_ray_visibility_beats_the_default` and `a_prioritised_default_beats_a_per_ray_visibility`, plus 3Delight probes `G`/`G2`. Note that the first of those survives *deleting* the specificity comparator -- with both attributes on one node the push order already ranks them -- so the comparator itself is pinned by `specificity_is_compared_before_proximity`, not by this row | -- |
| A per-ray query falls back to the default `visibility` | Covered | `resolve/mod.rs` `attribute_value` `fallback` | `resolve::tests::the_default_visibility_answers_a_per_ray_query`; removing the fallback reddens it | -- |
| `visibility.set.subsurface` is not a ray type | Covered | `resolve/mod.rs` `RAY_TYPES`, the eight the specification lists | `resolve::tests::visibility_set_subsurface_is_not_a_ray_type`. It is a *connection* to a `set` node; accepting any `visibility.*` suffix would rank a connection against a flag | -- |
| An `ATTR.priority` that is not an `int` is ignored | Covered | `resolve/mod.rs` `priority_value` reads `I32` and nothing else | `resolve::tests::a_non_integer_priority_is_ignored` (an `F32`) and `an_int64_priority_is_ignored`; restoring either arm reddens one of them. The `I64` arm shipped in `e8839cc` and **no test was red** -- 3Delight ignores an `int64` priority too (probes `I1`, `I4`), while accepting `int64` for the `visibility` *value* (`I3`), so the rejection is specific to the priority | -- |
| `shaderattributes` nodes are resolved | Covered | `resolve/mod.rs` `shader_attributes` and `shader_attribute_value`, over `gathered_containers(.., ShaderAttributes)` | `resolve::tests::the_nearest_shader_attribute_wins`, `an_ancestor_shader_attribute_is_inherited`, `an_undefined_shader_attribute_resolves_to_none`; reversing proximity reddens the first | -- |
| The primitive's own attributes outrank every container | Covered | `resolve/mod.rs` `shader_attribute_value` checks `node_entry(geometry)` first; `shader_attributes` lists the geometry first | `resolve::tests::the_geometrys_own_shader_attribute_outranks_every_container`, `a_shader_attribute_on_the_primitive_needs_no_container`; removing the check reddens both. ɴsɪ: "with the highest priority given to attributes set directly on the geometric primitive". Rendered (`T1`, `T4`, `T4m`, `T5`): a `tint` on the mesh beats one on an `attributes` node attached to that mesh in **both** directions -- so the mesh is not winning for holding the larger value -- and beats one carrying a `tint.priority`. This crate returned the container's value, a **wrong answer** rather than a missing one, for one commit | -- |
| Shader-attribute sources are in precedence order | Covered | `resolve/mod.rs` `shader_attributes` | `resolve::tests::shader_attribute_sources_are_in_precedence_order`, `the_first_connected_shader_attribute_wins_at_one_level`. Three orderings were unpinned before these: reversing the list, reversing the within-level order, and dropping `.root` edges each left the suite green | -- |
| The two containers do not cross | Covered | `resolve/mod.rs` gathers each by its own `EdgeKind` | `resolve::tests::the_two_attribute_containers_do_not_cross`: a `geometryattributes` node does not answer a shader-attribute query, nor the reverse. Pointing the shader walk at `AttributeBinding` reddens four tests | -- |
| A shader attribute ignores `ATTR.priority` | Covered | `resolve/mod.rs` `shader_attribute_value` takes the nearest and stops | `resolve::tests::a_shader_attribute_ignores_attr_priority`; making the selection honour a priority reddens it. ɴsɪ gives this node proximity only -- "priority is given to nodes attached closest to the geometric primitive" -- so applying the `geometryattributes` rule here would invent one | -- |
| Attributes provided through a `set` | Covered | `resolve/mod.rs` `gathered_containers` walks `SetMember` edges from **every node on the chain**, deduplicated to the nearest occurrence | `resolve::tests::an_attributes_node_on_a_set_binds`, `a_set_on_a_transform_in_the_chain_is_gathered`, `the_geometrys_own_container_outranks_its_set`, `a_transforms_own_container_outranks_its_set`, `a_set_of_the_geometry_outranks_a_set_of_its_transform`, `a_set_outranks_the_transform_above_it`, `the_first_set_membership_wins`, `a_nested_sets_attributes_are_not_inherited`, `an_attr_priority_on_a_set_beats_the_geometrys_own`, `a_set_provides_shader_attributes_below_the_geometry`, `a_set_holding_two_chain_nodes_is_one_source`. Restricting sets to the geometry, dropping the dedup, and keeping the last occurrence instead of the first each redden a different one. Rendered in 3Delight, every case mirrored; see `research.md` D13 for the decisive scenes inline | -- |
| A node with `ATTR.priority` but no `ATTR` | Open | `resolve/mod.rs` `attribute_value` skips it | `resolve::tests::a_priority_without_its_attribute_is_skipped` pins the divergence so it cannot change silently | 3Delight treats such a node as defining `ATTR` **at its default value**, with that priority: a node carrying only `visibility.priority` makes the geometry visible over a farther `visibility 0`, at priority `10` (`B3x`) and at `0` (`B3y`) alike, while a node with no attributes at all does not (`B3v`). This crate has no value to return -- `AttributeValue::arg` borrows a recorded argument, and ɴsɪ's per-attribute defaults are not carried here. Closing it means either carrying the defaults or widening `AttributeValue`; `#[non_exhaustive]` leaves room. A backend can meanwhile scan `Binding::attributes` for `<name>.priority`. |
| Specificity is compared before proximity | Covered | `resolve/mod.rs` `attribute_value` `sort_by`, specificity ahead of the gathered order | `resolve::tests::specificity_is_compared_before_proximity`; removing the comparator reddens it. The first version of that test did **not** cover it: with both attributes on one node the push order already ranks them, and the mutation stayed green. Confirmed in 3Delight: probes `E` (near `visibility 1`, far `visibility.camera 0` renders invisible) and `E2` (the mirror image), so the distant per-ray value beats the nearer default | -- |
| Duplicate `sourcemodels` indices are refused | Covered | `resolve/mod.rs` `instance_transforms`, `ResolveError::DuplicateModelIndex` | `resolve::tests::duplicate_model_indices_are_refused`. ɴsɪ requires distinct indices so the models "form an ordered list"; picking the first was a guess |

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
