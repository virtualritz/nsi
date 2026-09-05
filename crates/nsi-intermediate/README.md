# `nsi-intermediate`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-intermediate/badge.svg)](https://docs.rs/nsi-intermediate)
[![Crate](https://img.shields.io/crates/v/nsi-intermediate.svg)](https://crates.io/crates/nsi-intermediate)

<!-- cargo-rdme start -->

A renderer-agnostic intermediate representation for the Nodal Scene
Interface.

ɴsɪ is the front end and a renderer is the back end; this is what
sits between them. It does the jobs an IR does — capture the
incoming calls, classify and canonicalise them, lower ɴsɪ's graph
semantics into flat facts a renderer can consume, and serialise the
result for inspection.

Nothing here is specific to any one renderer, and none of it needs a
renderer present to build or test.

Backends consume a lowered scene and flush it into their own
representation: [`nsi-mitsuba`] into Mitsuba `Properties`,
[`nsi-moonray`] into `scene_rdl2` `SceneObject`s.

# Naming

The crate is spelled out in full so the name says what it is.
Consumers who prefer the shorter idiom may alias it:

```rust,ignore
use nsi_intermediate as nsi_ir;
```

[`nsi-mitsuba`]: https://github.com/virtualritz/nsi-mitsuba
[`nsi-moonray`]: https://github.com/virtualritz/nsi-moonray

# Writing a backend

Record with [`Recorder`], which implements [`nsi_trait::Nsi`], then
ask the [`Scene`] for facts rather than walking it yourself:

```rust,ignore
let recorder = Recorder::new();
build_the_scene(&recorder)?;          // any generic ɴsɪ consumer.
let scene = recorder.scene();

for output in scene.render_outputs() {
    // one entry per screen, with its layers and drivers in order.
}

for (handle, node) in &scene.nodes {
    if node.node_type != "mesh" {
        continue;
    }

    // Material, gathered along the whole path to `.root`.
    if let Some(binding) = scene.geometry_binding(handle)? {
        let shader = binding.surface_shader;
        // `binding.attributes` is every `attributes` node on the
        // path, in ɴsɪ's precedence order: take the first that
        // defines the attribute you want.
    }

    // Transform. Ask whether it moves before asking where it is.
    let matrices = if scene.motion_times(handle)?.is_empty() {
        vec![(0.0, scene.world_transform(handle)?)]
    } else {
        scene.world_transform_samples(handle)?
    };
}
```

# What it refuses to answer

ɴsɪ permits scenes with no single correct answer, and this crate
returns a typed error for each rather than a plausible wrong one: a
node with two `objects` parents (ɴsɪ's lightweight instancing), a
cycle, a node that never reaches `.root`, and a motion-sampled
transform asked for at a time it has no sample at. Nothing here
interpolates between motion samples -- element-wise interpolation of
a matrix is wrong for anything containing a rotation, and the right
decomposition is the backend's to choose.

# What it does not resolve

Shader networks are classified and carried with their ports intact,
because their consumer is OSL rather than a graph walk. `evaluate`
is a no-op: procedurals and Lua imply an execution model this
surface does not define. An instanced node is detected and refused,
not expanded into one transform per path.

# The ɴsɪ copy contract

Every ɴsɪ argument except a `NSIType` pointer (`Type::Reference`) is
copied during the call, so a caller may free its data as soon as the
call returns. The recorder copies for the same reason a renderer
does; it introduces no cost that a live context would not have paid.
`Reference` is the exception: it is passed through, retained, and is
why the recorder carries a context lifetime.

<!-- cargo-rdme end -->

## Testing

Everything but one test runs without a renderer:

```bash
cargo test -p nsi-intermediate
```

The exception is `tests/stream_roundtrip.rs`, the fidelity gate. It
builds one scene through both a real 3Delight `apistream` context and
the recorder, and compares the two streams. That test needs 3Delight
installed, `DELIGHT` set, and a reachable licence server. It is
deliberately **excluded from the published package**, so `cargo test` on
a crates.io checkout does not fail for want of a renderer.

The gate is what settled the parts of the ɴsɪ stream format that are not
written down: that doubles are `%.17g`, that argument flags are letter
prefixes inside the type name, and that a pointer argument's parameter
line is omitted while its statement is kept. Each was read off 3Delight's
own output rather than inferred.

## Specification

The behaviour of this crate is specified in
[`specs/003-nsi-intermediate-representation/`](../../specs/003-nsi-intermediate-representation),
with a contract matrix per surface. Rows are marked `Covered` only where
a named test proves them, and what remains `Open` is listed rather than
omitted.

## License

MIT OR Apache-2.0 OR Zlib, at your option.
