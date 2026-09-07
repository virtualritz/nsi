# Research: `nsi-tessellate`

## Decisions

### D1: Two published kernels, no third implementation

`subdiv-kernels` 0.1 and `monstertruck` 0.4 (its `monstertruck-meshing`
member) already carry the machinery. This crate maps ɴsɪ onto them and
implements no subdivision or patch evaluation of its own.

`subdiv-kernels` is a good fit for a reason worth stating: it "holds no
geometry and needs no host mesh type". You supply the control cage's
topology and it returns [`StencilTable`]s -- sparse maps where each
output point is a weighted sum of a few input points -- which you then
apply to *your own* per-vertex data. So positions, `st`, `N` and every
user variable go through the same tables, and this crate never owns a
mesh type.

*Rejected:* wrapping either crate's types. `nsi-profile` re-declared a
shader network that `nsi-intermediate` already held, and the lesson is
recorded there.

### D2: The driver is a tolerance, and a view is one way to compute one

Screen-space error needs a camera; edge length and chord tolerance do
not. Both are the same parameter at the kernel -- a number that says
how fine -- so the API takes the tolerance and offers a helper that
derives one from a camera and a resolution.

That keeps the 3D-printing case a first-class caller rather than a
degenerate view: a dumped mesh wants a tolerance in scene units, and
asking it to invent a camera would be backwards.

### D3: Primitive variables ride the stencils, per class

`nsi-intermediate`'s `Scene::primitive_variable` already answers what
ɴsɪ leaves implicit: the interpolation class, and which value each
face-vertex reads. That maps onto `subdiv-kernels` as:

| class | how it refines |
| --- | --- |
| `Constant` | carried unchanged |
| `Uniform` | one value per face, mapped through the refined face's parent |
| `Vertex` | through the vertex stencils, like positions |
| `FaceVarying` | through `face_varying_stencils`, with a `FaceVaryingInterpolation` rule |

The classification must not be redone here. Two encodings of one rule
drift -- four times inside `nsi-intermediate` alone.

### D4: A polygon mesh is not tessellated

Without `subdivision.scheme` a `mesh` **is** the geometry. Refining it
would hand a consumer a different shape than the scene described, and
a consumer that wants triangles from an n-gon wants triangulation, not
subdivision. Triangulating faces is a separate, honest operation and
belongs behind its own call.

### D5: Sparse re-evaluation is the interactive path

`subdiv-kernels` exposes `affected_outputs` and `evaluate_sparse`;
`nsi-intermediate` exposes `Scene::affected`, which names the roots a
`synchronise` touched. The two compose: an edit names roots, the roots
name control points, and only their dependent output points are
re-evaluated. This is why `Changes`/`Affected` was built, and the
first consumer that can prove it.

## Open Questions

### R1: how an ɴsɪ `nurbs` patch reaches `monstertruck-meshing`

That crate's entry points are shell-oriented --
`shell_triangulation`, `trimmed_shell_triangulation`,
`compressed_trimmed_shell_triangulation_with_isoparams` -- with
`TessellationOptions { tolerance, search_trials, primitive }`. Its
geometry member has `NurbsSurface` and `NurbsCurve`, and the
triangulator's internals are generic over a `PreMeshableSurface`.

Open: whether a single ɴsɪ `nurbs` node can be handed to a
surface-level entry point, or must be wrapped as a one-face shell.
Evidence to gather: whether `PreMeshableSurface` is public, and what
`NurbsSurface` needs beyond control points, knots and order.

### R2: what an ɴsɪ `nurbs` node's attributes are called

The node exists in the renderer and in no manual. `lib3delight.so`
carries `nurbs`, `NSINurbsNode.cpp`, `number_of_nurbs_patches` and
`"%.0f NURBS surfaces (%.0f patches)"`. A string search of that
library finds `nvertices`, `nholes` and `clockwisewinding` but none of
`uorder`, `vorder`, `uknots`, `vknots`, `Pw`, `position` or
`knot-interval`.

`nsi.readthedocs.io` documents a `nurbs` node using the *new* naming
convention throughout, on the stated grounds that "no renderer
implements it yet, so there is nothing to be backward-compatible
with". That premise is false against 2.9.207.

