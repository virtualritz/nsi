# Contract: Stream Grammar

## Scope

Covers turning `.nsi` bytes into ɴsɪ calls. Does not cover Lua
(`lua.md`) or what a sink does with the calls.

## Why This Contract Exists

The ɴsɪ specification gives no grammar for the stream, only examples.
Every rule here is an observation of 3Delight 2.9.207, and an
unobserved construct is an assumption. So each row names the capture
that established it, and anything unestablished is `Open` rather than
guessed.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A statement's parameter list ends at the next keyword | Covered | `parse.rs` `parameters` returns on the first non-quoted token | `roundtrip::a_scene_on_one_line_parses`, an entire scene on one line -- the case a line-oriented reader gets wrong and 3Delight accepts | -- |
| Whitespace runs are insignificant | Covered | `lex.rs` `skip_trivia` | `roundtrip::comments_and_spacing_are_ignored` | -- |
| `#` comments run to end of line | Covered | `lex.rs` `skip_trivia`, `memchr` to the newline | `roundtrip::comments_and_spacing_are_ignored`, with a leading, a blank and a trailing case | -- |
| Every statement 3Delight writes is parsed | Partial | `parse.rs` `apply` dispatches all nine | `roundtrip::a_written_stream_parses_back_into_the_same_scene` and `renderdl::what_3delight_writes_parses_back_into_the_same_scene` cover `Create`, `SetAttribute`, `SetAttributeAtTime` and `Connect`; `roundtrip::an_evaluate_statement_round_trips` covers `Evaluate`, which the writer has emitted since `nsi-intermediate` recorded it | `Delete`, `DeleteAttribute`, `Disconnect` and `RenderControl` are dispatched but not driven by either gate: the writer emits none of them, by the R10 preconditions of `specs/003`. |
| Every argument type is parsed | Covered | `value.rs` `Base::parse`, `parse_type` | `roundtrip::a_written_stream_parses_back_into_the_same_scene` drives every type, both flag kinds and an `array_len`; `renderdl` drives the renderer's own spellings. Dropping a flag or an array length fails it | -- |
| An unescaped string is borrowed, not copied | Partial | `lex.rs` `Quoted::Borrowed`, only copying when an escape is present | Implemented and exercised by every test, but nothing asserts the borrow | A test asserting the parsed `str` points into the input buffer. |
| Escapes are decoded | Covered | `lex.rs` `unescape` | `roundtrip::octal_escapes_are_decoded`, `a_short_octal_escape_is_decoded` -- named here as `..._is_an_error` until a reviewer looked it up, which had the row asserting the opposite of the test. Measured from the renderer: `\"`, `\\`, `\t` and `\n` by name, every other byte below `0x20` as **three-digit octal**, and every byte at or above `0x7f` **raw**. There is no `\xHH`; decoding that instead rejected `\001` outright, so any stream carrying a tab or carriage return in a string was unreadable | -- |
| A malformed stream fails with an offset | Covered | `lib.rs` `Error::Syntax`, carrying the byte offset | `roundtrip::a_malformed_stream_reports_an_offset`, which also asserts the sink kept the statements before it; `an_unknown_statement_is_rejected`; `a_sink_refusal_is_reported` | -- |
| `NSI_PATH_` references are **not** expanded here | Covered | `lex.rs` carries the string verbatim | The row assumed the parser must expand. Rendered, it must not: 3Delight expands at *use* time, in whatever consumes the value, and `renderdl -cat` echoes `${NSI_PATH_TEST}/out.exr` back unexpanded. A render naming that path writes to the expanded location and creates no literal directory. Carrying it verbatim is therefore right, and expanding here would corrupt a stream on re-emission | -- |
| The expansion rule is recorded for consumers | Covered | `nsi-intermediate` `owned/mod.rs` `OwnedData::String`, where a backend reads a path value; `research.md` Q1 | Measured, and wider than the row's name suggests: **any** `${VAR}` expands, not only `NSI_PATH_`-prefixed ones -- `${HOME}` and `${OTHER_VAR}` both resolved. The `NSI_PATH_` prefix governs which variables 3Delight *writes* as references under `streampathreplacement`, a write-side convention, not which it reads. A backend that uses a path value without expanding opens the wrong file. Measured in detail, because each rule changes what a backend implements: `${NAME}` only (a bare `$NAME` stays literal); path-valued attributes only (`imagefilename`, `shaderfilename`, `Evaluate`'s `filename`, nesting -- but **not** `drivername`, a handle, or an attribute name); an unset variable stays literal rather than expanding to empty; and `${...}` in an output layer's `variablename` **segfaults** 3Delight 2.9.207 | -- |

