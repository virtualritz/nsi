# Research

## R1: How many names, and of what kinds?

The draft's *Complete Attribute Mapping* has 176 rows across 22 node
sections and five API sections. Parsed:

| Kind | Rows | Here |
| ---- | ---- | ---- |
| One name to one name | 147 | renamed |
| Expanded per node type (cameras share a section) | 153 pairs | renamed |
| ᴏsʟ globals (`P`, `N`, `Pw`, `u`, `v`, `Ng`) | 6 | kept |
| New attributes, no shipped name | 5 | nothing to map |
| Marked *API change* (the two consolidations, and `trimcurves.inside` -> `trim-curves.hole`) | 3 | carried, reported by readers |
| Unchanged (`trimcurves.order` and kin) | 3 | nothing to map |
| API arguments | 12 | out of scope |

## R2: Why scope by node type?

The draft's tables are per node, and the same word means different
things on different nodes. `objects` on `instances` is what
`sourcemodels` named -- the models being instanced -- while `objects`
everywhere else is scene membership. A flat table would make those one
name and one edge kind.

## R3: Why canonicalize on the way in rather than on lookup?

Both were considered. Resolving aliases at lookup keeps every existing
reader working, but leaves a scene holding two spellings of one
attribute, so two exporters' output does not compare equal and
`Node::attributes` answers with whatever was written. Canonicalizing on
the way in makes the scene's vocabulary single, at the cost of updating
readers in this repository to the draft's names.

## R4: Why warn once per name, not once per occurrence?

A mesh with a thousand `nvertices` attributes would otherwise print a
thousand lines. The name, not the occurrence, is what an exporter
author has to fix.

## R5: Where the mapping comes from

`names/table.rs` is generated from the draft by
`crates/nsi-intermediate/tools/names_from_draft.py`, which parses the
published Markdown. The generator lives in the
repository so the table can be regenerated when the draft moves; the
generated file is committed, so a build needs no network and the docs
are not a build dependency.
