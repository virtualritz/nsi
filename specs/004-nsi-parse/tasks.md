# Tasks: ɴsɪ Parsing

Ordered by `plan.md`: correctness before speed. That order held -- the
gates were green before anything was measured.

## User Story 1: Read A Stream Into Anything (P1)

- [x] T1.1 Lexer over `&[u8]`: bare words, quoted strings with escapes,
      numbers, brackets, `#` comments. Borrowing.
- [x] T1.2 The keyword-terminated grammar. First test is the whole
      scene on one line, because that is the case a line-based parser
      gets wrong and 3Delight accepts.
- [x] T1.3 The nine statements. Five are dispatched but undriven by
      either gate; see `contracts/grammar.md`.
- [x] T1.4 The argument types, including `type[n]` and the `v`/`f`/`l`
      flag prefixes.
- [x] T1.5 Apply to the sink through `nsi_ffi_wrap::Arg`.
- [x] T1.6 `Error` with a byte offset, and the sink's own error
      carried through.
- [x] T1.7 Round-trip gate against `nsi-intermediate`'s writer.
- [x] T1.8 **Gate against 3Delight's own output.** Grouped attributes
      and wrapped continuation lines, which our own writer never emits.

## User Story 2: Read It Fast (P1)

- [x] T2.1 Scratch buffers reused across statements.
- [x] T2.2 Borrow unescaped strings from the input.
- [x] T2.3 Counting-allocator test. It measured 3 allocations per
      node, so R6 was false as written; borrowing parameter names and
      stacking the argument list took it to a flat 5 for any scene.
- [x] T2.4 A corpus generator and a recorded figure: 175--183 MiB/s,
      release, x86-64 Linux. See `contracts/performance.md`.

## User Story 3: Read A Lua Scene (P2)

- [x] T3.1 Embed an interpreter behind the `lua` feature; document that
      reading a script runs it.
- [x] T3.2 The `nsi` table over the sink.
- [x] T3.3 Parameter tables, the `nsi.Type*` constants, `arraylength`,
      and ɴsɪ's untyped defaults.
- [x] T3.4 Round-trip `write_lua` output.
- [x] T3.5 A computed scene, which no pattern-matcher could read.
- [x] T3.6 Script errors surface as errors, and a sink refusal is
      carried out rather than stringified into one.

## User Story 4: Compressed Input (P2)

- [x] T4.1 `gzip` and `zstd` inputs, detected from the leading bytes
      rather than declared by the caller.

## Open Questions

- [ ] T5.1 `NSI_PATH_` replacement: establish the trigger and the
      reference syntax, then expand or document. `research.md` Q1.

## Found By Review, Round 6

- [x] T6.1 Octal escapes. The lexer decoded `\xHH`, which the renderer
      never writes, and rejected the three-digit octal it does -- so any
      stream with a tab or carriage return in a string was unreadable.
      `nsi-intermediate`'s writer was not escaping control bytes at all.
      Evidence: `roundtrip::octal_escapes_are_decoded`,
      `a_short_octal_escape_is_an_error`, and a control character in the
      3Delight stream gate.
- [x] T6.2 The declared element count is enforced; the renderer
      validates it. Evidence:
      `roundtrip::a_count_that_disagrees_with_its_values_is_an_error`.
- [x] T6.3 Error offsets name the token, not the trivia before it.
      Evidence: `roundtrip::an_error_offset_points_at_the_token`.
- [x] T6.4 Lua: a sink refusal stops the script; `RenderControl` is
      bound; `Delete` carries `recursive`; short tuple data is refused;
      the type constants are exactly the renderer's. Evidence:
      `lua::a_refusal_stops_the_script`, `render_control_is_bound`,
      `delete_carries_its_parameters`, `short_tuple_data_is_refused`.
- [x] T6.5 `RenderControl` no longer passes `action` twice -- ɴsɪ's own
      `render_control` appends it.
- [x] T6.6 The cross-crate round-trip, which nothing covered.
