# `nsi-mitsuba` — an ɴsɪ backend on Mitsuba 3

Date: 2026-09-03
Status: approved design, not yet implemented

## Problem

`akatela` and `monster-step-viewer` both render through ɴsɪ, and ɴsɪ today
means 3Delight: a closed-source binary and its closed-source OSL shaders
(`dlPrincipled`, `dlToon`, `dlMetal`, `dlNoise`). We want a second,
open-source renderer behind the same interface.

`nsi-trait` already defines the seam. Nothing implements it yet.

## Decision

Implement the ɴsɪ trait against **Mitsuba 3** (BSD-3-Clause, actively
developed), with a pure-Rust ɴsɪ layer and a `babble`-generated binding to
Mitsuba's C++ core.

### Why Mitsuba

Mitsuba is a complete renderer. We write a scene mapping; we do not write
an integrator, sampler, BSDF system, film, or shape system.

Its `Properties` class is close to an ɴsɪ node already. It carries a
plugin name, an id, and a typed key/value bag.

Nodes map well. **Connections do not** — see [Connections](#connections).
An ɴsɪ connection is a typed multi-relation whose meaning depends on its
destination attribute, and only two of the nine classes we must support
become Mitsuba references. Treating connections as references uniformly
produces a backend that looks right and binds materials to the wrong
shapes.

A GPU is optional. `scalar_*` variants run single-ray on CPU, `llvm_*`
variants run JIT-compiled vectorised CPU kernels, `cuda_*` variants need
an NVIDIA GPU. The acceleration layer covers Embree, a native builder,
OptiX and Metal.

### Alternatives rejected

**LuisaCompute / `luisa-compute-rs`.** LuisaCompute is healthy, but it is
a compute framework, not a renderer — `luisa-compute-rs` contains no
renderer at all, only a 569-line path-tracer example. We would write the
renderer _and_ the shading bridge. The Rust frontend was last pushed
2025-12-19 and our checkout is pinned to the older `siga2022-legacy`
line. Its extension seam (`native_include`) is a string pasted into
generated backend source — CUDA C++, MSL, HLSL, or C++ per backend — with
no PTX-module linking, which is the furthest of the three candidates from
what OSL emits.

**Cycles.** Already embeds generic OSL on CPU and OptiX, so it is the
cheapest route to OSL specifically. Rejected because its standalone
embedding is documented as not production-ready and its API is
Blender-shaped. Reconsider if OSL is ever promoted ahead of everything
else.

**LuisaRender.** C++, CLI-driven, JSON/text scenes, no programmatic scene
API to bind against.

**AkariRender / practical-stylized.** Both GPL-3.0 — incompatible with
akatela's MIT OR Apache-2.0 — and both archived (2026-04 and 2025-09).

## Architecture

Four new pieces, and two changes upstream in `nsi`.

### `nsi-mitsuba` (pure Rust)

Implements `nsi_trait::Nsi`. It is a **recorder**, not a renderer: a node
table, an attribute store, a connection graph, and a `render_control`
state machine. It holds no C++ and can be tested with no renderer
present.

`Nsi` has nine methods to satisfy: `create`, `delete`, `set_attribute`,
`set_attribute_at_time`, `delete_attribute`, `connect`, `disconnect`,
`evaluate`, `render_control`. All take `&self`, so the recorder uses
interior mutability.

### `bbl-mitsuba` (C++ binding definitions)

`babble` bindings over `Properties`, `PluginManager`, `Scene`, `Mesh`,
`Bitmap`, and the render entry point. This mirrors the existing
`bbl-usd` setup and builds through `bbl-build-rs`.

### `mitsuba-sys` (Rust)

Generated bindings, built by `bbl-build` from `bbl-mitsuba`.

### `nsi-osl` (deferred)

Generic OSL. Last phase; see Risks.

### Upstream changes in `nsi`

1. **`impl Nsi for nsi::Context`.** The trait and the 3Delight context
   are unconnected today. This is what lets a consumer be generic.
2. Nothing else. `nsi-ffi-wrap::c_adapter::FfiApiAdapter<T>` already
   exposes any `T: Nsi` through the C ABI, should we ever want
   `nsi-mitsuba` loadable as a drop-in `lib3delight` replacement.

### Consumer changes

`monster-step-viewer` and `akatela`/`geseon` both hold a concrete
`nsi::Context<'static>`. Each becomes generic over `T: Nsi`.

### Data flow

```
consumer  ->  T: Nsi  ->  nsi-mitsuba records
                             |
                     render_control(Start)
                             |
                       Properties tree
                             |
                       PluginManager
                             |
                          Scene  ->  integrator  ->  Film  ->  Bitmap
                                                                 |
                                                   ɴsɪ outputdriver callback
                                                                 |
                                                             consumer
```

## Mapping

### Types

| ɴsɪ `Type`                  | Mitsuba `Properties::Type`       |
| --------------------------- | -------------------------------- |
| `F32`, `F64`                | `Float`                          |
| `I32`, `I64`                | `Integer`                        |
| `String`                    | `String`                         |
| `Color`                     | `Color`                          |
| `Point`, `Vector`, `Normal` | `Vector`                         |
| `MatrixF32`, `MatrixF64`    | `Transform`                      |
| `Reference`                 | **none — never reaches Mitsuba** |

`create(handle, node_type)` becomes a `Properties` with
`set_plugin_name(node_type)` and `set_id(handle)`.

ɴsɪ `Type::Reference` is a **raw pointer**, not an object link —
`nsi-trait/src/nsi_trait.rs` notes it is "called `Pointer` in the C API;
renamed for clarity". It carries host-side addresses such as output
driver callbacks. It must not be confused with Mitsuba's `Reference` /
`ResolvedReference`, which are id-based links between scene objects. The
two share a name and nothing else. The recorder consumes `Reference`
parameters itself and never forwards them.

### Nodes

The conformance target is the closed set both consumers actually use, not
all of ɴsɪ: `ROOT`, `GLOBAL`, `TRANSFORM`, `MESH`, `NURBS`, `INSTANCES`,
`ATTRIBUTES`, `SHADER`, `PERSPECTIVE_CAMERA`, `ORTHOGRAPHIC_CAMERA`,
`SCREEN`, `OUTPUT_LAYER`, `OUTPUT_DRIVER`, `ENVIRONMENT`.

Instancing maps to Mitsuba's `shapegroup` + `instance` plugins.

### Connections

An ɴsɪ connection is not one relation. `connect(from, from_attr, to,
to_attr)` means something different for each destination attribute, and
the recorder **must classify every edge by `to_attr` before flushing**.
Only the last two rows below become Mitsuba references.

| ɴsɪ edge                                     | Meaning             | Mitsuba                                        |
| -------------------------------------------- | ------------------- | ---------------------------------------------- |
| `X → .root "objects"`                        | scene membership    | append to `Scene::shapes()` / emitters         |
| `xform → xform "objects"`                    | transform hierarchy | **flatten** into one `to_world` per leaf shape |
| `attributes → geo "geometryattributes"`      | attribute binding   | **dissolve** the node (see below)              |
| `geo → instances "sourcemodels"`             | instancing          | `shapegroup` + `instance`                      |
| `screen → camera "screens"`                  | sensor config       | sub-object on `Sensor`                         |
| `outputlayer → screen "outputlayers"`        | AOV routing         | `Film` channels                                |
| `outputdriver → outputlayer "outputdrivers"` | pixel destination   | `Bitmap` + host callback; not a scene object   |
| `shader → attributes "surfaceshader"`        | material binding    | shape's `bsdf` reference                       |
| `shader.out → shader.in`                     | shader network      | nested plugin objects; `<ref id>` for fan-out  |

Two of these are graph rewrites rather than edge classifications, and
they are the parts most likely to be got wrong:

**Transform hierarchy must be flattened.** ɴsɪ chains `transform` nodes;
Mitsuba has no transform tree, only a `to_world` per shape. The recorder
composes each chain at flush time. This is order-dependent, and motion
blur (`set_attribute_at_time`) has to be composed per time sample.

**`attributes` nodes must be dissolved.** They have no Mitsuba analogue.
An `attributes` node's `surfaceshader` becomes the shape's `bsdf`, and
its visibility flags become `Shape::visibility_mask()` — a different
mechanism entirely, not a property. One `attributes` node bound to
several shapes must fan out to each of them.

Two known losses to accept or design around:

- ɴsɪ shader connections name an **output port** (`from_attr`, e.g.
  `outColor`). Mitsuba references point at whole objects, so port
  selection needs an adapter plugin or must be resolved during flush.
- ɴsɪ permits connecting attributes of differing types; Mitsuba's plugin
  system does not. Type-invalid connections must be rejected at record
  time with a useful error rather than at render time.

### Shading, without OSL

Mitsuba ships a principled BSDF, which covers the common case directly:

| Consumer shader                 | Mitsuba          |
| ------------------------------- | ---------------- |
| `dlPrincipled`                  | `principled`     |
| `dlMetal`                       | `roughconductor` |
| `dlConstant`                    | `null` + `area`  |
| `environmentLight`              | `envmap`         |
| `checker`, `uvCoord`, `dlNoise` | texture plugins  |
| `dlToon`                        | no analogue      |

`dlToon` is confirmed not load-bearing, so the native mapping is
sufficient and generic OSL is a later capability rather than a
prerequisite.

### Geometry

Mitsuba has no NURBS and no subdivision surfaces. Shapes are
`bsplinecurve`, `cube`, `cylinder`, `disk`, `ellipsoids`, `instance`,
`linearcurve`, `merge`, `obj`, `ply`, `rectangle`, `sdfgrid`,
`serialized`, `shapegroup`, `sphere`.

3Delight tessellates trimmed NURBS natively; Mitsuba cannot. We
tessellate ahead of it using `monstertruck-meshing`, which already
provides `compressed_trimmed_shell_triangulation`,
`..._with_isoparams`, and `robust_` variants of both.

### Incremental edits

ɴsɪ is a mutable graph with `render_control(Synchronize)`. Mitsuba
supports this, if bluntly:

`Scene::shapes()` returns a non-const `std::vector<ref<Shape>>&`.
`Scene::parameters_changed()` (`src/render/scene.cpp:770`) scans shapes
and shapegroups for `dirty()`, and on any dirt calls
`m_accel.rebuild(this)`, `clear_shapes_dirty()`,
`update_instance_transforms()`, then recomputes the bounding box.

So `Synchronize` becomes: mutate the shape vector, mark dirty, call
`parameters_changed()`. This is a **full acceleration-structure rebuild**,
not a refit, and will therefore be slower than 3Delight's incremental
updates. Interactive editing remains correct but should not be assumed
to match 3Delight's latency.

## Phases

`monster-step-viewer` is the target through Phase 4: it is a fraction of
akatela's size, uses the identical ɴsɪ surface, and exercises the NURBS
path. akatela follows in Phase 5.

```
Phase 1  impl Nsi for nsi::Context; make consumers generic over T: Nsi
         verify: 3Delight path renders a byte-identical golden image

Phase 2  nsi-mitsuba recorder + connection classifier, pure Rust, no C++
         verify: replay a recorded scene, emit an .nsi stream, diff it
                 against 3Delight's own apistream output
         verify: every edge class in Connections is classified, and an
                 unknown to_attr is an error rather than a silent
                 reference; transform chains flatten to the expected
                 to_world; attributes nodes dissolve to the expected
                 (shape, bsdf, visibility_mask) triples

Phase 3  bbl-mitsuba + mitsuba-sys; flush recorder -> Properties -> render
         verify: TRANSFORM / MESH / camera / SCREEN / OUTPUT_LAYER /
                 ENVIRONMENT render; compare against 3Delight
         verify: a scene with two shapes and two distinct materials binds
                 each material to the correct shape (the failure mode a
                 uniform connection->reference mapping would hide)

Phase 4  Native BSDF and texture mapping
         verify: monster-step-viewer renders without 3Delight

Phase 5  NURBS via monstertruck-meshing; akatela as consumer
         verify: brep fixtures (nist_ex1.iges, boxy_with_surfacetex.stp)

Phase 6  Generic OSL, CPU first
         verify: an arbitrary .osl shader compiles and shades
```

Phase 1 is worth doing whether or not Mitsuba works out — it is what makes
any second backend possible. Phase 2 is verifiable against 3Delight before
a line of C++ exists.

## Risks

**Silent connection miscategorisation.** The highest-consequence failure
mode in this design. A connection mapped to the wrong Mitsuba concept
does not error — it renders, with materials on the wrong shapes,
instancing collapsed, or output routed nowhere. Mitigation: the
classifier is exhaustive over `to_attr` and rejects unknown attributes
rather than defaulting to a reference, and Phase 3 carries an explicit
two-shape / two-material binding test.

**`m_shapes_dr` refresh.** Unverified: whether `m_accel.rebuild()`
refreshes the Dr.Jit shape-pointer buffer when shapes are added or
removed, or whether that needs doing by hand. Settle during Phase 3 —
it is the one thing that could reshape the recorder's flush strategy.

**Accel rebuild cost.** Every `Synchronize` rebuilds. If interactive
editing proves too slow, the mitigation is to batch edits and to keep
static geometry in shapegroups so only instance transforms change.

**Two LLVMs in one process.** OSL 1.14+ requires LLVM 18 and links it;
Dr.Jit loads LLVM dynamically at runtime. Symbol collision is plausible.
This only affects Phase 6, so it no longer gates the design — but it must
be probed before Phase 6 is committed to.

**OSL on GPU is OptiX-only.** OSL emits CPU machine code or PTX for
CUDA+OptiX, and nothing else — no SPIR-V, no Metal, no HLSL. Even Cycles
supports GPU OSL only through OptiX. Whenever Phase 6 happens, generic
OSL will be CPU-everywhere and GPU-NVIDIA-only.

**3Delight's `dl*` shaders are closed source.** Generic OSL will not run
them; they ship as compiled `.oso`. The native BSDF mapping in Phase 4 is
the answer for those, not OSL.

## Open questions

None blocking. `dlToon` was confirmed not load-bearing, which is what
allowed OSL to move to last.
