# Feature: Parsing A Stream In Parallel

## Problem

`parse_stream` reads one statement, applies it to the sink, and reads
the next: 149 MiB/s into a sink that does nothing, on one core of
sixteen. A renderer's context accepts calls from many threads, and a
scene is mostly independent statements -- thousands of `Create`s, each
node's `SetAttribute`s, each `Connect` -- so both the parsing and the
applying leave the machine idle.

ɴsɪ's own semantics say what may be reordered:

- A node must exist before anything refers to it, so every `Create`
  comes first.
- Once every node exists, `SetAttribute`s on different nodes and
  `Connect`s into different attributes do not interact.
- Two `SetAttribute`s on one node do: the later wins. Two `Connect`s
  into one attribute do: at equal priority the first wins. So do a
  `Connect` and a `Disconnect` of one edge, a `Delete` and anything
  touching the node, and a `RenderControl` and everything before it.

## User Stories

1. **As a renderer author** feeding a large `.nsi` file to a context
   that accepts concurrent calls, I call one function and the file is
   read and applied on every core.
2. **As a user of either entry point**, the scene that results is the
   one the sequential parser produces.
3. **As a caller with an interactive stream** -- edits, then a
   `RenderControl`, then more edits -- the edits after the
   `RenderControl` are not applied before it.

## Acceptance Criteria

- `parse_stream_parallel`, behind a `parallel` feature, with the same
  signature as `parse_stream` plus `N: Sync`.
- Statements are applied in phases:
  1. every `Create`, in parallel, in stream order per handle;
  2. every `SetAttribute` and `SetAttributeAtTime`, in parallel, in
     stream order per handle, and every `Connect`, in parallel, in
     stream order per destination attribute.
- `Delete`, `DeleteAttribute`, `Disconnect`, `Evaluate` and
  `RenderControl` are **barriers**: everything before one is applied
  before it, and it is applied alone, before anything after it.
- For every stream the sequential parser accepts, the recorded scene
  is identical to the sequential one's.
- Every error the sequential parser reports, the parallel one reports
  too, at the same offset, when it is the only error in the stream.
- A measured speed-up on the throughput corpus.

## Non-Goals

- **Replacing `parse_stream`.** The parallel version reorders calls
  and applies statements after the first failure; that is a different
  contract, chosen by calling a different function.
- **Parallel Lua.** A script is a program.
- **Parallel decompression.** gzip is sequential by construction.

## Risks

- **Reordering makes some invalid streams valid.** A `SetAttribute`
  on a node created later in the stream fails sequentially and
  succeeds here. Accepted: the scene that results is the one the
  stream describes.
- **Which statements are applied when one fails is unspecified.** The
  sequential parser stops at the failure; this one stops each group
  at its own. Documented.
- **Connection order into one attribute.** Settled by rendering
  before implementation: it matters (research D3), so it is kept.
- **Finding statement boundaries is itself sequential.** A string may
  hold anything, so the boundary scan has to lex. If it costs most of
  a parse, parallel application cannot pay for it.
