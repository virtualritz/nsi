# Data Model: Weld Declarations

```text
weld node  ──weld──▶  geometry (nurbs | mesh)
                        weld.id            int        per use
                        weld.segment-count int        per use   (default 1 each)
                        weld.kind          string     per segment
                        weld.index         int[3]     per segment
                        weld.reverse       int        per segment (default 0)
                        weld.range         float[2]   per segment (default [0 1])
```

| Type | Fields |
| --- | --- |
| `WeldUse<'a>` | `geometry`, `weld` (node handle), `id`, `segments: Vec<WeldSegment>` |
| `WeldSegment` | `kind: WeldKind`, `reverse: bool`, `range: [f32; 2]` |
| `WeldKind` | `TrimLoop { loop }`, `TrimCurve { curve }`, `NurbsSide { side }`, `MeshEdge { face, loop, edge }` |
| `Weld<'a>` | `weld`, `id`, `uses: Vec<WeldUse<'a>>` -- one joined boundary |
| `WeldProblem<'a>` | `geometry`, `use_index`, `kind: WeldProblemKind`; `Display` is the diagnostic |

A `Weld` with one use is an *open* declaration; with more than two, a
*non-manifold* join. Both are reported by `welds()` as they are and
flagged, not refused.
