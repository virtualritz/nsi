# Contract: Connection Classification

## Scope

Covers turning an ɴsɪ connection into an `EdgeKind`. Does not cover what
a backend then does with it.

## Why This Contract Exists

An ɴsɪ connection is a typed multi-relation whose meaning depends on its
destination attribute. Of the classes in use, only `SurfaceShader` and
`ShaderNetwork` become object references in a target renderer. Mapping
them uniformly produces a backend that renders, with materials on the
wrong shapes and output routed nowhere. **This failure is silent**,
which is why every class carries its own row.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `objects` is scene membership | Covered | `edge.rs` `classify` | `classifier::scene_membership` | -- |
| `geometryattributes` is an attribute binding | Covered | `edge.rs` `classify` | `classifier::geometry_attributes_dissolve` | -- |
| `surfaceshader` is a material reference | Covered | `edge.rs` `classify` | `classifier::surface_shader_is_a_material_reference` | -- |
| `displacementshader` and `volumeshader` are shader references | Covered | `edge.rs` `classify` | `binding_tests::displacement_and_volume_shaders_resolve_too`; before this both were rejected, so no displaced or volumetric scene could be recorded | -- |
| `sourcemodels` is an instance source | Covered | `edge.rs` `classify` | `classifier::instancing_source_models` | -- |
| `screens`, `outputlayers`, `outputdrivers` are output routing | Covered | `edge.rs` `classify` | `classifier::output_chain` | -- |
| A named source port is a shader-network edge | Covered | `edge.rs` `classify` early return on `from_attr` | `classifier::a_named_output_port_is_a_shader_network_edge` | -- |
| An unknown destination is rejected, not guessed | Covered | `edge.rs` `ClassifyError` | `classifier::unknown_to_attr_is_rejected` | -- |
| Rejection propagates out of `Scene::connect` | Covered | `scene.rs` `connect` returns `Result` | `scene::tests::connect_rejects_an_unmapped_destination` | -- |
| Rejection propagates out of `Nsi::connect` | Covered | `recorder.rs` `connect` | `recorder::tests::an_unmapped_connection_is_an_error` | -- |
| `classify` and `stream::to_attr_of` stay inverse | Covered | `edge.rs` `classify`, `stream.rs` `to_attr_of` | `stream_roundtrip::recorder_replays_what_3delight_writes`; the fixture now connects `objects`, `geometryattributes`, `surfaceshader`, `sourcemodels`, `screens`, `outputlayers`, `outputdrivers` and a shader-network edge, and 3Delight's own stream is the expectation for each | -- |
| `Some("")` is not a source port | Covered | `edge.rs` `classify` filters the empty string before the port branch | `classifier::an_empty_source_port_is_not_a_port`, and `stream_roundtrip` drives one through 3Delight | -- |

## Invariants

- Classification depends only on `from_attr` and `to_attr`, never on
  node types. ɴsɪ permits connections the node types would not imply.
- A `from_attr` of `None` and of `Some("")` classify identically. ɴsɪ
  documents both as connecting the `from` node itself.
- `to_attr_of` is the inverse of `classify` for every non-shader class.
  These two functions must change together.

## Failure Modes

- **Unknown destination attribute:** `ClassifyError` naming the
  attribute, with a message pointing at `nsi_intermediate::classify`. The call
  fails; nothing is recorded.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --test classifier`
- `cargo test -p nsi-intermediate --test stream_roundtrip`, which is
  what holds `classify` and `to_attr_of` inverse over every non-shader
  class. It needs 3Delight; see `quickstart.md`.
