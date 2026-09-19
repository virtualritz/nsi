# Feature: Weld Declarations In `nsi-intermediate`

## Problem

A B-rep solid is many surfaces that share edges. Tessellate each
surface on its own and the shared edges come out as two different
polylines: a crack, which displacement opens further. More triangles do
not close it. What closes it is knowing *which* boundaries are the same
edge, so a tessellator can build that edge once and use it from both
sides.

The ɴsɪ draft [Shared Boundaries: Weld
Declarations](https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html)
states that identity. A `weld` node is a namespace; each geometry node
connected to one carries a table of *boundary uses*, and two uses with
the same `(weld node, id)` are one joined boundary. The draft leaves
how the join survives tessellation and displacement to the renderer.

`nsi-intermediate` records scenes for renderers built on it. Today it
carries a `weld` connection as an unclassified `Other` edge and a
geometry's `weld.*` attributes as opaque values, so a renderer would
have to re-implement the draft's parsing and validation itself.

## User Stories

1. **As a tessellator**, I ask the scene for every joined boundary --
   each `(weld, id)` with its uses, each use with its geometry and its
   ordered segments -- and build shared edges from that.
2. **As a renderer author**, a malformed declaration (counts that do
   not partition the arrays, an unknown kind, an index out of range)
   comes back as a diagnostic naming the geometry and the use, not as a
   silently wrong weld.
3. **As an exporter author**, the table I write -- including the
   `trim-loop` and multi-segment forms -- round-trips through a stream
   unchanged.

## Acceptance Criteria

- `EdgeKind::Weld` for a connection into `weld`; a geometry with more
  than one `weld` connection is reported, since the draft allows one.
- `Scene::weld_uses(geometry)` parses the geometry's table: `weld.id`
  per use; `weld.segment-count` per use (default: one segment each);
  per segment `weld.kind`, `weld.index` (`int[3]`), `weld.reverse`
  (default 0) and `weld.range` (`float[2]`, default `[0 1]`).
- The four kinds: `trim-loop`, `trim-curve`, `nurbs-side`, `mesh-edge`,
  with the draft's index meanings, validated against the geometry's own
  arrays (a trim loop that exists, a side 0--3, a face/loop/edge that
  exists). A `trim-loop` use is complete: one segment, full range.
- `Scene::welds()` groups every use in the scene by `(weld node, id)`.
  A group with one use is an open declaration and says so; more than two
  uses is a non-manifold join, reported, not refused.
- Every violation the draft names -- sums that do not match, empty
  uses, invalid indices, a closed loop mixed with other segments -- is a
  diagnostic with its geometry and use index, as `order_decided`
  reports ties: data a backend prints, not a panic and not a silent
  drop.
- Legacy shorthand (`trim-curves.edge-id`, `stitch.edge-id`) is out of
  scope: 3Delight ships neither, and the draft lowers them to this table.

## Non-Goals

- **Geometric validation.** Whether two uses really trace the same path
  within tolerance is the exporter's guarantee; checking it needs surface
  evaluation, which is the tessellator's (spec 007).
- **Chain connectivity in space.** "The selected segments must form one
  connected chain" is checked topologically where the kind allows it
  (consecutive trim curves of one loop, consecutive mesh edges); a chain
  of trim curves from different loops is reported as unverifiable, not
  refused.
- **Occurrence scope for instances.** The draft leaves it open. A welded
  geometry reached through an `instances` node is reported.

## Risks

- **The draft is a draft.** Attribute names and the index encoding may
  change. They are read in one module, so a change is local.
- **Names on the shipped node.** The draft's examples use the draft
  `nurbs` vocabulary; the shipped node counts loops by the number of
  `trimcurves.ncurves` values. The resolver reads the shipped names.
