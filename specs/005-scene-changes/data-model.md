# Data Model: What Changed

## `Changes`

Recorded by every mutator on `Scene`, cleared by `Scene::take_changes`.
`#[non_exhaustive]`, public fields.

| Field | Type | Notes |
| --- | --- | --- |
| `created` | `IndexSet<String>` | created and not since deleted |
| `deleted` | `IndexMap<String, String>` | handle to the node type it had; the type is kept because the handle is gone |
| `attributes` | `IndexSet<(String, String)>` | `(handle, attribute)`; names only, never values |
| `edges_added` | `Vec<Edge>` | at most one entry per `(from, to, kind)` |
| `edges_removed` | `Vec<Edge>` | in full: a `.all` disconnect is expanded here, since the pattern cannot be re-expanded once the edges are gone |
| `edges_rearmed` | `Vec<Edge>` | a repeated `connect` replaced the arguments in place; no edge appeared or disappeared |

Net, not a log: one entry per fact. The three edge lists are keyed on
`(from, to, kind)` -- an edge's identity to ɴsɪ -- so a host that
re-connects one edge every frame records one entry rather than a full
`Edge` per call.

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
