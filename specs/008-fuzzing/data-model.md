# Data Model: Fuzzing The Untrusted Boundaries

## Layout

| Path | Committed | Holds |
| --- | --- | --- |
| `crates/nsi-parse/fuzz/Cargo.toml` | yes | the fuzz crate, its own workspace |
| `fuzz/fuzz_targets/parse_stream.rs` | yes | the target and its do-nothing sink |
| `fuzz/seeds/parse_stream/` | yes | ten hand-written streams, one per statement kind |
| `fuzz/corpus/` | no | what the fuzzer grows; working state |
| `fuzz/artifacts/` | no | reproducing inputs for a crash, until one becomes a test |
| `fuzz/target/`, `fuzz/coverage/` | no | build output |

## The Target's Contract

Input: any `&[u8]`, unchanged. Output: none. The single assertion is
that `parse_stream` returns -- `Ok` for a valid stream, any `Err` for
anything else. A panic, an abort, a timeout or a sanitizer report is a
finding.
