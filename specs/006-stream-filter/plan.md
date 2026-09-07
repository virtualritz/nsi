# Plan: Filtering an ɴsɪ Stream

## Approach

Add the missing half. The reading half is `nsi-parse`, generic over
[`Nsi`]; the writing half is two sinks in `nsi-intermediate`,
`StreamWriter` and `LuaWriter`, that emit one statement per call. A
filter is then a user type between them, and needs nothing from this
workspace but the trait.

Both writers convert each `Arg` with `OwnedArgument::from_param` and
format it with the `write_arg` that `write_stream` uses, so the two
emitters cannot drift. Everything else is statement spelling, which is
one `write!` per method.

## Gates

1. **Round-trip through the filter.** Parse a fixture, pass it through
   an identity filter into `StreamWriter`, parse the result, compare
   the two `Scene`s. Reddens if any statement is dropped or mis-spelt.
2. **Call order survives.** A fixture that sets one attribute twice
   and re-connects one edge comes out with both calls, in order --
   which recording into a `Scene` would coalesce. This is the property
   that distinguishes a filter from `write_stream`.
3. **A dropped call is dropped.** A filter that swallows `Connect`
   yields a stream with no `Connect`, and the rest intact.
4. **Lua and stream agree.** The same calls through `LuaWriter` parse
   back, with `run_lua`, to the same `Scene` as through
   `StreamWriter`.
5. `cargo clippy --all-targets -- -D warnings`, both feature
   configurations, plus rustdoc and `fmt`.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/filter.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`

## Out Of Scope

Binary `.nsi` input, a closure-based filter adapter, byte-identical
round-tripping, and any reordering -- see `spec.md`.
