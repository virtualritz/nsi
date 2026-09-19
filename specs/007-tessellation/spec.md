# Feature: `nsi-tessellate`

## Problem

ɴsɪ describes geometry a renderer may not be able to intersect
directly. A `mesh` with `subdivision.scheme` is a control cage for a
limit surface; a `nurbs` node is an analytic patch. 3Delight
intersects both analytically and **never tessellates** unless a
displacement shader forces it. Many other consumers cannot: a
rasteriser, a GPU ʙᴠʜ builder, a realtime backend, or a tool dumping
a mesh for 3D printing all need explicit triangles.

Today each of those writes its own subdivision and patch evaluation,
or drops the geometry. The machinery already exists in two published
crates -- `subdiv-kernels` for subdivision stencils and
`monstertruck-meshing` for ᴄᴀᴅ tessellation -- and what is missing is
the piece that maps ɴsɪ's description onto them.

## The Position In The Chain

    nsi-trait / nsi-ffi-wrap → nsi-intermediate → nsi-tessellate → consumer

**Optional, not a stage.** An analytic renderer goes straight from
`nsi-intermediate` to its own scene; only a consumer that needs
explicit geometry calls this crate. It is the consumer that calls,
never the other way round: nothing here pushes at a renderer.

## User Stories

1. **As a realtime backend**, I hand a resolved `Scene` handle and a
   camera to `nsi-tessellate` and get triangles whose density follows
   screen-space error, so a distant asset costs what it looks like.
2. **As a 3D-printing tool**, I ask for the same geometry with **no
   view at all** -- a maximum edge length, or a chord tolerance -- and
   get a watertight triangle mesh.
3. **As a ʙᴠʜ builder**, I get positions *and* every primitive
   variable carried through the refinement, because `st` and `N` on
   the control cage must land on the refined mesh or the shading is
   wrong.
4. **As an interactive host**, I re-tessellate only what moved:
   `Scene::affected` names the roots, and `subdiv-kernels` has sparse
   re-evaluation (`affected_outputs`, `evaluate_sparse`) to match.

## Watertightness

A consumer that tessellates two adjacent patches independently gets a
crack between them, because each approximates the shared edge from its
own side and the two approximations differ. Under displacement the gap
opens further. **More triangles do not close it** -- the fix is
knowing that the two boundaries are the same 3D edge, so the shared
arc is evaluated once and used from both sides.

ɴsɪ's draft already defines that identity: a scene-global
edge-identifier space, exposed as `stitch.edge-id` on `nurbs` (one per
parametric border) and `stitch.index`/`stitch.edge-id` on the draft
`t-nurcc` node, where "boundaries anywhere in the scene that carry the
same non-negative value trace the same model edge in 3D, and the
renderer welds them".

This crate **consumes** those identifiers where a scene supplies them
and welds accordingly; it never guesses that two boundaries are the
same edge from proximity, because a wrong weld is a fused model rather
than a visible crack. Where a scene carries no identity, the output is
honest about being unwelded.

See `research.md` D6 for the evidence, including a ᴄᴀᴅ consumer that
holds the identity at emit time and has nowhere in ɴsɪ 2.9 to put it,
and R3 for whether identity should reach `mesh` boundaries too.

## Acceptance Criteria

- A `mesh` with `subdivision.scheme = "catmull-clark"` tessellates
  through `subdiv-kernels`, with ɴsɪ's crease, corner and boundary
  attributes mapped onto that crate's rules.
- A `nurbs` node tessellates through `monstertruck-meshing`.
- Primitive variables come through in their resolved interpolation,
  using `nsi-intermediate`'s `PrimitiveVariable` -- face-varying
  channels through `subdiv-kernels`'s face-varying stencils, uniform
  and constant carried as they are.
- **Both drivers work**: a view (screen-space error) and no view
  (edge length or chord tolerance). Neither is the default the other
  has to pretend to be.
- A polygon `mesh` with no `subdivision.scheme` passes through
  unchanged -- tessellating it would be a lie about what the scene
  said.
- Two boundaries carrying the same `stitch.edge-id` come back welded:
  one shared arc, evaluated once, with vertices shared rather than
  merely coincident. Two boundaries carrying `-1`, or none at all, come
  back unwelded and say so.

## Non-Goals

