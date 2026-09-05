# Specs

Feature specs live in this directory. The active feature directory is stored
in `.specify/feature.json`.

## Coverage Order

Surfaces on the path from "interactive" (the API's design target) to
"realtime" (frame-budget) use, in intended coverage order:

1. Pixel streaming/output contract -- how rendered pixels reach a client with
   GPU residency (`001-gpu-pixel-streaming`).
2. Input-side data residency -- zero-copy/GPU-handle geometry and attribute
   handoff via `Reference`-style arguments.
3. Frame pacing and temporal semantics -- publication deadlines, edit-class
   fast paths across `synchronize`.
4. Shading profile -- the fixed closure/node vocabulary for network
   translation to SPIR-V (`002-shading-profile`; direction decided in
   `001-gpu-pixel-streaming/research.md` D7; arbitrary ᴏsʟ stays offline).
5. Realtime backend -- an `Nsi`-trait renderer implementation consuming
   1--4.

Directory numbers reflect creation order, not coverage order.

## Surfaces

| # | Surface | Status |
| --- | --- | --- |
| `001-gpu-pixel-streaming` | Pixel streaming/output contract | active |
| `002-shading-profile` | Shading profile for network translation | -- |
| `003-nsi-intermediate-representation` | The `nsi-intermediate` crate: the renderer-agnostic IR between ɴsɪ and any back end | implemented; `Partial` and `Open` rows remain |

`003` arrived with the crate, which was extracted from `nsi-mitsuba`
once a second backend made its renderer-agnosticism structural rather
than notional. It is the reference implementation of the `Nsi` trait
declared in `nsi-trait`, which is why it belongs beside it, and it is
consumed by [`nsi-mitsuba`](https://github.com/virtualritz/nsi-mitsuba)
and [`nsi-moonray`](https://github.com/virtualritz/nsi-moonray).

Coverage-order item 5 above -- an `Nsi`-trait renderer implementation --
is what those backend repositories are. `003` is the half of that work
which is common to all of them.

## Definition Of Covered

A surface is covered only when all of these are present:

- A feature spec exists in `specs/<feature>/spec.md`.
- Contracts exist in `specs/<feature>/contracts/`.
- Implementation tasks exist in `specs/<feature>/tasks.md`.
- Every contract matrix row is `Covered`, `Partial`, or `Open`.
- Each `Covered` row cites source evidence and either executable test
  evidence or explicit manual QA evidence.
- Each contract file contains a `Required Evidence Before Marking Complete`
  section.

Docs, checkboxes, and TODOs are not evidence by themselves.

## Active Spec

Current active feature:
`specs/001-gpu-pixel-streaming/spec.md`.
