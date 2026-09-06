# Feature Spec: ɴsɪ Parsing

## Context

`nsi-intermediate` writes ɴsɪ out. This reads it back in. Together they
close the loop: a scene can be captured, inspected, replayed, and
round-tripped without a renderer.

The consumer drives the direction. A parser that produced its own scene
type would force every consumer to translate; instead the parser **calls
`nsi_trait::Nsi`**, so the same code feeds a live 3Delight context, an
`nsi-intermediate` `Recorder`, or a backend's own implementation. Parse
to render, parse to inspect, parse to re-emit -- one parser.

The crate is `nsi-parse`. `nsi-stream` is taken: it is the GPU pixel
streaming surface of `specs/001-gpu-pixel-streaming` and has nothing to
do with this.

## User Stories

### User Story 1: Read A Stream Into Anything (P1)

As a tool author, I want to feed a `.nsi` file to any `Nsi`
implementation, so that reading a scene and doing something with it are
separate problems.

**Acceptance Criteria**

- Given a `.nsi` stream and a type implementing `Nsi`, when it is
  parsed, then that type receives one call per statement, in order.
- Given a stream 3Delight wrote, when it is parsed into a `Recorder` and
  written back out, then the two carry the same statements and values.
  They are compared per parameter rather than byte for byte, because the
  renderer groups a node's attributes into one call and this workspace's
  writer does not.
- Given a malformed stream, then parsing fails with an error naming the
  byte offset and what was expected, and the sink has received only the
  statements that preceded it.

### User Story 2: Read It Fast (P1)

As a pipeline author, I want a parser fast enough that reading a scene
is not the slow part of using it.

**Acceptance Criteria**

- Given a stream, when it is parsed, then no allocation happens per
  argument or per statement in the steady state.
- Given a string or handle with no escape sequences, when it is parsed,
  then it is borrowed from the input rather than copied.
- Throughput is measured, recorded, and regressions are visible.

### User Story 3: Read A Lua Scene (P2)

As a tool author, I want the same treatment for a `.lua` scene, so that
which front end a scene was written in stops mattering.

**Acceptance Criteria**

- Given a Lua script using the `nsi` table, when it is run, then the
  `Nsi` sink receives the same calls the renderer would make.
- Given a script this crate's own `write_lua` produced, when it is run
  into a `Recorder`, then the recorded scene equals the original.

### User Story 4: Read Compressed And Piped Input (P2)

As a pipeline author, I want to read what the renderer writes, which is
often compressed.

**Acceptance Criteria**

- Given a gzip or zstd compressed stream, when parsed, then the result
  equals parsing its decompressed form.

## Non-Goals

- `binarynsi`. ɴsɪ names the format; the encoding is undocumented. See
  `003`'s R28: matching it means reading the renderer's bytes.
- Rendering, or any interpretation of what the scene means. This crate
  turns bytes into ɴsɪ calls and stops.
- A scene type of its own. `nsi-intermediate` already has one, and a
  parser that insisted on it would be useless to a backend that does
  not want it.

## Requirements

- R1: Parsing drives `nsi_trait::Nsi`. The sink chooses what a parsed
  scene becomes.
- R2: The grammar is derived from observed 3Delight output, because the
  ɴsɪ specification does not give one. `renderdl -cat` is the oracle,
  and every rule cites a capture.
- R3: Every statement 3Delight writes is parsed: `Create`,
  `SetAttribute`, `SetAttributeAtTime`, `Delete`, `DeleteAttribute`,
  `Connect`, `Disconnect`, `Evaluate`, `RenderControl`.
- R4: Every argument type is parsed, with the flag prefixes and the
  `type[n]` array spelling `003` established.
- R5: A parse failure names the byte offset and what was expected.
  Partial application is visible, not rolled back: the sink has received
  the statements before the failure, and a caller that wants
  all-or-nothing parses into a `Recorder` first.
- R6: **The parser** allocates nothing as a scene grows: scratch buffers
  are cleared rather than freed, parameter names and string values are
  borrowed from the input, and a statement's argument list lives on the
  stack. A string *argument* does allocate, three times, inside
  `nsi_ffi_wrap::Arg` -- it owns a `Vec<CString>` because ɴsɪ's C
  boundary needs NUL-terminated strings. That cost belongs to the
  argument type, and the requirement says so rather than claiming a
  number the crate does not control.
- R7: An unescaped string is borrowed from the input, not copied.
- R8: Throughput is measured against a generated corpus and recorded in
  the spec, so a regression is a visible number rather than a feeling.
- R9: Lua is read by running it (`lua` feature). ɴsɪ's Lua *is* Lua, so
  an interpreter is the only honest reader: a script may compute its
  scene.
- R10: `gzip` and `zstd` inputs are read behind the features of the same
  name, mirroring `nsi-intermediate`'s writer.
- R11: A stream written by `nsi-intermediate` parses back into an equal
  scene, and a stream 3Delight wrote does too. Both are gates, not
  claims.

## Risks

- **A grammar with no specification.** Every rule is an observation, and
  an unobserved case is an assumption. Mitigated by making the corpus
  explicit: the gate parses 3Delight's own output for scenes built to
  exercise each rule, and an unhandled construct is a loud error rather
  than a skipped line.
- **Fast and wrong.** A parser optimised before it is correct is a
  liability. Correctness gates land first; the throughput number is
  recorded only once the round-trip gates pass.
- **A borrowed lifetime that does not fit the sink.** `Nsi::Arg` is a
  GAT, and the parser must build arguments that live exactly as long as
  the call. Getting this wrong shows up as a compile error rather than a
  bug, which is the reason to prefer it.
- **Lua is a dependency, not a format.** Running a script means
  embedding an interpreter and executing untrusted code. Feature-gated,
  and stated plainly rather than hidden behind "supports Lua".
