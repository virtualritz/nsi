# Data Model: What Changed

## `Changes`

Recorded by every mutator on `Scene`, cleared by `Scene::take_changes`.
`#[non_exhaustive]`.

| Field | Type | Read with | Notes |
| --- | --- | --- | --- |
| `created` | `IndexSet<Handle>` | `created()`, `was_created()` | created and not since deleted |
| `deleted` | `IndexMap<Handle, Handle>` | `deleted()`, `deleted_type()` | handle to the node type it had; the type is kept because the handle is gone |
| `attributes` | `IndexSet<(Handle, Handle)>` | `attributes()` | `(handle, attribute)`; names only, never values |
| `edges_added` | `Vec<Edge>` | public field | at most one entry per `(from, to, kind)` |
| `edges_removed` | `Vec<Edge>` | public field | in full: a `.all` disconnect is expanded here, since the pattern cannot be re-expanded once the edges are gone |
| `edges_rearmed` | `Vec<Edge>` | public field | a repeated `connect` replaced the arguments in place; no edge appeared or disappeared |

Net, not a log: one entry per fact. The three edge lists are keyed on
`(from, to, kind)` -- an edge's identity to ɴsɪ -- so a host that
re-connects one edge every frame records one entry rather than a full
`Edge` per call.

The three handle-keyed fields are private behind `&str` accessors, for
the reason [`Node`](../003-nsi-intermediate-representation/data-model.md)'s
are: a `Handle` is a `String`, or an interned `ustr::Ustr` under the
`ustr_handles` feature, and no consumer should be able to tell. The
journal is the one structure written on every edit of every frame, so
it is also where interning pays most. Measured, debug build, 20 000
attribute sets on a 100 000-node scene: 6.0 allocations per edit and
128.5 retained bytes per edit before, **3.0 and 49.2** after -- the
three that remain are the caller's own `OwnedArgument`. Without the
feature the numbers are unchanged, since a `Handle` is a `String`
there and recording one still copies it. No wall-clock difference was
measurable in a debug build; this is an allocation and footprint
change, not a speed one.

## `Affected<'a>`

Returned by `Scene::affected`. `#[non_exhaustive]`, public fields,
borrows from the `Scene` and the `Changes` it was computed from.

| Field | Type | Notes |
| --- | --- | --- |
| `roots` | `IndexSet<&'a str>` | these nodes **and everything below them** on the `objects` chain |
| `shaders` | `IndexSet<&'a str>` | shader nodes only: material parameters, no geometry work |
| `outputs` | `bool` | the camera/screen/layer/driver chain changed |
| `everything` | `bool` | an attribute on `.root` or `.global`; `roots` is then empty rather than a copy of the scene |

`Scene::descendants(root)` expands a root when a caller wants the list.

## Ownership

`Scene::changes` is private and does not appear in `Scene`'s equality
(see `research.md` D9). Everything else here borrows.

## Wire format

None. The record is in-memory only: `write_stream` and `write_lua`
replay a scene, not a diff. A `.nsi` stream *is* a series of edits by
ɴsɪ's own definition, so a stream replayed into a live context is the
wire format, and it needs nothing added here.

## Migration

Additive to `Scene` apart from `PartialEq` becoming hand-written and
`Affected` gaining a lifetime. Nothing is published, so no consumer
breaks.
