# Contract: Output Formats

## Scope

Covers the formats a recorded `Scene` can be written back out as: the
`.nsi` stream (always), its compressed variants (`gzip`, `zstd`
features), and a Lua script (`lua` feature). Byte-level fidelity of the
plain stream is `stream.md`; this covers the feature-gated surface and
the Lua emitter.

## Why This Contract Exists

ɴsɪ has three front ends -- the C API, the stream, and Lua -- and they
are not equivalent. The Lua binding exposes **fewer types** than the C
API, so a scene that records perfectly can be inexpressible as a script.
Emitting it anyway is silent data loss: an untyped Lua number becomes a
`float`, so a `double` loses precision and a 64-bit integer comes back
as a different number entirely. Verified against 3Delight 2.9.207:
naming `nsi.TypeDouble` or `nsi.TypeInt64` is a parse error.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| A Lua script rebuilds the recorded scene | Covered | `lua.rs` `write_lua` | `lua_roundtrip::a_lua_script_rebuilds_the_recorded_scene`: the script is fed to `renderdl -lua -cat`, and 3Delight's resulting stream is compared against this crate's own for the same scene | -- |
| A typed Lua parameter's data is a table | Covered | `lua.rs` `write_arg` always writes `data={...}` | Same gate. A bare typed scalar makes 3Delight write `"name" "float" 0 [ ]` -- count zero, no value -- so this is not cosmetic | -- |
| Lua refuses a type it cannot express | Covered | `lua.rs` `lua_type_name` returns `None` for `F64`, `I64`, `Reference`, `Invalid` | `lua_roundtrip::lua_refuses_what_it_cannot_express`, over a `double` and a 64-bit integer | -- |
| Lua refuses an argument carrying flags | Covered | `lua.rs` `write_arg` rejects `PerFace`/`PerVertex`/`InterpolateLinear` | Same test, over a `per_vertex` normal. A Lua parameter table has nowhere to put a flag, and every spelling was probed and ignored by the renderer -- so the argument rebuilt a *different surface* while the gate stayed green | -- |
| Lua refuses an empty string array | Covered | `lua.rs` `write_arg` | Same test. Setting one from Lua aborts 3Delight with a heap error, so emitting it hands a consumer a script that kills the renderer | -- |
| `array_len(1)` survives both writers | Covered | `stream/mod.rs` `type_name` and `lua.rs` `write_arg` key on `IsArray`, not on length | Both round-trip fixtures set one; 3Delight writes `float[1]`. Keying on `> 1` dropped it silently | -- |
| `doublematrix` survives Lua | Covered | `lua.rs` `Type::MatrixF64 => TypeDoubleMatrix` | The gate's fixture sets a `transformationmatrix`; emitting it as `TypeMatrix` fails the gate | -- |
| `arraylength` survives Lua | Covered | `lua.rs` `write_arg` | The gate's fixture sets `resolution` with `array_len(2)`; dropping it fails the gate | -- |
| Lua strings are escaped | Covered | `lua.rs` `quoted` | The gate's fixture uses a name containing a quote; leaving it raw fails the gate | -- |
| Every connection class emits in Lua | Covered | `lua.rs` uses `EdgeKind::to_attr` | The gate's fixture drives `objects`, `geometryattributes` and `surfaceshader`; emitting a fixed attribute fails the gate | -- |
| Motion samples and connection arguments emit in Lua | Covered | `lua.rs` `nsi.SetAttributeAtTime`, and `edge.args` under `nsi.Connect` | The gate's fixture sets a sample at `1/3` and a prioritised connection | -- |
| Reserved handles are not created in Lua | Covered | `lua.rs` skips `crate::is_reserved` | The Lua gate's fixture now sets `.global`, so this is proven through the renderer rather than by shared-code reasoning | -- |
| gzip round-trips | Covered | `stream.rs` `write_stream_with`, `flate2::write::GzEncoder` | `compression::gzip_decompresses_to_the_plain_stream` | -- |
| zstd round-trips | Covered | `stream.rs` `write_stream_with`, `zstd::stream::write::Encoder` | `compression::zstd_decompresses_to_the_plain_stream` | -- |
| **3Delight reads a compressed stream** | Covered | Same | `compression::the_renderer_reads_a_gzipped_stream` feeds the file to `renderdl -cat`. Writing zlib instead of gzip -- the same DEFLATE data in a different container -- fails this and the round-trip, which is what makes it worth having | -- |
| A compressor is finished, not merely dropped | Covered | `stream.rs` calls `finish()` on both encoders | The round-trip tests. Note `GzEncoder` also finishes on `Drop`, so the explicit call buys error propagation rather than correctness; `zstd`'s does not, and dropping it fails the round-trip | -- |
| The extension names the compressor | Covered | `stream.rs` `Compression::extension` | `compression::the_extension_names_the_compressor` | -- |
| 3Delight does **not** read a zstd stream | Covered | `stream/mod.rs` `Compression` documentation | Probed: `renderdl -cat` on a `.nsi.zst` fails with `E1000 Invalid char`, and a context with `streamcompression="zstd"` writes plain text byte-identical to one with a bogus value. The feature is for consumers of this crate, and now says so rather than claiming an ɴsɪ format | -- |
| `binarynsi` | Open | Not implemented | None | ɴsɪ names three stream formats: `nsi`, `binarynsi` and `autonsi` (which picks `nsi` for a `.nsia` name and `binarynsi` otherwise). The binary encoding is **not documented**; implementing it means reading 3Delight's output byte by byte, as the text format was. Decide whether a backend needs it. |
| A compressed Lua script | Open | Not implemented | None | ɴsɪ's `streamcompression` is a property of a *stream*. Whether 3Delight reads a gzipped `.lua` is unknown and untested; a caller can wrap the writer itself. |

## Invariants

- Compression does not change the format. A compressed stream
  decompresses to exactly the bytes `write_stream` would have written.
- The Lua emitter writes **one attribute per `nsi.SetAttribute` call**,
  matching `write_stream`. Lua would allow one call per node, but then
  the two emitters would disagree about statement boundaries and could
  not be compared against each other -- which is how the Lua gate works.
- The Lua emitter shares the classifier's `EdgeKind::to_attr`, so it
  cannot drift from the stream emitter's spelling of a connection.

## Failure Modes

- **`LuaError::Inexpressible`** names the node, the attribute and the
  type. It is not recoverable by retrying; the scene has to change, or
  the consumer has to use the stream.
- **No 3Delight** fails both gates. A missing prerequisite is a failure,
  never a pass.

## Required Evidence Before Marking Complete

- `cargo test -p nsi-intermediate --features lua,gzip,zstd`, with
  `DELIGHT` set and a reachable licence server.
- To close the zstd row: feed a `.nsi.zst` to `renderdl -cat`.
- To close the `binarynsi` row: capture a binary stream from a
  `streamformat="binarynsi"` context and decide whether to match it.
