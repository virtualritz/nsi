# Feature: Filtering an ɴsɪ Stream

## Problem

A consumer wants to sit *between* an ɴsɪ front end and whatever
consumes the scene, and see every API call as it goes past: to drop
statements, rewrite them, count them, or split them across two back
ends. 3Delight offers exactly this in `nsicallbacks.h` -- a struct of
function pointers handed to `NSIBegin`, each returning `bool` to say
whether execution continues in the original context.

The reading half of that already exists here. `nsi_parse::parse_stream`
and `nsi_parse::run_lua` are generic over the [`Nsi`] trait, so a
consumer's own sink *is* the callback table, with one method per ɴsɪ
entry point and a `Result` where 3Delight has a `bool`: forward a call
to a downstream sink to continue, do not forward it to swallow it,
return `Err` to abort the stream.

The writing half does not. Nothing implements [`Nsi`] by emitting a
stream, so a filter has to terminate in a `Scene` and then call
`write_stream`. A `Scene` is state, not a call log: it coalesces forty
re-sets of one attribute into one, loses where a statement fell among
its neighbours, hoists `Evaluate` to the front, and holds the whole
scene in memory. That is right for "load it, edit it, save it" and
wrong for a filter.

## User Stories

1. **As a pipeline author**, I read a `.nsi` file, drop every
   `Connect` naming a light I do not want, and write a `.nsi` file --
   in constant memory, with every other statement byte-identical in
   meaning and in the order it arrived.
2. **As a tool author**, I read a Lua scene and write a plain `.nsi`
   stream, or the reverse, without going through a `Scene`.
3. **As a host author**, I put my filter in front of a live renderer:
   the same filter type forwards to `nsi_ffi_wrap::Context` instead of
   to a writer, because both are [`Nsi`].
4. **As a debugging author**, I tee a stream: one sink writes it to
   disk while another renders it.

## Acceptance Criteria

- A sink that writes `.nsi` and a sink that writes Lua, each
  implementing [`Nsi`], each emitting one statement per call, in call
  order, holding no scene.
- An identity filter over any stream this workspace can parse produces
  a stream that parses to an equal `Scene`.
- A filter can drop a call by not forwarding it, and abort by
  returning `Err`; neither needs support from the writers.
- The written stream is the syntax 3Delight writes, which is what
  `write_stream` already emits -- the two share their formatting.
- A worked filter example that a reader can copy.

## Non-Goals

- **Binary `.nsi` input.** `parse_stream` rejects it
  (`Error::BinaryStream`) and that does not change here.
- **A closure-based adapter.** A filter is a type implementing a
  nine-method trait; wrapping that in `Fn` fields buys a little syntax
  and costs a second way to do it. Revisit with a caller who wants it.
- **Round-tripping a stream byte for byte.** Comments, whitespace and
  the spelling of numbers are the parser's to normalise; the contract
  is on meaning, not bytes.
- **Reordering or deduplicating.** A filter that wants that can record
  into a `Scene`, which already does it.

## Risks

- A writer that disagrees with the parser about syntax makes a filter
  silently lossy. Mitigated by the round-trip gate, and by both
  writers sharing one argument formatter with `write_stream`, which
  3Delight's own output already gates.
- `RenderControl` carries its action as a parameter in the stream and
  as a typed `Action` in the trait. Emitting both, or neither, is a
  stream 3Delight will not read.
- A `Reference` parameter has no stream representation. Writing a
  header with no value would be malformed; 3Delight omits the whole
  parameter line, and so must a filter.
