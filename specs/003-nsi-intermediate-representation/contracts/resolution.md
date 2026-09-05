# Contract: Graph Resolution

## Scope

Covers turning ɴsɪ's scene-graph semantics into flat facts: world
transforms, material bindings, output chains and instance sources. Does
not cover mapping those facts onto a renderer.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| No transforms yields identity | Covered | `resolve.rs` `world_transform` | `resolve::tests::a_node_with_no_transforms_is_identity` | -- |
| A parent transform applies to its child | Covered | `resolve.rs` `world_transform` | `resolve::tests::a_single_transform_applies_to_its_child` | -- |
| Nested transforms accumulate | Covered | `resolve.rs` `mul` | `resolve::tests::nested_transforms_compose` | -- |
| A child's matrix applies before its parent's | Covered | `resolve.rs` `mul` doc + call order | `resolve::tests::child_transform_applies_before_parent` (non-commuting pair) | -- |
| A transform's own matrix is included | Covered | `resolve.rs` `local_transform` | `resolve::tests::a_transforms_own_matrix_is_included` | -- |
| A cycle is a typed error, not a hang and not an answer | Covered | `resolve.rs` `chain` tracks visited nodes | `resolve::tests::a_cycle_is_an_error` asserts `ResolveError::Cycle`; `binding_tests::a_cycle_is_an_error_for_bindings_too` | -- |
| More than one parent is a typed error | Covered | `resolve.rs` `chain` counts `SceneMember` parents | `resolve::tests::more_than_one_parent_is_an_error` asserts `ResolveError::MultipleParents` naming both parents | -- |
| A motion-sampled transform is a typed error | Covered | `resolve.rs` `has_motion_transform` | `resolve::tests::a_motion_sampled_transform_is_an_error`; `motion_samples_of_other_attributes_do_not_block_resolution` pins that only a sampled *transform* blocks | -- |
| A non-`f64` matrix is ignored, not reinterpreted | Covered | `resolve.rs` `local_transform` matches `OwnedData::F64` only | `resolve::tests::a_non_f64_matrix_is_skipped_not_reinterpreted`, which sets the same numbers as `MatrixF32` and asserts identity | -- |
| `attributes` dissolves to a shader | Covered | `resolve.rs` `geometry_binding` | `binding_tests::dissolves_attributes_to_a_shader` | -- |
| Unbound geometry has no binding | Covered | `resolve.rs` `geometry_binding` | `binding_tests::unbound_geometry_has_no_binding` | -- |
| `attributes` without a shader still binds | Covered | `resolve.rs` `geometry_binding` | `binding_tests::attributes_without_a_shader_still_bind` | -- |
| One `attributes` node fans out to many shapes | Covered | `resolve.rs` resolves per geometry | `binding_tests::one_attributes_node_fans_out_to_every_shape` | -- |
| A binding on an ancestor transform is inherited | Covered | `resolve.rs` `geometry_binding` searches the whole `chain` | `binding_tests::a_binding_on_an_ancestor_transform_is_inherited` | -- |
| The nearest binding wins at equal priority | Covered | `resolve.rs` `max_by` on `(priority, -depth, -order)` | `binding_tests::the_nearest_binding_wins_at_equal_priority` | -- |
| `"priority"` overrides proximity | Partial | `resolve.rs` `max_by` orders on `priority` first; `recorder.rs` `priority_of` records it | `binding_tests::priority_beats_proximity` proves the implemented rule | ɴsɪ documents `priority` only as "in which order the nodes should be considered", without saying higher or lower wins, or how ties break. Confirm the direction and the tie-break against 3Delight, then restate R12 as observed rather than chosen. |
| The output chain resolves end to end | Covered | `resolve.rs` `render_outputs` | `output_tests::resolves_the_whole_output_chain` | -- |
| A screen with no layers still resolves | Covered | `resolve.rs` `render_outputs` | `output_tests::a_screen_without_layers_still_resolves` | -- |
| Layer order is connection order | Covered | `resolve.rs` iterates `edges` in order | `output_tests::multiple_layers_keep_connection_order` | -- |
| A layer may fan out to several drivers | Covered | `resolve.rs` `render_outputs` | `output_tests::a_layer_may_have_several_drivers` | -- |
| Multiple screens yield multiple outputs | Covered | `resolve.rs` iterates every `Screen` edge | `output_tests::multiple_screens_yield_one_output_each`, two cameras and two screens | -- |
| Instance sources resolve in connection order | Covered | `resolve.rs` `instance_sources` | `instance_tests::resolves_instance_source_models` | -- |
| Motion-sampled transforms compose per sample | Open | `scene.rs` stores `time_attrs` separately for this reason | None; the case is an error rather than an answer | Add `world_transform_at(handle, time)` composing per sample, and decide interpolation between samples. Until then a motion-blurred scene cannot be resolved at all, which is loud but not useful. |
| An instanced node resolves to one transform per path | Open | `resolve.rs` `chain` rejects it | None | The error is the honest stop-gap; ɴsɪ's lightweight instancing needs a per-path answer. Decide the shape -- `Vec<[f64; 16]>`, or a path-qualified handle -- then test a two-parent fixture. |
| `surfaceshader` honours `priority` too | Open | `resolve.rs` takes the first `SurfaceShader` edge | None | Only `AttributeBinding` consults `priority` today. Decide whether an `attributes` node with two shaders is legal, then match the rule. |

## Invariants

- ɴsɪ is row-major, row-vector: `p * child * parent`. Composition is
  `mul(child, parent)`.
- Resolution is pure. It reads a `Scene` and allocates results; it never
  mutates.
- `geometry_binding` returns the attributes **handle**, not its
  contents, because visibility encoding is renderer-specific.
- Node *types* are never consulted, matching classification. A
  `transformationmatrix` on a node ɴsɪ would not call a transform is
  composed like any other; the attribute is the fact, not the type.
- Every walk up the `objects` hierarchy goes through `Scene::chain`, so
  the scenes with no single answer are rejected in one place.

## Failure Modes

Every variant of `ResolveError` is a scene ɴsɪ permits and this crate
refuses to guess about. None is a hang, and none is a plausible wrong
matrix.

- **`Cycle`** -- the chain revisits a node. No correct answer exists.
- **`MultipleParents`** -- ɴsɪ's lightweight instancing, which has one
  world transform per path rather than one overall.
- **`MotionSampledTransform`** -- per-sample composition is not
  implemented, and the static transform would be the wrong answer.
- **A missing transform attribute** contributes identity rather than
  failing. A transform node without a matrix is legal in ɴsɪ.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --lib resolve`
- To close the motion row: `world_transform_at`, then a test where two
  time samples give different world transforms.
- To close the instancing row: a two-parent fixture asserting one
  transform per path.
- To close the `priority` row: a 3Delight scene binding two `attributes`
  nodes at different priorities, and an observation of which wins.
