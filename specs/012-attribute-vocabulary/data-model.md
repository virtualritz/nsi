# Data Model

## `names::Rename`

A row of the draft's mapping.

| Field | Meaning |
| ----- | ------- |
| `node_type` | The legacy node type the row belongs to; empty for the draft's *Common (All Nodes)* section. |
| `legacy` | The shipped spelling, as 3Delight reads it. |
| `draft` | The draft's spelling, as a `Scene` stores it. |

## Lookups

- `draft_name(node_type, legacy) -> Option<&'static str>`: the draft
  spelling of a shipped name on that node, `None` when the name is not
  in the mapping.
- `legacy_name(node_type, draft) -> Option<&'static str>`: the reverse,
  for writers.
- `draft_node_type(legacy)` and `legacy_node_type(draft)`: the same for
  the node types the draft names.

## State

One `Mutex<HashSet<&'static str>>` of the legacy names already warned
about, so each prints once per process.
