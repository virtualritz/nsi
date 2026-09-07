# Plan: What Changed Since The Last Synchronise

## Shape

```
ɴsɪ calls ──▶ Scene (records)  ──▶ take_changes ──▶ Changes
                                                      │
                                   affected(&Changes) ▼
                                                   Affected
                                                      │
                              descendants(root) ◀─────┘  (only if asked)
```

Two halves, deliberately separate:

1. **The record** is written by the mutators, because a delete takes
   its edges with it and a re-armed connection leaves the graph looking
   exactly as it did. That half cannot be reconstructed after the fact.
2. **The walk** is computed on demand from the record, because it is
   the inverse of walks this crate already does upward, and inverting
   them needs no new index -- `by_to_attr` already is the inverse.

## Gates

| Gate | Met |
| --- | --- |
| The record is net and valueless | yes |
| What a mutator destroys is recorded first | yes |
| `changed ⊆ affected` under a mutation-checked property gate | yes |
| The answer is constant in scene size for one edit | yes, measured |
| Clippy `-D warnings`, rustdoc `-D warnings`, `fmt` | yes |
| A backend drives it end to end | **no** -- `nsi-moonray` has not built against it |

## What This Does Not Settle

The consumer has not run. `nsi-moonray`'s flush does not build against
this crate's current API at all (it indexes the now-private `nodes`,
reads `time_attrs` under its old name, and treats `world_transform` as
non-`Result`), and it reads `Node::attributes` directly where the crate says
to use `Node::effective` -- so an attribute set through
`SetAttributeAtTime` is invisible to it today, before any interactive
work. Until it builds, every row above is proven by this crate's own
tests and the renderer, not by a backend.

That is also why `nsi-intermediate` is unpublished: the API a backend
binds to should be one a backend has bound to.

## Order Of Work

Done, in this order, each gated: the record; the walk; the review
round; the fixes it found; the shape and cost work (roots, borrowing,
streaming instances, batch composition).
