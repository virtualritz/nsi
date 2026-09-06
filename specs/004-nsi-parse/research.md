# Research: ɴsɪ Parsing

## Decisions

### D1: The parser drives `Nsi`; it does not own a scene

`parse_stream` takes a sink implementing `nsi_trait::Nsi` and calls it.
A live 3Delight `Context`, an `nsi-intermediate` `Recorder`, and a
backend's own type are all valid sinks, so "read a scene" and "do
something with a scene" stay separate.

**Rejected:** producing `nsi_intermediate::Scene` directly. It would
force a dependency and a data copy on every consumer that wants
something else, and `Recorder` already provides exactly that path for
consumers who do want it.

**Consequence:** the parser must construct `Nsi::Arg`, which is a GAT.
Following `003`'s D1, `nsi_ffi_wrap::Arg` is the shared argument
currency, so the sink bound is
`for<'a> N: Nsi<Arg<'a> = nsi_ffi_wrap::Arg<'a, 'static>>` -- the same
bound the round-trip gates already use.

### D2: The grammar is token-based, not line-based

The ɴsɪ specification gives no grammar for the stream; it shows
examples. The format was therefore read off 3Delight, and the decisive
observation is that **an entire scene on one line parses**:

```text
Create "a" "transform" Create "b" "mesh" SetAttribute "b" "fov" "float" 1 45
```

`renderdl -cat` re-emits that as four correctly separated statements. So
the newlines and two-space indents 3Delight writes are formatting, not
syntax, and a line-oriented parser would be wrong on valid input.

Further observations from the same probes:

- Whitespace runs are free: `Create   "b"   "mesh"` parses.
- `#` begins a comment to end of line, at the start of a line or after a
  statement. Blank lines are skipped.
- A parameter list ends where the next **bare** token is a statement
  keyword. Parameter names are always quoted, so a bare word is
  unambiguous. This is what lets a statement occupy one line or twenty.

That last rule is the grammar. Everything else follows from it.

### D3: Lua is read by running it

ɴsɪ's Lua front end is Lua: a script may loop, branch, and compute the
scene it describes. The specification's own example builds shaders
programmatically. There is nothing to parse -- an interpreter is the
only correct reader.

**Rejected:** pattern-matching `nsi.Create(...)` calls out of the source.
It would appear to work on scripts this crate emits, which are trivially
regular, and fail on every script a person wrote.

**Consequence:** the `lua` feature embeds an interpreter and executes
the input. That is stated in the crate documentation rather than implied
by "supports Lua", because it is a different trust decision from parsing
a data file.

### D4: Correctness gates before throughput

The speed requirement is real, but a fast parser that is subtly wrong is
worse than a slow one. The round-trip gates land first; the throughput
number is measured afterwards and recorded, so a regression is a
comparison rather than an impression.

The optimisations the design allows for, in the order they are worth
doing: borrow unescaped strings from the input instead of copying;
reuse scratch buffers across statements so the steady state does not
allocate; scan with `memchr` rather than per-byte matching; parse
numbers directly from bytes.

### D5: `binarynsi` stays out, for the same reason as in `003`

ɴsɪ names three stream formats. The binary encoding is not documented,
so supporting it means reverse-engineering the renderer's bytes. `003`
R28 already records that decision for the writer; a reader that accepted
what the writer cannot produce would be a strange asymmetry.

## Answered Questions

### Q1: `NSI_PATH_` replacement -- settled, and the parser's job is to
do nothing

`streampathreplacement` is documented: "replacement of path prefixes by
references to environment variables which begin by `NSI_PATH_` in an nsi
stream ... to ease creation of files which can be moved between
systems". An earlier probe failed to produce a replacement, so this
stayed open on the assumption that the *parser* would have to expand.

Rendered, the assumption was backwards. The syntax is `${NAME}`, and
3Delight expands it **at use time**, in whatever consumes the value:

- `renderdl -cat` echoes `${NSI_PATH_TEST}/out.exr` back unexpanded, so
  the stream reader is not where expansion happens.
- A render whose `imagefilename` is `${NSI_PATH_TEST}/pathout.exr`, with
  that variable set, writes to the expanded path and creates no literal
  `${NSI_PATH_TEST}` directory.

So carrying the string verbatim is correct, and expanding in the parser
would corrupt a stream on re-emission -- baking one machine's paths into
a file whose whole purpose is to move between machines.

**The rule is wider than the name.** Any `${VAR}` expands, not only
`NSI_PATH_`-prefixed ones: `${HOME}` and `${OTHER_VAR}` both resolved in
the same probe. The `NSI_PATH_` prefix governs which variables 3Delight
*writes* as references, a write-side convention under
`streampathreplacement`. On read, nothing is special about the prefix.

The obligation therefore lands on the **consumer**: a backend that takes
a path value and opens it without expanding `${VAR}` opens the wrong
file, and will do so only on the machines where the variable was meant
to matter -- the failure appears when the scene is moved, which is
exactly when it is hardest to diagnose.

## References

- 3Delight 2.9.207 `nsi.pdf`, and `renderdl -cat` / `-lua -cat` as the
  oracle for what is not written down.
- `specs/003-nsi-intermediate-representation` -- the writer these gates
  round-trip against, and the source of the type spellings, flag
  prefixes and float formatting this parser must accept.