| The declared element count is enforced | Covered | `value.rs` `read` compares the values against `count * array_len` | `roundtrip::a_count_that_disagrees_with_its_values_is_an_error`. The count is authoritative: given `"P" "point" 1 [ 0 0 0 1 2 3 ]` the renderer warns and keeps one point, where ignoring it yielded two |
| An error's offset names the offending token | Covered | `lex.rs` records `token_start` after trivia | `roundtrip::an_error_offset_points_at_the_token`. It previously pointed at the whitespace before the token, or past the end of it |
| A raw byte at or above `0x7f` | Covered | `lex.rs` `Quoted` carries bytes | See the **Byte Fidelity** section below, which supersedes this row. It was left `Open` by the commit that fixed it, so the contract contradicted both the code and its own new section for two commits. Values carry the byte; identifiers still refuse it, and that limit is `nsi_trait::Nsi`'s `&str`, not the format's | -- |

| Compressed input is detected, not declared | Covered | `lib.rs` `parse_compressed` sniffs the leading bytes | `compressed::gzip_is_detected_and_read`, `zstd_is_detected_and_read`, `uncompressed_input_is_passed_through`, `a_truncated_compressed_stream_is_an_error`. R10 had no row at all |

| The two crates round-trip each other | Covered | `nsi-intermediate`'s writer and this parser | `cross_crate::what_one_crate_writes_the_other_reads` and `the_round_trip_is_idempotent`, over every type, all three flags, `array_len(1)` and `(n)`, empty slices, octal-escaped control characters, `.global`, motion samples, connection arguments and all seventeen connection classes. The test's own documentation records the two defect classes it *cannot* catch, both confirmed by breaking them and watching it stay green |

## Invariants

- The parser calls the sink; it does not accumulate a scene.
- A statement is applied when it completes, so a failure leaves the sink
  holding everything before it. R5 makes that visible rather than
  pretending otherwise.
- The grammar accepts at least everything `nsi-intermediate` writes and
  everything 3Delight writes. Those are the two gates.

## Required Evidence Before Marking Complete

- A round-trip gate: parse a stream into a `Recorder`, write it back,
  compare.
- A 3Delight gate: parse what the renderer wrote, re-emit, compare.
- A malformed-input case per error variant.

## Byte Fidelity

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A non-UTF-8 string *value* parses | Covered | `lex.rs` `Quoted` holds `&[u8]` / `Vec<u8>`; `unescape` returns bytes | `roundtrip::a_non_utf8_string_value_survives_parsing`, `a_non_utf8_byte_survives_an_escaped_value`; re-adding the UTF-8 check reddens the first. The parser used to **refuse** such a stream outright, so it could not read what 3Delight writes -- `renderdl -cat` echoes a raw `0xE9` in a file name back unchanged | -- |
| A non-UTF-8 *identifier* is refused | Covered | `lex.rs` `Quoted::into_ident`, called where a handle, node type, parameter name or type spelling is read | `roundtrip::a_non_utf8_identifier_is_refused_in_every_position`, covering all four; making the conversion lossy reddens it. The first version of that test covered the handle alone, and a lossy parameter name reddened only the *allocation* test | -- |
| The refusal's stated reason is the real one | Covered | `lex.rs` `into_ident` doc, `parse.rs` `string` doc | **Not** "because ɴsɪ compares them", which the renderer contradicts: `renderdl -cat` accepts and echoes a Latin-1 handle and parameter name, comparing bytes. The constraint is this crate's -- [`nsi_trait::Nsi`] takes `&str` for handles and names, so the parser has nothing to carry them in. Refusing beats inventing a different name, but it is a limit, not a rule of the format | Carrying identifiers as bytes would mean changing `nsi_trait::Nsi`'s signatures. Until then a stream 3Delight wrote with a Latin-1 *handle* is unreadable here -- the same failure class this section fixes for values. |
| The offset names the failing token | Covered | `parse.rs` `string` reads `lexer.offset()` **after** `next_token` | Same test asserts all four offsets. `Lexer::offset` reports the start of the token last returned, so reading it first named the *previous* operand: a bad handle reported `0` | -- |
