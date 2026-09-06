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

It does need **libclang** to build: the dependency chain reaches
`nsi-sys`, whose build script runs `bindgen` over ɴsɪ's header
unconditionally. That is a build-host requirement, not a runtime one --
no renderer has to be installed -- but a machine without libclang fails
there rather than here.

Backends consume a lowered scene and flush it into their own
representation: `nsi-mitsuba` into Mitsuba `Properties`,
`nsi-moonray` into `scene_rdl2` `SceneObject`s.

## Naming

The crate is spelled out in full so the name says what it is.
Consumers who prefer the shorter idiom may alias it:

```rust
use nsi_intermediate as nsi_ir;
```

`nsi-mitsuba`: https://github.com/virtualritz/nsi-mitsuba
`nsi-moonray`: https://github.com/virtualritz/nsi-moonray

## Writing a backend

Record with `Recorder`, which implements `nsi_trait::Nsi`, then
ask the `Scene` for facts rather than walking it yourself:

```rust
use nsi_intermediate::{Recorder, ResolveError};
use nsi_trait::Nsi;

let recorder = Recorder::new();

// Any generic ɴsɪ consumer drives this; a renderer would too.
recorder.create("mesh", "mesh", None)?;
recorder.create("attr", "attributes", None)?;
recorder.create("metal", "shader", None)?;
recorder.connect("mesh", None, ".root", "objects", None)?;
recorder.connect("attr", None, "mesh", "geometryattributes", None)?;
recorder.connect("metal", None, "attr", "surfaceshader", None)?;

let scene = recorder.into_scene(); // or `scene()` to borrow it.

for output in scene.render_outputs() {
    // One entry per screen, with its layers and drivers in order.
    let _ = (&output.camera, &output.layers);
}

let meshes: Vec<String> = scene
    .nodes()
    .filter(|(_, node)| node.node_type == "mesh")
    .map(|(handle, _)| handle.clone())
    .collect();

for handle in &meshes {
    // Material, gathered along the whole path to `.root`.
    if let Some(binding) = scene.geometry_binding(handle)? {
        assert_eq!(binding.surface_shader.as_deref(), Some("metal"));
        // `binding.attributes` is every `attributes` node on the
        // path, in ɴsɪ's precedence order: take the first that
        // defines the attribute you want.
        assert_eq!(binding.attributes, vec!["attr".to_string()]);
    }

    // Transform, at whatever times the backend samples. Use the
    // *interpolating* accessor: `world_transform_samples` answers
    // only where every node on the chain has a sample at exactly
    // that time, and refuses a chain whose nodes are sampled at
    // different times -- a scene 3Delight renders.
    let times = scene.motion_times(handle)?;
    let matrices: Vec<(f64, [f64; 16])> = if times.is_empty() {
        vec![(0.0, scene.world_transform(handle)?)]
    } else {
        times
            .iter()
            .map(|&t| {
                Ok((t, scene.world_transform_interpolated_at(handle, t)?))
            })
            .collect::<Result<_, nsi_intermediate::ResolveError>>()?
    };
    assert_eq!(matrices.len(), 1);
}
```

A geometry with more than one `objects` parent is ɴsɪ's lightweight
instancing and has no single transform or binding. Use
`Scene::placements` (or `Scene::placements_at` when it moves),
which returns one entry per path with the transform *and* the
binding resolved along it, and agrees with the single-answer
accessors when there is only one path. Its `binding.attributes` is a
list of nodes; `Scene::attribute_value_along` applies ɴsɪ's
precedence to one attribute along that same path.

Borrowing the scene while recording is possible but narrow:
`Recorder::scene` holds the lock every `nsi_trait::Nsi` method
takes, so two of those calls in one expression deadlock. When
recording is finished, `Recorder::into_scene` hands the scene over
without copying it.

## Output formats

ɴsɪ has three front ends, and this crate can write two of them back
out. Both are optional, so a backend that only wants the resolver
pays for neither.

| Feature | What it adds |
| --- | --- |
| *(none)* | `write_stream` -- the `.nsi` stream, ɴsɪ's own text format. |
| `lua` | `write_lua` -- the same scene as a Lua script. |
| `gzip` | `Compression::Gzip` for `write_stream_with`. |
| `zstd` | `Compression::Zstd`. |

