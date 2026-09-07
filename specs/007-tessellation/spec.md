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
