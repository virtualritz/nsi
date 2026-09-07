# Research: What Changed Since The Last Synchronise

Decisions, with what was rejected and why.

## D1: A net record, not a journal of calls

`nsi-moonray`'s `002` asked for "a journal: creates, deletes, attribute
sets, connection changes". Rejected as the stored shape.

A journal entry is in ɴsɪ's domain and the consumer's objects are not:
`transform` and `attributes` have no rdl2 counterpart, and one geometry
under two parents is *N* objects. So a journal has to be turned into
"these nodes may have changed" before it is useful, which is the
affected-set walk -- and coalescing the log into a set would be the
consumer's work rather than ours.

Net also settles order: create then delete in one interval nets to a
deletion; forty sets of one attribute net to one name.

**Rejected:** a version counter per node. It still needs tombstones for
deletes and removed edges, and it costs an O(N) scan per synchronise on
a scene whose edits per tick number one.

## D2: No values in the record

The scene holds the current value and `Node::effective` answers it. A
literal "journal of attribute sets" would copy every `P` array per
edit. An entry is `(handle, attribute)`.

## D3: Record what a mutator destroys, before it destroys it

Three mutators threw away exactly what a later walk needs:

- `delete` and `delete_recursive` drop the node and every edge naming
  it. Afterwards the type is gone and the orphaned children cannot be
  found from a handle nothing points at.
- `disconnect` matches inside a `retain` closure and discards the
  matches; a `.all` pattern cannot be re-expanded once its edges are
  gone.
- A repeated `connect` replaces an edge's **arguments in place**. ɴsɪ's
  `"priority"` rides on those arguments, so a scene can change which
  shader wins with no edge added and none removed. A record keyed on
  additions and removals misses it entirely.

## D4: The affected set inverts walks this crate already has

Resolution climbs `objects` to `.root` gathering containers; a
synchronise descends the same edges. The indexes that make the climb
cheap (`by_to_attr`) make the descent cheap, so no new index was
needed.

The rules, and what each inverts: a transform's attribute or an
`objects` edge inverts the chain walk; an `attributes` node or a shader
edge into one inverts the container gather; a shader's own attribute
propagates nowhere, because it maps one-to-one onto a renderer's
material parameters.

## D5: Match on `EdgeKind`, not on the destination attribute string

The first version matched `edge.kind.to_attr()` against string
literals with a `_` arm. `EdgeKind` is `#[non_exhaustive]`, but *inside
the crate* an exhaustive `match` is still checked, so a new variant is a
compile error at the place a decision is owed rather than a silent fall
into the wildcard. It also mis-bucketed: a `ShaderNetwork` whose port is
named `objects` took the child arm.

## D6: Candidates, not a minimal set

A re-armed `surfaceshader` under a nearer winner changes nothing after
re-resolution. Reporting it is over-approximation, which is allowed;
computing the minimum would mean remembering every previous answer,
which is the renderer's own change mask (`nsi-moonray` `research.md`
F1) and not ours to duplicate.

## D7: Roots, not an enumeration

Measured, one transform edit: naming the root costs 5.2 us at 50 000
nodes and 1.85 us at 200 000 -- constant -- where enumerating the same
answer costs 29 ms and 145 ms. A consumer walking its own objects
compares against the roots and never builds the list;
`Scene::descendants` expands one when it wants it.

**Rejected:** enumerating and letting the consumer ignore it. Moving
one transform near the top of a set-dressing scene names every geometry
under it, and production scenes have millions.

## D8: Borrowed, not owned

Every handle in the answer already lives in the scene or in the
`Changes` it came from. Owning them cost a `String` clone per named
node on a path a host walks every frame. `Affected<'a>` borrows; the
cost is a lifetime on the type.

## D9: Pending changes are outside scene equality

`Scene`'s `PartialEq` is written out rather than derived, excluding the
record and the three edge indexes. A scene that has just been
synchronised is still the same scene as the identical one that has not,
and the indexes are a function of the edges they index.

## What `synchronize` means, measured

The specification: "There should be no difference between scene
description and scene edits" (210-211); "synchronize -- for an
interactive render, apply all the **buffered** calls to scene's state"
(672-673); `NSIRenderSynchronized` is "an image which reflects all
changes" (694-697).

Driven against 3Delight, interactively, denoise off: edits without a
synchronise change nothing and report nothing; the synchronise reports
`Restarted` then `Synchronized` once for the whole batch; a delete
mid-render is legal. There is no observable "cannot apply
incrementally" -- 3Delight always restarts. A backend's "camera edit
means full reload" is a *backend* cost, not an ɴsɪ semantic.