Evidence to gather: render a `nurbs` node through 3Delight with
candidate attribute spellings and see which it accepts -- the same
oracle method `contracts/stream.md` uses. Until then this crate
tessellates subdivision surfaces only, and `nurbs` support waits on
the renderer's answer rather than on a guess.

## References

- `subdiv-kernels` -- <https://github.com/virtualritz/subdiv-kernels>,
  `Scheme`, `BoundaryInterpolation`, `CornerRule`,
  `CreaseComputationMethod`, `FaceVaryingInterpolation`,
  `face_varying_stencils`, `affected_outputs`, `evaluate_sparse`.
- `monstertruck-meshing` -- "Mesh algorithms: tessellation of CAD
  shapes, mesh analysis, and mesh optimization filters."
- `specs/003`'s `Scene::primitive_variable`, `Scene::faces` and
  `Scene::affected`, which are this crate's whole input.

### D6: edge identity is the crack fix, and the draft already specifies it

Cracks between adjacent patches are not a tessellation-quality
problem to be solved with more triangles. Two patches meeting along a
shared model edge are tessellated independently, each approximating
that edge from its own side, and the two approximations differ. Under
displacement the gap opens further, because each side displaces its
own sampling of a curve the other side sampled elsewhere.

**The fix is identity, not density**: tell the consumer which
boundaries are the *same* 3D edge, and it can weld them -- evaluate
the shared arc once and use it from both sides.

`nsi.readthedocs.io`'s draft already defines this, in a
**scene-global edge-identifier space**:

- On `nurbs`: `stitch.edge-id` as `int[4]` for the four parametric
  borders (`u = u.min`, `u = u.max`, `v = v.min`, `v = v.max`) with
  `stitch.edge-orientation` beside it, and
  `trim-curves.edge-id`/`trim-curves.edge-orientation` where the
  boundary is a trim curve instead.
- On the draft `t-nurcc` node: `stitch.index`, a list of boundary cage
  edges as *ordered* pairs of position indices -- "pair order is
  significant… no separate orientation attribute is needed" -- with
  one `stitch.edge-id` per pair.

The rule is one sentence: *"Boundaries anywhere in the scene that
carry the same non-negative value trace the same model edge in 3D, and
the renderer welds them."* `-1` means no identity.

**And the data exists on the way in.** `monster-step-viewer` converts
each face of a `CompressedTrimmedShell` into ɴsɪ `nurbs` attributes,
and its `NsiBrepSurfaceData` "mirrors NSI's `nurbs` node attribute
names exactly": `nu`, `nv`, `uorder`, `vorder`, `uknot`, `vknot`,
`umin`/`umax`, `vmin`/`vmax`, `pw`, plus a `trimcurves.*` block. It
walks `CompressedEdge` and `CompressedEdgeUse`, so the B-rep's shared
edges -- exactly the identity the draft wants -- are in hand at emit
time. ɴsɪ 2.9 has nowhere to put them, and that emitter records the
consequence itself: where an exact trim is unavailable it falls back
to a sampled parameter boundary and keeps a per-face diagnostic count
"because those trims can create visible cracks".

So the identity is discarded at the ɴsɪ boundary and every consumer
downstream re-derives it or cracks. That is the gap.

## Open Questions

### R3: whether identity should reach beyond patches

The draft puts stitching on `nurbs` and `t-nurcc`. Nothing carries it
for a `mesh`, subdivision or otherwise -- yet a subdivision cage
boundary welded to a patch is the same problem, and the `t-nurcc`
attribute is already defined in terms of "the arc of the limit-surface
boundary that the listed cage edge maps to", which is a subdivision
notion.

Extending `stitch.index`/`stitch.edge-id` to `mesh` would make edge
identity a property of *any* geometry boundary, which is what a ᴄᴀᴅ
consumer needs: a shell mixing analytic faces, patches and meshed
regions welds along one identifier space or not at all.

This is an ɴsɪ specification question rather than a crate one, and
worth putting to the specification's authors with the
`monster-step-viewer` evidence attached. What this crate can do
meanwhile: consume the identifiers where they exist, and refuse to
pretend two boundaries are welded where they do not.
