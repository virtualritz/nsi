# Contract: Lua Input

## Scope

Covers running a `.lua` ɴsɪ scene into a sink, behind the `lua` feature.

## Why This Contract Exists

ɴsɪ's Lua front end is a programming language, not a serialisation. A
script may compute the scene it describes, so reading one means running
it. That is a different trust decision from parsing a data file, and it
is stated rather than implied.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| The `nsi` table drives the sink | Covered | `lua.rs` `run_lua` binds all nine | `lua::a_lua_script_this_workspace_wrote_reads_back_the_same`, `lua::render_control_is_bound`. `RenderControl` was missing, so a scene ending in one failed with "attempt to call a nil value" | -- |
| `Delete` carries its parameters | Covered | `lua.rs` `Delete` reads a parameter list | `lua::delete_carries_its_parameters`. Dropping them turned ɴsɪ's `recursive` delete into a plain one -- a different scene | -- |
| Short tuple data is refused | Covered | `lua.rs` `param_of` checks the element width | `lua::short_tuple_data_is_refused`. Chunking silently turned a two-value point into an empty one | -- |
| A refusal stops the script | Covered | `lua.rs` `keep` returns an `Err` that unwinds the interpreter | `lua::a_refusal_stops_the_script`. Recording the error and running on contradicted what `Error::Sink` promises, and the sink kept receiving calls | -- |
| Parameter tables are read as ɴsɪ arguments | Covered | `lua.rs` `param_of` | Same test, over every type constant and an `arraylength`; wiring `TypeDoubleMatrix` to `TypeMatrix` or dropping the length fails it | -- |
| An untyped value takes ɴsɪ's default | Covered | `lua.rs` `param_of` infers from Lua 5.4's own integer/float distinction, so it is exact rather than a guess | `lua::a_computed_scene_is_read` asserts an untyped integer becomes an ɴsɪ `int` | -- |
| A script `write_lua` emitted round-trips | Covered | `lua.rs` | `lua::a_lua_script_this_workspace_wrote_reads_back_the_same` compares the two scenes' streams | -- |
| A computed scene is read correctly | Covered | `lua.rs` runs the script | `lua::a_computed_scene_is_read`, a loop building five nodes -- the case that makes an interpreter the only correct reader | -- |
| A script error surfaces, not panics | Covered | `lib.rs` `Error::Lua`, and `Error::Sink` carried out of the script rather than stringified | `lua::a_broken_script_is_an_error`, `lua::a_sink_refusal_escapes_the_script` | -- |
| Both parameter shapes are accepted | Covered | `lua.rs` `params_of` | `lua::both_parameter_shapes_are_accepted`; ɴsɪ allows variadic tables or one table of them |

| The type constants match the renderer's | Covered | `lua.rs` offers exactly the nine 3Delight's `nsi` table has | `lua::a_lua_script_this_workspace_wrote_reads_back_the_same`. `TypeDouble` and `TypeInt64` were offered here and are `nil` there, so a script written against this reader would have failed in the renderer |

## Invariants

- Running a script executes arbitrary code. The feature is off by
  default and the crate documentation says so plainly.
- The Lua surface accepts what ɴsɪ's does, which is a superset of what
  `nsi-intermediate::write_lua` emits.

## Required Evidence Before Marking Complete

- `write_lua` output round-tripping into an equal scene.
- A comparison against `renderdl -lua -cat` for the same script, so the
  interpretation is held against the renderer's rather than our own.
