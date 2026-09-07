# Contract: Filtering an ɴsɪ Stream

A filter is any [`Nsi`] implementation holding another. This contract
is on the two writers that let one terminate in a stream, and on the
properties a filter must have that a `Scene` does not.

| Behaviour | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| An identity filter changes nothing | Covered | `stream/mod.rs` `StreamWriter` | `nsi-parse` `tests/filter.rs::an_identity_filter_changes_nothing`: byte-identical output and an equal `Scene` | -- |
| Repeated calls survive, in order | Covered | `StreamWriter` writes per call and holds no state | `tests/filter.rs::repeated_calls_survive_in_order`: two `SetAttribute` on one name, `45` before `60`, where the same calls through a `Scene` write **none** -- the later `DeleteAttribute` undid them | This is the property that makes the writer a different tool from `write_stream` |
| A call is dropped by not forwarding it | Covered | Nothing in the writers; the filter simply returns `Ok` | `tests/filter.rs::a_swallowed_call_is_absent_from_the_output`, and the `filter` example | 3Delight's `nsicallbacks` returning `false` |
| A `Create` carries its parameters | Covered | `StreamWriter::create` writes `args` | `tests/filter.rs::a_create_carries_its_parameters`; dropping them reddens it | A `Scene` discards them, so this is the writer's business |
| `RenderControl` writes its action exactly once | Covered | `StreamWriter::render_control` skips an `action` parameter and appends the typed one | `tests/filter.rs::a_render_control_round_trips_its_action` and `::a_redundant_action_parameter_is_not_written_twice`; passing the incoming `action` through reddens the second | The stream carries it as a parameter, the trait as an enum |
| The Lua writer says the same thing | Covered | `lua.rs` `LuaWriter` | `tests/filter.rs::the_lua_writer_agrees_with_the_stream_writer` (feature `lua`): the same calls through both parse to equal `Scene`s | -- |
| A call Lua cannot express is refused, not mangled | Covered | `LuaWriter::create` returns `LuaError::CreateArguments`; `write_arg` refuses wide types, flags and empty string arrays | `tests/filter.rs::the_lua_writer_refuses_a_create_with_parameters` | `nsi.Create` takes a handle and a type and nothing else |
| 3Delight reads what a filter wrote | Covered | Both writers share `write_arg` with `write_stream` | `tests/renderdl.rs::a_filtered_stream_is_one_3delight_reads_back`: the renderer writes a stream, the filter passes it through, `renderdl -cat` reads the result and every statement value matches. Skipping one parameter name in the writer reddens it | The strongest gate here: the oracle reads our output |
| A `Reference` parameter is dropped with its line | Covered | `write_arg` returns early for `OwnedData::Reference` | `nsi-intermediate` `stream::tests` (pre-existing) | A host pointer has no stream form; 3Delight omits the line |
| Constant memory over a stream of any size | Partial | The writers hold a `Mutex<W>` and one converted argument | No test measures a large stream through a filter | Structural: nothing is retained between calls. A measurement would need a fixture larger than this repo carries |
| Binary `.nsi` input | Open | `parse_stream` rejects it with `Error::BinaryStream` | -- | Out of scope; see `spec.md` |

## Required Evidence Before Marking Complete

- `Partial` (constant memory): a test that filters a stream of at least
  one million statements while sampling peak resident memory, and
  asserts it does not grow with statement count. Needs a generated
  fixture, not one stored here.
- `Open` (binary input): a binary reader, gated by
  `renderdl -cat -binary` output as the oracle, and a round-trip
  through the filter for the same scene in both encodings.

## Commands

```sh
cargo test -p nsi-parse --test filter --features lua
cargo test -p nsi-parse --test renderdl   # needs 3Delight
cargo run -p nsi-parse --example filter -- scene.nsi lightset
```
