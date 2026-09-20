# Feature: Draft Attribute And Node Names As The Default

## Problem

ɴsɪ's shipped attribute names grew one at a time: `numberofthreads`
next to `networkcache.size`, `nu` next to `trimcurves.ncurves`,
`sourcemodels` on `instances` meaning what `objects` means everywhere
else. The [naming convention
redesign](https://nsi.readthedocs.io/en/latest/naming-convention.html)
settles a vocabulary for all of them -- hyphenated words, grouped by
concern, no jargon -- and maps every shipped name onto it.

A scene recorded by `nsi-intermediate` is the place that mapping can
live once, instead of in every renderer and exporter built on it. Today
the crate carries whatever spelling the stream used, so a renderer must
know both and a scene written by two exporters is inconsistent with
itself.

## User Stories

1. **As a renderer author**, I read a scene by the draft's names
   (`u.count`, `thread-count`) whatever spelling its exporter used, so
   my code carries one vocabulary.
2. **As an exporter author**, my existing stream still loads, and the
   log tells me once per name which spellings are deprecated and what
   replaces them.
3. **As a 3Delight user**, a scene written back out is the spelling the
   shipped renderer reads, so nothing has to change downstream yet.

## Acceptance Criteria

- Reading a scene canonicalizes the shipped names to the draft's: a
  `nurbs` node's `nu` is stored as `u.count`, `nicename` on any node as
  `nice-name`. A legacy spelling is warned about once per name, with
  the replacement, through `log::warn!`.
- The mapping is scoped by node type, as the draft's tables are: `P` on
  a `particles` node and on a `mesh` node are the same name, but
  `sourcemodels` renames only on `instances`.
- Connection destinations rename with the rest: `geometryattributes`
  becomes `attributes`, `outputlayers` becomes `output-layers`. On an
  `instances` node a connection to `objects` is the instance source
  that `sourcemodels` used to name; on every other node it is scene
  membership, and the edge kinds say so.
- Node types rename where the draft names them: `outputdriver` becomes
  `output-driver`, `vdbparticles` becomes `vdb-particles`. A scene can
  be created with either spelling; `Node::node_type` answers with the
  draft's.
- Writers emit the shipped spellings: a scene canonicalized on the way
  in comes back out as the stream a 3Delight of today reads, node types
  included. A stream therefore round-trips unchanged.
- An unknown name is carried as written, never guessed at.

## Non-Goals

- **The ᴏsʟ globals keep their names.** `P`, `N`, `Pw`, `Ng`, `u`, `v`,
  `dPdu`, `dPdv` and `I` are what a shader reads them as, and the draft
  leaves the conflict open (its Option A against Option B). This
  feature takes Option A: they are not renamed and not deprecated.
- **The rows the draft marks as API changes are not renames.**
  `trimcurves.u`, `.v` and `.w` becoming one interleaved
  `trim-curves.position` changes the values, not just the name; so does
  `trimcurves.inside` becoming `trim-curves.hole`, which is one value
  per loop and inverted -- `inside` 1 keeps the surface inside a loop,
  `hole` 1 removes it. Aliasing those would silently invert a trim.
  They are carried as written, and the reader that cares reports them:
  `nsi-tessellate` refuses a `nurbs` node with `trimcurves.inside` 0
  rather than tessellate the wrong side.
- **API arguments** -- `NSIBegin`'s, `NSIEvaluate`'s,
  `NSIRenderControl`'s -- are not attributes on nodes; the draft's
  twelve rows for them are out of scope here.
- Two camera types the draft's tables do not name, `sphericalcamera`
  and `orthographiccamera`, are left alone rather than renamed by
  analogy.
