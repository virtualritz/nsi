# Plan: Parsing A Stream In Parallel

## Approach

`parse_stream_parallel`, behind a `parallel` feature, in
`crates/nsi-parse/src/parallel.rs`:

1. **Scan**, sequentially: find every statement's keyword offset, its
   phase, and a hash of what it touches. Skips strings with `memchr2`
   and rejects most bare words on their first byte; builds no tokens.
2. **Segment** at barriers: `Delete`, `DeleteAttribute`, `Disconnect`,
   `Evaluate`, `RenderControl`.
3. Per segment, **two phases** on rayon: every `Create`; then every
   `SetAttribute`, `SetAttributeAtTime` and `Connect`. Within a phase,
   statements are stably sorted by key and grouped; groups run in
   parallel, each in stream order. Each statement is parsed by the
   sequential parser's own `apply`, from its offset.
4. The barrier, alone.

Errors: every group stops at its first; the earliest by offset is
returned. A stream not starting with a keyword goes to the sequential
parser, which fails before touching the sink.

## Gates

1. **Same scene**: `tests/parallel.rs` records both parsers' scenes and
   compares them in a canonical form that forgives exactly the
   reordering allowed -- node and attribute order, edges across
   destination attributes -- and keeps the rest.
2. **Same error**, for single-error streams: leading junk, junk between
   statements, a bad value, an unterminated string, a missing action, a
   sink refusal.
3. **Barriers hold**, observed on a call log, since a scene cannot show
   when a `RenderControl` arrived.
4. **Falsified**: each ordering rule is broken on purpose and a gate
   reddens.
5. **Measured**: `tests/throughput.rs`, into a null sink and into
   3Delight.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/parallel-parse.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`