- **Displacement.** It needs shader execution, which is a renderer's
  job and `nsi-profile`'s vocabulary question.
- **Being on the default path.** An analytic renderer must never need
  this crate; if it does, the layering is wrong.
- **Its own geometry types.** The input is `nsi-intermediate`'s
  `Scene`; the output is a triangle buffer plus attribute buffers, not
  a new scene graph. Duplicating either crate's types is the mistake
  `nsi-profile` already made.
- **Curves and volumes**, until a consumer asks.

## Risks

- **ɴsɪ's `nurbs` node is undocumented.** It ships in 3Delight
  2.9.207/208 -- `lib3delight.so` carries `NSINurbsNode.cpp`,
  `number_of_nurbs_patches` and the node type string -- but neither
  shipped manual describes it, and `nsi.readthedocs.io`'s addendum
  believes "no renderer implements it yet". Its attribute names must
  be established against the renderer before they are relied on.
- **`monstertruck-meshing` is shell-oriented.** Its entry points take
  ᴄᴀᴅ shells (`shell_triangulation`, `trimmed_shell_triangulation`),
  while an ɴsɪ `nurbs` node is a single patch. Mapping one onto the
  other is an open question, recorded in `research.md`.
- A refinement that silently drops a primitive variable shades
  plausibly and wrongly, which is the failure `nsi-intermediate`'s
  `AmbiguousInterpolation` exists to prevent. The same discipline
  applies here: refuse rather than guess.

## Addendum (2026-09-19): NURBS With Weld Declarations

The draft this spec cited, `stitch.edge-id`, has been superseded by
[weld declarations](https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html),
which `nsi-intermediate` now resolves (spec 013). This addendum
replaces the `stitch.edge-id` criterion above.

### Acceptance Criteria

- Behind a `nurbs` feature, `nurbs_meshes(scene, options)` tessellates
  every `nurbs` node through `monstertruck-meshing`, reading the shipped
  3Delight 2.9.210 attributes (`nu`, `uorder`, `uknot`, `P` or `Pw`,
  `trimcurves.*`).
- The `nurbs` nodes of one weld namespace are meshed as **one shell**:
  each `(weld, id)` group is one shared edge, so both sides sample it
  once. Vertex identity at edge ends comes from topology -- loop
  junctions linked through shared edges -- never from positions.
- Along a welded edge, the output shares positions **and normals**
  across faces, bit for bit, so a displacement that depends on `P` and
  `N` displaces both sides identically.
- A closed solid with complete welds comes back with **no open edges**;
  the same solid without welds comes back with open edges -- the
  negative control that makes the first result mean something.
- One mesh per ɴsɪ node, so per-face attributes and shaders still apply;
  the shared seams are what make them one watertight surface.
- Orientation follows 3Delight's convention for `nurbs`: the front is
  the side ∂P/∂u × ∂P/∂v points to. The convention is established by
  rendering 3Delight's own `nurbs`, not by reading the manual, which
  does not state it.
- Displaced by a shader that pushes along `N`, a welded solid renders
  closed and the same solid unwelded renders cracked, in 3Delight.

### Evidence

`tests/displacement.rs` renders a cube with a camera at a corner, a
shader that paints back faces red, and a displacement of 0.15 along `N`
(3Delight needs the undocumented `displacementbound` attribute to
displace at all). Red pixels are surface seen from inside.

| Subject                                   | Red pixels |
| ----------------------------------------- | ---------- |
| 3Delight `nurbs`, u × v outward           | 0          |
| 3Delight `nurbs`, u × v inward            | 11488      |
| Tessellated, welded, displaced            | 0          |
| Tessellated, unwelded, displaced          | 3753       |

Falsified: skipping the merge of welded seam points, or flipping the
shell orientation, each fails the suite.

#### A Real Part

`nsi-procedural`'s `step_procedural` example emits `io1-ec-214.stp`, a
flange with a bore and six holes, as 17 `nurbs` nodes welded along 35
edges. `tests/step_displacement.rs` tessellates it and renders it
displaced by 1.0 along `N`.

The cube hid four defects that the part exposed. The first three are
fixed in `nsi-tessellate`:

- **Rounding past the domain.** A trim that should end at 2π ends at
  `6.283186` in `f32`, past the knot end `6.2831855`, and the mesher
  folds the face over itself (face area 3.2× and 7.8× the true one).
  Parameters a rounding outside the domain are clamped onto it.
