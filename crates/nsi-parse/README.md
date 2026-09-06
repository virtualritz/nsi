# `nsi-parse`

[![Build](https://github.com/virtualritz/nsi/workflows/Build/badge.svg)](https://github.com/virtualritz/nsi/actions)
[![Documentation](https://docs.rs/nsi-parse/badge.svg)](https://docs.rs/nsi-parse)
[![Crate](https://img.shields.io/crates/v/nsi-parse.svg)](https://crates.io/crates/nsi-parse)

<!-- cargo-rdme start -->

A fast reader for ɴsɪ scenes.

`nsi-intermediate` writes ɴsɪ out; this reads it back. Together they
close the loop: a scene can be captured, inspected, replayed and
round-tripped without a renderer.

## It calls you

Parsing drives `nsi_trait::Nsi` rather than producing a scene type
of its own, so the same parser feeds a live 3Delight context, an
`nsi-intermediate` `Recorder`, or a backend's own implementation. A
reader that insisted on its own representation would make every
consumer translate.

```rust
let bytes = std::fs::read("scene.nsi")?;
let recorder = nsi_intermediate::Recorder::new();
parse_stream(&bytes, &recorder)?;
```

## The grammar is observed, not specified

ɴsɪ publishes no grammar for its stream, only examples. This parser
is written against what 3Delight accepts, and the decisive
observation is that **an entire scene on one line parses**: the
newlines and indents a renderer writes are formatting, not syntax.
So a parameter list runs until the next *bare* token naming a
statement -- parameter names are always quoted, which makes that
unambiguous -- and a line-oriented reader would be wrong on valid
input.

## Features

| Feature | What it adds |
| --- | --- |
| *(none)* | The `.nsi` stream reader. |
| `lua` | Reading a Lua scene, which **runs** the script. Builds Lua 5.4 from vendored C source. |
| `gzip` | Reading a gzip-compressed stream. |
| `zstd` | Reading a zstd-compressed stream. |

Reading a Lua scene means executing it. ɴsɪ's Lua front end is a
programming language -- a script may compute the scene it describes
-- so an interpreter is the only correct reader, and that is a
different trust decision from parsing a data file.

<!-- cargo-rdme end -->

## Testing

```bash
cargo test -p nsi-parse --features lua,gzip,zstd
```

One gate needs 3Delight: `tests/renderdl.rs` builds a scene through a
real `apistream` context and parses what the renderer wrote, with
grouped attributes, wrapped continuation lines and the renderer's own
float spellings. A round-trip against this workspace's own writer proves
much less, because that writer emits one attribute per statement and
never wraps.

## Specification

Behaviour is specified in
[`specs/004-nsi-parse/`](https://github.com/virtualritz/nsi/tree/master/specs/004-nsi-parse),
with a contract matrix per surface. The stream grammar there is recorded
as **observed**, not quoted: ɴsɪ publishes no grammar for its stream, so
every rule names the probe that established it.

## License

MIT OR Apache-2.0 OR Zlib, at your option.
