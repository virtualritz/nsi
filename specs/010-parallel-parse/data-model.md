# Data Model: Parsing A Stream In Parallel

## `Statement`, one per keyword

| Field | Holds |
| --- | --- |
| `phase` | `Create`, `Modify` or `Barrier` |
| `start` | the keyword's byte offset; the statement ends at the next one's |
| `key` | hash of what it touches |

## Keys

| Statement | Phase | Key operands |
| --- | --- | --- |
| `Create` | Create | handle |
| `SetAttribute`, `SetAttributeAtTime` | Modify | handle |
| `Connect` | Modify | destination handle, destination attribute |
| `Delete`, `DeleteAttribute`, `Disconnect`, `Evaluate`, `RenderControl` | Barrier | -- |

An escaped operand is keyed by its decoded bytes, so two spellings of
one handle share a group.

## Schedule

```text
[ Create* | Modify* ]  Barrier  [ Create* | Modify* ]  Barrier  ...
   par      par        alone       par      par
```
