# Data Model: ɴsɪ Intermediate Representation

## Entities

### `Recorder`

Owns the recorded scene and the render state, each behind a `Mutex`
because `Nsi` takes `&self` throughout. `Send + Sync`.

| Field | Type | Ownership |
| --- | --- | --- |
| `scene` | `Mutex<Scene>` | owned |
| `state` | `Mutex<RenderState>` | owned |

`type Arg<'call> = nsi_ffi_wrap::Arg<'call, 'static>`;
`type Error = RecordError`.

### `Scene`

| Field | Type | Ownership |
| --- | --- | --- |
| `nodes` | `IndexMap<String, Node>` | owned, private |
| `edges` | `Vec<Edge>` | owned, private |
| `by_from`, `by_to`, `by_to_attr` | `HashMap<_, Vec<usize>>` | derived indexes |

The fields are private and `Scene` is `#[non_exhaustive]`: the indexes
are an implementation detail, and exposing the tables would have frozen
them before the index existed. Read through `nodes()`, `node()`,
`edges()`, `edges_from()`, `edges_to()` and `edges_to_attr()`.

`IndexMap` and `Vec` are load-bearing: insertion order is replay order,
and the stream comparison in `contracts/stream.md` is meaningless if
replay reorders. `delete` uses `shift_remove`, not `swap_remove`.

### `Node`

| Field | Type | Notes |
| --- | --- | --- |
| `node_type` | `String` | the ɴsɪ node type |
| `attrs` | `IndexMap<String, OwnedArg>` | static attributes |
| `samples` | `IndexMap<String, Vec<(f64, OwnedArg)>>` | every `set_attribute_at_time` call, per attribute, in **call** order -- ɴsɪ's rules are stated over calls, and a table keyed by time cannot say which call was last |

Motion samples are separate because transform composition is per-sample.

### `OwnedArg` / `OwnedData`

| Field | Type |
| --- | --- |
| `name` | `String` |
| `type_tag` | `nsi_trait::Type` |
| `array_length` | `usize` |
| `flags` | `i32` |
| `data` | `OwnedData` |

`OwnedData` variants are **storage representations, not ɴsɪ types**:
`F32`, `F64`, `I32`, `I64`, `String`, `Reference`. Colour, point,
vector, normal and 4x4 `f32` matrices all live in `F32` and are told
apart by `type_tag`.

### `HostPtr`

`#[repr(transparent)]` over `*const c_void`, with `Send`/`Sync`
asserted. Recorded from `Type::Reference`; never dereferenced.

### `Edge` / `EdgeKind`

| Field | Type | Notes |
| --- | --- | --- |
| `from` | `String` | |
| `to` | `String` | |
| `kind` | `EdgeKind` | |
| `args` | `Vec<OwnedArg>` | Every argument of the `connect` call, kept whole. `priority()`, `index()` and `strength()` read the three ɴsɪ defines. Not part of edge identity: `disconnect` ignores them. |

`EdgeKind` covers every `<connection>` attribute the ɴsɪ specification
declares: `SceneMember`, `AttributeBinding`, `SurfaceShader`,
`DisplacementShader`, `VolumeShader`, `LensShader`, `InstanceSource`,
`SetMember`, `LightSet`, `ShaderAttributes`, `BackgroundLayer`,
`Bounds`, `SubsurfaceSet`, `ExclusiveShading`, `Screen`, `OutputLayer`,
`OutputDriver`, and `ShaderNetwork { from_port, to_port }`.
`EdgeKind::to_attr` is the inverse of `classify`; they live together in
`edge.rs` so the stream emitter, the Lua emitter and `disconnect` cannot
drift apart.

Most classes are *carried*, not resolved. Only membership, attribute
binding, the shader slots, instancing and the output chain are walked.

### Resolved views

`Binding { attributes: Vec<String>, surface_shader, displacement_shader,
volume_shader }` -- a list, because ɴsɪ considers *every* `attributes`
node on the path;
`RenderOutput { camera, screen, layers }`,
`OutputLayer { handle, drivers }`,
`Instance { source, transform }` -- one placement of an `instances`
node, pairing its matrix with the position in `instance_sources` it
draws. Produced on demand; not stored.

### `RecordError`

