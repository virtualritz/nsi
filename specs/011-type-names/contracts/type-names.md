# Contract: Type Names And `hpoint`

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| `Type` uses role, components, machine type | Covered | `nsi-trait/src/nsi_trait.rs` | `type_values_match_c_header` pins every discriminant, `Point4F32` = 10 included | |
| Old `Type` names compile, as values and patterns | Covered | deprecated associated constants | `tests/deprecated_names.rs::old_type_names_work_in_patterns` | |
| Old macros and wrapper names compile and mean the same | Covered | forwarders and aliases in `argument.rs`, `lib.rs` | `old_macros_forward_to_the_new_ones`, `old_wrapper_names_are_aliases` | `ArgData` variants have no aliases (research D5) |
| No crate uses an old name | Covered | the rewrite | clippy `-D warnings` on every affected crate: a leftover is a deprecation warning | |
| `Point4F32` sends `NSITypeHPoint` and renders | Covered | `nsi_tuple_data_array_def!(.., DataType::Point4F32, 4)` | `tests/hpoint.rs::a_rational_patch_renders_from_rust`, in 3Delight 2.9.210: with weight 2 the patch covers a quarter of the pixels. Falsified: tagging it `RealF32` again, the patch does not render | Research D2 |
| `hpoint` parses, records and writes back | Covered | `nsi-parse` `value.rs`, `lua.rs`; `nsi-intermediate` `owned`, `stream`, `lua` | `nsi-parse/tests/roundtrip.rs::an_hpoint_round_trips` | |
| Element counts include `Point4F32` | Covered | the three `components_per_element` matches | the round trip writes `2` for two `hpoint`s | Research D6: found by search, not by the compiler |

## Observed While Testing

`nsi-ffi-wrap`'s `tests/safety.rs` crashed once, without a panic
message, in one run of seven; the other six passed all nine tests. The
suite exercises contexts and callbacks, not type names, and the crash
did not reproduce. Recorded, not explained.
