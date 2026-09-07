# Research: Filtering an ɴsɪ Stream

## D1 -- The writers live in `nsi-intermediate`, not `nsi-parse`

`nsi-intermediate` already emits both formats (`stream::write_stream`,
`lua::write_lua`) and owns the argument formatting those need:
`quoted_str`, `format_f32`/`format_f64`, `type_name`, `element_count`,
`scalar_count`. A streaming writer emits the same statements one at a
time, so putting it beside them shares that code rather than growing a
second speller.

*Rejected:* putting them in `nsi-parse`, which would be tidy for a
reader who thinks of "filter" as a parsing concept and would duplicate
every formatting rule -- including the ones a render settled, like a
lone scalar being bare and `[ ]` for an empty one.

## D2 -- Arguments are converted, not formatted generically

A writer sink receives `nsi_ffi_wrap::Arg`, and `write_arg` formats an
`OwnedArgument`. `OwnedArgument::from_param` already converts one, with
the count rule ɴsɪ applies (`count = len / array_length`, a partial run
dropped) that this crate got wrong once.

*Rejected:* making the formatter generic over `ParamValue` and reading
the raw pointer at each type. That is `from_param`'s dispatch written a
second time, next to the one that is tested, and it saves one memcpy
per argument on a path that then formats every scalar as decimal text
-- which dominates by an order of magnitude.

## D3 -- One statement per call, in call order, with no memory

[`Nsi`] takes `&self`, so the writer holds its output in a `Mutex`.
Nothing else is retained: the writer never sees a second statement's
data while writing the first, which is what makes a filter over a
100 GB stream cost the size of one statement.

## D4 -- `Create` writes its parameters

ɴsɪ's `NSICreate` takes optional parameters and 3Delight writes them.
`Scene` drops them (a create's arguments are not scene state, and the
`Recorder` ignores them); a filter must not, or a stream loses data on
the way through.

## D5 -- `RenderControl`'s action is written from the typed `Action`

The stream carries `"action"` as a parameter; the trait carries it as
[`Action`]. `nsi-parse` strips the parameter and passes the enum, and
`nsi-ffi-wrap`'s `Context` appends it again on the way out. The writer
does the same, and drops any `action` parameter that survived in the
argument list -- so a stream that arrived with one gets exactly one on
the way out, and it is the one the sink was actually called with.

## D6 -- `Reference` parameters are dropped, with their line

A host pointer has no stream representation. 3Delight omits the whole
parameter line and keeps the statement; `write_arg` already does this,
so the streaming writers inherit it. A filter is therefore lossy for
`Reference` arguments by construction, which is a property of the
format rather than of this code.

## D7 -- `io::Error` is the writers' error type

[`Nsi::Error`] must be `Error + Send + Sync + 'static`, which
`io::Error` is. A writer has no other failure: it does not validate,
because the sink downstream of a filter may be a renderer that will,
and rejecting here would make a filter stricter than the renderer it
feeds.

## D8 -- The Lua writer emits statements, not a scene

`write_lua` walks a `Scene` and emits `nsi.Create` in node order. The
streaming twin emits whatever call it is handed, so a Lua file written
by a filter has the *stream's* order, which is the order the renderer
would have seen. The two therefore disagree on output for the same
scene, and that is the point.

## D9 -- Rejected: a `Filter` adapter with closure fields

A filter is a type implementing nine methods and forwarding the ones it
does not care about. A `Filter { on_create: Box<dyn Fn ...>, .. }`
would spare a caller that boilerplate, at the cost of a second way to
write a filter, `Fn` bounds fighting `&self`-plus-`Send + Sync`, and
nine `Option<Box<dyn Fn>>` indirections on a hot path. The example
shows the trait version, which is what 3Delight's own struct of
function pointers is anyway.

## References

- `$DELIGHT/include/nsicallbacks.h` -- the C shape this mirrors:
  "callbacks provide a way to trap NSI calls in a given NSI stream",
  each returning `true` to continue in the original context.
- `$DELIGHT/include/nsi.hpp`, `PointerArg::FillNSIParam` -- why a
  `Reference` has no text form.
- `specs/004-nsi-parse` -- the reading half, and the `Arg` type every
  sink in this workspace speaks.