- **Closed edges have no direction by their ends.** A circle starts and
  ends at one vertex, so comparing ends made every use of it "forward".
  Direction is decided by the ends and the quarter points.
- **Samples moved by rounding.** On periodic surfaces the mesher
  re-evaluates boundary samples from their parameters, and a loop's
  closing vertex can come back twice, a rounding apart, inside a fold of
  slivers. Vertices are snapped onto the shared samples of the welded
  edges *their own face* uses. A boundary vertex snaps within the
  tolerance and any other vertex within a hundredth of it. Triangles
  that collapse in the merge are dropped.

The fourth is in `monstertruck-meshing` 0.4.0. It projects a boundary's
first point onto a trim with no hint. On a trim closed in space but
open in `uv` (a cylinder's circle, `u` from 0 to 2π) both ends match, so
Newton can land on the far end. The walk then runs off the curve, and
the `uv` boundary spans 12 laps. The fix is to seed the projection at
the trim's start, in `impl ExactTrimBoundary2D for ParameterCurve`:

```rust
fn project_boundary_point(&self, point: Point3, hint: Option<f64>) -> Option<(f64, Point2)> {
    let hint = hint.or(Some(self.curve().range_tuple().0));
    self.search_parameter(point, hint, 100)
    // ... unchanged
```

| Tessellation of the part          | Open edges, welded | Unwelded |
| --------------------------------- | ------------------ | -------- |
| Before the three fixes            | 1819               | 3940     |
| With them, stock `monstertruck`   | 172                | 3940     |
| With them and the upstream fix    | 0                  | 3940     |

With the upstream fix, displaced by 1.0, the welded part renders closed
(0 inside pixels) and the unwelded part cracks (1268). The fix shipped
in `monstertruck` 0.4.1 (2026-09-19), which `nsi-tessellate` now
requires.

#### Natural Sides (2026-09-19)

A STEP face whose only trim loop traces its surface's domain is, in ɴsɪ,
an untrimmed patch; most of `io1-ec-214`'s faces and half of `boxy`'s
are. Their shared edges are natural sides, so welding them needs
`nurbs-side`, against another side or against a trim curve.

- A patch that declares side welds gets its active domain's outline
  (`umin`..`vmax`, else the knot range) as one more boundary loop,
  counter-clockwise in `(u, v)`. Each side is a straight trim, and the
  sides join the same shared-edge machinery as trim curves.
- `weld.range` splits a side or a trim curve where its selections begin
  and end, and each part belongs to the use whose range covers it. So a
  side can be shared, part by part, with several neighbours.
- A use through both a side and trim curves is reported and left
  unwelded. The sides form a loop of their own, and no STEP export here
  produces such a use.
- `weld.reverse` is not needed to weld: direction comes from the
  geometry, as for trim curves.
- The STEP procedural drops full-domain loops whether it welds or not.
  With welds, it declares each edge-use such a loop traced as
  `nurbs-side` segments, with the part of the side as `weld.range` and
  its direction as `weld.reverse`.

Evidence:

| Fixture | Welded open edges | Unwelded |
| ------- | ----------------- | -------- |
| Cube, faces 0, 2, 4 by sides, rest by trims | 0 | > 0 |
| Cube, all six by sides | 0 | > 0 |
| The mixed cube, every edge two welds by range | 0 | > 0 |

The mixed cube, halves and all, displaced by 0.15 in 3Delight renders
closed (0 inside pixels) where unwelded it cracks (3753).

On the real parts, `io1-ec-214` has 12 of 17 faces untrimmed (48 side
segments, 22 trim segments) and `boxy` 43 of 80 (116 and 132). Every
weld on both is closed and manifold. With the `monstertruck` fix,
`io1-ec-214` tessellates with 0 open edges and, displaced by 1.0,
renders closed (0 inside pixels) where unwelded it cracks (1267).

Falsified:
- building no domain loop fails every side test;
- ignoring ranges fails the range test, and so does a fixture that
  counts ranges from the wrong end;
- a procedural declaring no sides fails `untrimmed_faces_weld_by_their_natural_sides`
  and the manifold test, and leaves the real part with 3940 open edges.

### Non-Goals
- Mesh-edge welds between `nurbs` and subdivision surfaces.
