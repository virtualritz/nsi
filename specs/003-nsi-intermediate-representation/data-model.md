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
`type Error = ClassifyError`.

### `Scene`

| Field | Type | Ownership |
| --- | --- | --- |
| `nodes` | `IndexMap<String, Node>` | owned |
| `edges` | `Vec<Edge>` | owned |

`IndexMap` and `Vec` are load-bearing: insertion order is replay order,
and the stream comparison in `contracts/stream.md` is meaningless if
replay reorders. `delete` uses `shift_remove`, not `swap_remove`.

### `Node`

| Field | Type | Notes |
| --- | --- | --- |
| `node_type` | `String` | the ɴsɪ node type |
| `attrs` | `IndexMap<String, OwnedArg>` | static attributes |
| `time_attrs` | `Vec<(f64, IndexMap<String, OwnedArg>)>` | motion samples, time-sorted |

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
| `priority` | `i32` | ɴsɪ's `"priority"` connect argument, `0` when absent. Read only by `geometry_binding`. Not part of edge identity: `disconnect` ignores it. |

`EdgeKind` is one of `SceneMember`, `AttributeBinding`, `SurfaceShader`,
`InstanceSource`, `Screen`, `OutputLayer`, `OutputDriver`,
`ShaderNetwork { from_port, to_port }`.

### Resolved views

`Binding { attributes, surface_shader }`,
`RenderOutput { camera, screen, layers }`,
`OutputLayer { handle, drivers }`. Produced on demand; not stored.

### `ResolveError`

The three scenes ɴsɪ permits and this crate refuses to answer for:
`MultipleParents { handle, parents }`, `MotionSampledTransform { handle }`
and `Cycle { handle }`. Returned by `world_transform`; the first and
third by `geometry_binding` too, since both walk the same chain.

### Constants

`ROOT` (`".root"`) is ɴsɪ's root handle, the terminator of every chain
walk. `IDENTITY` is a row-major 4x4 identity, the transform of a node
with no matrices above it.

## Derives

`Debug` and `Clone` on everything public. Beyond that:

| Type | `PartialEq` | `Eq` / `Hash` | Why |
| --- | --- | --- | --- |
| `EdgeKind`, `Edge`, `ClassifyError`, `ResolveError`, `Binding`, `RenderOutput`, `OutputLayer`, `HostPtr`, `RenderState` | yes | yes | No float fields. `HostPtr` hashes the address, which is what it is. |
| `OwnedArg`, `OwnedData` | yes | **no** | Both carry `f32`/`f64` payloads. See `research.md` D7. |
| `Node`, `Scene` | yes | **no** | Transitively contain `OwnedArg`. `PartialEq` alone still lets a test assert a whole scene is unchanged, which is how the `evaluate` no-op is proven. |

`Copy` where derivable: `HostPtr`, `RenderState`.

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