Why a recording call failed: `Classify`, `Reserved { handle }`,
`UnknownHandle { handle }`,
`TypeMismatch { handle, existing, requested }`. One type across every
`Nsi` method, so a consumer matches on one thing.

### `ResolveError`

The scenes ɴsɪ permits and this crate refuses to answer for:
`MultipleParents { handle, parents }`, `Cycle { handle }`,
`Detached { handle }`, `Instanced { instancer }`,
`NotAnAncestor { handle, ancestor }`,
`MotionSampledTransform { handle }` and
`MissingSampleAtTime { handle, time, available }`. Returned by
`world_transform*`; the graph-shaped ones by `geometry_binding` too,
since both walk the same chain.

### Constants

`ROOT` (`".root"`) is ɴsɪ's root handle and the last entry of every
chain walk. `GLOBAL` (`".global"`) is the options node. Both are
reserved: ɴsɪ says they "don't need to be created". `ALL` (`".all"`) is
the `disconnect` wildcard. `IDENTITY` is a row-major 4x4 identity, the
transform of a node with no matrices above it.

## Derives

`Debug` and `Clone` on everything public. Beyond that:

| Type | `PartialEq` | `Eq` / `Hash` | Why |
| --- | --- | --- | --- |
| `EdgeKind`, `ClassifyError`, `RecordError`, `Binding`, `RenderOutput`, `OutputLayer`, `HostPtr`, `RenderState` | yes | yes | No float fields. `HostPtr` hashes the address, which is what it is. |
| `OwnedArg`, `OwnedData` | yes | **no** | Both carry `f32`/`f64` payloads. See `research.md` D7. |
| `Edge` | yes | **no** | Carries its connection arguments, which are `OwnedArg`. |
| `ResolveError` | yes | **no** | `MissingSampleAtTime` carries the requested time and the available ones. |
| `Node`, `Scene` | yes | **no** | Transitively contain `OwnedArg`. `PartialEq` alone still lets a test assert a whole scene is unchanged, which is how the `evaluate` no-op is proven. |

`Copy` where derivable: `HostPtr`, `RenderState`.

`#[non_exhaustive]` on `RecordError`, `ResolveError`, `EdgeKind`,
`OwnedData` and `Binding`. Each grew a variant or a field during review,
and each will grow again: ɴsɪ has more node types, more shader slots and
more rules than this surface enforces. Marking them now means enforcing
the next rule is not a breaking change for a backend that matches on
them.

## Wire Formats

### ɴsɪ argument marshalling

`ParamValue::len()` is the **raw element count**. The C `count` field is
`len / array_length`. Total scalars is
`len * components_per_element(type_tag)`, where colour/point/vector/
normal are 3 and matrices are 16.

`Type::Reference` is a raw pointer, called `Pointer` in the C API. It is
unrelated to a renderer's object references despite the name.
`Reference::as_c_ptr` yields a pointer *to* the pointer, so recording
dereferences exactly one level.

### `.nsi` stream

Verified against 3Delight 2.9.207.

```text
Create "cam" "perspectivecamera"
SetAttribute "cam"
  "fov" "float" 1 45
  "resolution" "int[2]" 1 [ 1280 720 ]
SetAttributeAtTime "xf" 0.5
  "t" "double" 1 1
Connect "xf" "" ".root" "objects"
```

Type names: `float`, `double`, `int`, `int64`, `string`, `color`,
`point`, `vector`, `normal`, `matrix`, `doublematrix`. An array length
appends as `[n]`. A lone scalar is bare; more than one is bracketed.

### Transform convention

Row-major storage, RenderMan row-vector convention: `p * child * parent`.
Composition is `mul(child, parent)`.

## Migrations

No in-memory format is persisted, but the `.nsi` stream is a persisted,
cross-language format and needs its compatibility stated:

- **Compatibility:** the emitter tracks 3Delight 2.9.207 and promises
  nothing beyond it. The oracle is generated live in the same test run,
  so a format change surfaces as a failing gate rather than as silent
  drift. When it does, `stream.rs` is corrected and never the
  expectation.
- **Load behavior:** none. Emission is one-way; nothing here parses a
  `.nsi` stream.
- **A `Scene` is process-local.** `HostPtr` values are addresses in the
  recording process. Cloning a `Scene` copies them; serialising one
  would carry dead addresses into another process. Neither is forbidden,
  and neither is meaningful.
