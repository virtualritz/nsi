# Data Model: ɴsɪ Parsing

## Entities

### The sink

Not a type in this crate. `parse_stream` and `run_lua` take
`&N where for<'a> N: Nsi<Arg<'a> = nsi_ffi_wrap::Arg<'a, 'static>>` and
call it. See `research.md` D1.

### `Token`

Internal. The lexer's output over `&[u8]`:

| Variant | Notes |
| --- | --- |
| `Word(&str)` | A bare token: a statement keyword, or a number in operand position. Numbers are parsed on demand by the type the parameter declares. |
| `Quoted(Quoted<'_>)` | A string literal, `Borrowed` when it carried no escape and `Owned` when it did. |
| `Open` / `Close` | Array delimiters. |

Not public: the token stream is an implementation detail, and exposing
it would freeze the lexer.

### `Statement`

Internal. A keyword and its operands, followed by a parameter list for
the three that take one. The parser applies each statement to the sink
as it completes, so nothing accumulates.

### `Scratch<'a>`

The reused buffers: one `Vec` per storage representation (`f32`, `f64`,
`i32`, `i64`, `Cow<'a, str>`), plus folded buffers for the tuple types
whose argument constructors take `&[[f32; 3]]` and `&[[f32; 16]]`, and
the parameter descriptors. Cleared, never freed, between statements.

A statement's `Arg`s are *not* in here -- they borrow it, so they live on
the stack in a `SmallVec` instead. That, and borrowing names and string
values from the input, is what R6 is about: without them the parser
allocated three times per node.

### `ParseError`

| Field | Notes |
| --- | --- |
| `offset` | Byte offset of the offending **token**, not of the trivia before it. |
| `expected` | What the parser wanted there. |

Plus variants for the lexer's own failures, for a compressor that would
not decompress, for a Lua script that failed, and for the sink's own
error -- applying a statement can fail (`RecordError::UnknownHandle`,
for instance). Generic over the sink's `Error`, so a caller keeps its
type.

## Wire Format

The `.nsi` stream, as established in `research.md` D2. Type spellings,
flag prefixes, the `type[n]` array form and `%.17g` doubles are the ones
`specs/003` recorded from the same renderer; the parser accepts them and
the round-trip gate is what keeps the two in step.

## Migrations

None. This crate reads a format it does not define. Compatibility is
whatever `specs/003` records for the writer, and the gates fail if the
two drift.
