# Tasks: Filtering an ɴsɪ Stream

## Story 1 -- A `.nsi` stream in, a `.nsi` stream out

- [x] T1 `StreamWriter<W>` implementing [`Nsi`], one statement per
      call, sharing `write_arg` with `write_stream`.
      *Evidence:* `nsi-parse` `tests/filter.rs`, every statement of
      the fixture, plus the `StreamWriter` doctest.
- [x] T2 `RenderControl` writes its action from the typed [`Action`]
      and drops an `action` parameter that arrived in the argument
      list. *Evidence:* a test that round-trips `RenderControl` and one
      that hands it a redundant `action`.
- [x] T3 Identity filter round-trip: fixture -> parse -> writer ->
      parse -> equal `Scene`.
      *Evidence:* `tests/filter.rs::an_identity_filter_changes_nothing`,
      and `tests/renderdl.rs::a_filtered_stream_is_one_3delight_reads_back`
      for the same over 3Delight's own output.
- [x] T4 Call order and repeated calls survive.
      *Evidence:* `tests/filter.rs::repeated_calls_survive_in_order`.

## Story 2 -- Lua in, `.nsi` out, and back

- [x] T5 `LuaWriter<W>` implementing [`Nsi`].
      *Evidence:* the `LuaWriter` doctest, and
      `tests/filter.rs::the_lua_writer_refuses_a_create_with_parameters`
      for what it will not write.
- [x] T6 Lua and stream writers agree on the same call sequence.
      *Evidence:*
      `tests/filter.rs::the_lua_writer_agrees_with_the_stream_writer`.

## Story 3 -- The filter itself

- [x] T7 A worked example: read a `.nsi` file, drop a class of
      statement, write a `.nsi` file.
      *Evidence:* `crates/nsi-parse/examples/filter.rs`, run by hand
      per `quickstart.md` and built by `--all-targets`.
- [x] T8 A dropped call is absent from the output and nothing else is.
      *Evidence:*
      `tests/filter.rs::a_swallowed_call_is_absent_from_the_output`.

## Open

- [ ] T9 Whether `Delete`'s `recursive` parameter should be written
      from the argument list or synthesised. The recorder reads it and
      the writer copies it through, so a stream that arrived with it
      keeps it; a filter that *calls* `delete` on a recursive delete
      cannot say so. Needs a caller.
