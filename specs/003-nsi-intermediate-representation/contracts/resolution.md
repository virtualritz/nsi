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
| A cycle terminates | Covered | `resolve.rs` budget bounded by node count | `resolve::tests::a_cycle_terminates` | -- |
| A non-`f64` matrix is ignored, not reinterpreted | Partial | `resolve.rs` `local_transform` matches `OwnedData::F64` only | None | Add a case setting `transformationmatrix` as `MatrixF32` and asserting identity, so the skip is deliberate rather than incidental. |
| Motion-sampled transforms compose per sample | Open | `scene.rs` stores `time_attrs` separately for this reason | None; `world_transform` reads `attrs` only | Decide the API — a `world_transform_at(handle, time)` — then test a two-sample chain. **A motion-blurred scene currently resolves to its static transform.** |
| `attributes` dissolves to a shader | Covered | `resolve.rs` `geometry_binding` | `binding_tests::dissolves_attributes_to_a_shader` | -- |
| Unbound geometry has no binding | Covered | `resolve.rs` `geometry_binding` | `binding_tests::unbound_geometry_has_no_binding` | -- |
| `attributes` without a shader still binds | Covered | `resolve.rs` `geometry_binding` | `binding_tests::attributes_without_a_shader_still_bind` | -- |
| One `attributes` node fans out to many shapes | Covered | `resolve.rs` resolves per geometry | `binding_tests::one_attributes_node_fans_out_to_every_shape` | -- |
| The output chain resolves end to end | Covered | `resolve.rs` `render_outputs` | `output_tests::resolves_the_whole_output_chain` | -- |
| A screen with no layers still resolves | Covered | `resolve.rs` `render_outputs` | `output_tests::a_screen_without_layers_still_resolves` | -- |
| Layer order is connection order | Covered | `resolve.rs` iterates `edges` in order | `output_tests::multiple_layers_keep_connection_order` | -- |
| A layer may fan out to several drivers | Covered | `resolve.rs` `render_outputs` | `output_tests::a_layer_may_have_several_drivers` | -- |
| Instance sources resolve in connection order | Covered | `resolve.rs` `instance_sources` | `instance_tests::resolves_instance_source_models` | -- |
| Multiple screens yield multiple outputs | Partial | `resolve.rs` iterates every `Screen` edge | None; every test uses one screen | Add a two-camera, two-screen case and assert both appear with the right cameras. |

## Invariants

- ɴsɪ is row-major, row-vector: `p * child * parent`. Composition is
  `mul(child, parent)`.
- Resolution is pure. It reads a `Scene` and allocates results; it never
  mutates.
- `geometry_binding` returns the attributes **handle**, not its
  contents, because visibility encoding is renderer-specific.

## Failure Modes

- **A cycle** is bounded by the node count and returns whatever composed
  before the budget ran out. It does not hang and does not error; a
  cyclic scene is malformed and no correct answer exists.
- **A missing transform attribute** contributes identity rather than
  failing. A transform node without a matrix is legal in ɴsɪ.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --lib resolve`
- To close the motion row: an API decision, then a test where two time
  samples give different world transforms. This is the largest known gap
  in this surface.
- To close the multi-screen row: a two-screen fixture.
