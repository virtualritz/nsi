# Plan: Weld Declarations In `nsi-intermediate`

## Approach

- **Classification.** `EdgeKind::Weld` for a connection into `weld`.
  `EdgeKind` is `#[non_exhaustive]`, so the variant is additive.
- **Parsing, in `resolve/weld.rs`.** `Scene::weld_uses(geometry)` reads
  the geometry's table into `WeldUse { weld, id, segments }`, each
  segment a `WeldSegment { kind, index, reverse, range }`. Defaults per
  the draft: one segment per use, not reversed, full range.
- **Validation against the geometry.** A `trim-loop` names a loop that
  exists (the number of `trimcurves.ncurves` values); a `trim-curve` a
  curve that exists (their sum); a `nurbs-side` 0--3 on a `nurbs`; a
  `mesh-edge` a face, loop and edge that exist, `nholes` included.
- **Grouping.** `Scene::welds()` returns every `(weld node, id)` with its
  uses, in scene order.
- **Diagnostics as data**, like `order_decided`: `WeldProblem` names the
  geometry, the use and what is wrong, with a `Display` a backend
  prints.

## What The Tessellator Needs (spec 007)

Welds name *edges*. A tessellator also needs *vertex* identity at the
ends of welded edges, and derives it topologically: consecutive curves
of a trim loop share their junction, and a shared edge identifies the
junctions at its ends on every face that uses it. A union-find over
those links gives vertex classes without comparing positions -- the
draft's "spatial coincidence alone does not declare a join". So this
crate exposes, per use, its ordered segments, and per trim loop its
curve order; the union-find belongs to the tessellator.

## Gates

1. The draft's own examples -- the five-to-one join, the explicit
   five-curve form, two independent joins, a self-seam, a non-manifold
   join -- parse to the expected uses and groups.
2. Every invalid declaration the draft names comes back as a
   `WeldProblem`, one test each, and each test is falsified.
3. A stream carrying a weld table round-trips through
   `write_stream` and `nsi-parse` unchanged.