The stream writer is unconditional because it is also this crate's
verification backbone: the fidelity gate compares it against what
3Delight writes for the same calls.

Compression is a property of the *file*, not of the format --
3Delight reads a compressed stream wherever it reads a plain one --
so it is a parameter of `write_stream_with` rather than a separate
emitter.

Lua is not merely another spelling of the stream. ɴsɪ's Lua binding
exposes fewer types than its C API: there is no `nsi.TypeDouble`, no
`nsi.TypeInt64` and no pointer type. An attribute Lua cannot express
is refused with a `LuaError::Inexpressible` rather than degraded,
because emitting it untyped silently turns a double into a float and
a large integer into a different number.

## What it refuses to answer

ɴsɪ permits scenes with no single correct answer, and this crate
returns a typed error for each rather than a plausible wrong one: a
cycle, a node that never reaches `.root`, and -- from the
*exact-hit* accessors only -- a motion-sampled transform asked for
at a time it has no sample at.

A node with two `objects` parents is refused by the single-answer
accessors for the same reason, because there is no single answer;
`Scene::placements` enumerates them instead.

Interpolation is available and is the renderer's own model, not a
guess: `Scene::world_transform_interpolated_at` interpolates
element-wise, which is what 3Delight does -- its rotation blur fits
component-wise far better than slerp -- and holds the end sample
outside the sampled range, as 3Delight holds it. An earlier version
of this section said nothing here interpolates and that the
decomposition was the backend's; both stopped being true several
releases of this crate ago.

## What it does not resolve

Shader networks are classified and carried with their ports intact,
because their consumer is OSL rather than a graph walk. `evaluate`
is a no-op: procedurals and Lua imply an execution model this
surface does not define, and a stream carrying `Evaluate` records as
a scene without whatever it would have produced, silently.

Asking an instancing *prototype* for a world transform is
`ResolveError::Instanced`, because ɴsɪ gives an `instances` node
one matrix per instance rather than one for the prototype -- ask
`Scene::instance_transforms`, or
`Scene::instance_transforms_at` when the instancer is animated.
Lightweight instancing (a node under two parents) *is* expanded, by
`Scene::placements`; an earlier version of this section said it
was refused and not expanded, which was true only before that method
existed.

## The ɴsɪ copy contract

Every ɴsɪ argument except a `NSIType` pointer (`Type::Reference`) is
copied during the call, so a caller may free its data as soon as the
call returns. The recorder copies for the same reason a renderer
does; it introduces no cost that a live context would not have paid.
`Reference` is the exception: it is passed through and retained,
which is why its pointee must outlive the recorder -- see
`Recorder` for why that is a `'static` bound rather than a
lifetime parameter.

<!-- cargo-rdme end -->

## Testing

Everything but the three gates below runs without a renderer:

```bash
cargo test -p nsi-intermediate
```

Three gates are the exception, and each needs 3Delight installed,
`DELIGHT` set, and a reachable licence server:

- `tests/stream_roundtrip.rs` builds one scene through both a real
  `apistream` context and the recorder, and compares the two streams.
- `tests/lua_roundtrip.rs` feeds an emitted Lua script to
  `renderdl -lua -cat` and compares what the renderer rebuilds.
- `tests/compression.rs` hands a gzipped stream to `renderdl -cat`.

Only `tests/classifier.rs` is **shipped in the published package**, so
`cargo test` on a crates.io checkout does not fail for want of a
renderer.

These gates settled the parts of ɴsɪ that are not written down: doubles
are `%.17g` while floats take the shorter of decimal and exponent form;
argument flags are letter prefixes inside the type name; a pointer
argument's parameter line is omitted while its statement is kept; an
array is marked by a flag, so `array_len(1)` is real; and ɴsɪ's Lua
binding cannot express doubles, 64-bit integers, pointers or flags at
all. Each was read off the renderer rather than inferred, and several
were found only by widening a fixture that was already green.

## Specification

The behaviour of this crate is specified in
[`specs/003-nsi-intermediate-representation/`](https://github.com/virtualritz/nsi/tree/master/specs/003-nsi-intermediate-representation),
with a contract matrix per surface. Rows are marked `Covered` only where
a named test proves them, and what remains `Open` is listed rather than
omitted.

## License

MIT OR Apache-2.0 OR Zlib, at your option.
