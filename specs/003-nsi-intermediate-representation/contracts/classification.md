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
| `displacementshader` and `volumeshader` are shader references | Covered | `edge.rs` `classify` | `resolve::tests::displacement_and_volume_shaders_resolve_too`; before this both were rejected, so no displaced or volumetric scene could be recorded | -- |
| `sourcemodels` is an instance source | Covered | `edge.rs` `classify` | `classifier::instancing_source_models` | -- |
| `screens`, `outputlayers`, `outputdrivers` are output routing | Covered | `edge.rs` `classify` | `classifier::output_chain` | -- |
| A named source port is a shader-network edge | Covered | `edge.rs` `classify` early return on `from_attr` | `classifier::a_named_output_port_is_a_shader_network_edge` | -- |
| An unlisted destination is carried, never interpreted | Covered | `edge.rs` `EdgeKind::Other`; `classify` is total and returns no error | `classifier::an_unlisted_destination_is_carried_with_its_name`; `recorder::tests::an_unlisted_connection_is_carried_not_interpreted` also asserts it does not become a material. ɴsɪ's destination set is open -- §4.8 connects a node to another's `visibility` -- so refusing what is not listed made legal scenes unrecordable. Resolution still interprets only the named classes | -- |
| `classify` and `EdgeKind::to_attr` stay inverse | Covered | `edge.rs` `classify`, `edge.rs` `EdgeKind::to_attr` | `stream_roundtrip::recorder_replays_what_3delight_writes`; the fixture now connects `objects`, `geometryattributes`, `surfaceshader`, `sourcemodels`, `screens`, `outputlayers`, `outputdrivers` and a shader-network edge, and 3Delight's own stream is the expectation for each | -- |
| `Some("")` is not a source port | Covered | `edge.rs` `classify` filters the empty string before the port branch | `classifier::an_empty_source_port_is_not_a_port`, and `stream_roundtrip` drives one through 3Delight | -- |
| Every `<connection>` the specification declares is classified | Covered | `edge.rs` `classify` | `classifier::every_connection_the_specification_declares_is_classified`, which pins the list read out of `nsi.pdf` and checks each round-trips through `EdgeKind::to_attr`. Five were missing, so an exporter using a lens shader or a background layer could not record at all | -- |
| `members`, `lightset` and `shaderattributes` classify | Covered | `edge.rs` `classify` | `classifier::set_membership_and_light_sets`. ɴsɪ's light-set workflow connects lights to a `set` and that set to an `outputlayer` | -- |

## Invariants

- Classification depends only on `from_attr` and `to_attr`, never on
  node types. ɴsɪ permits connections the node types would not imply.
- A `from_attr` of `None` and of `Some("")` classify identically. ɴsɪ
  documents both as connecting the `from` node itself.
- `EdgeKind::to_attr` is the inverse of `classify` for every non-shader class.
  These two functions must change together.

## Failure Modes

- **An unlisted destination** is not a failure. It is recorded as
  `EdgeKind::Other` with its name, and resolution ignores it. The
  trade-off -- a typo now does nothing quietly instead of failing loudly
  -- is stated in `spec.md` R5.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --test classifier`
- `cargo test -p nsi-intermediate --test stream_roundtrip`, which is
  what holds `classify` and `EdgeKind::to_attr` inverse over every non-shader
  class. It needs 3Delight; see `quickstart.md`.
