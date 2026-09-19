# Contract: Weld Declarations

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| A `weld` connection is classified | Covered | `edge.rs` `EdgeKind::Weld` | the weld tests build their scenes through it | Additive: `EdgeKind` is `#[non_exhaustive]` |
| A table parses into uses with ordered segments | Covered | `resolve/weld/mod.rs` `Scene::weld_table` | `the_five_to_one_join_is_one_weld_with_two_uses`, `five_consecutive_trim_curves_are_one_use` | The draft's own examples |
| Uses group by `(weld node, id)` | Covered | `Scene::welds` | `one_table_declares_independent_joins`, `ids_are_local_to_their_weld_node`. Falsified: grouping by id alone | |
| Self-seams and non-manifold joins are kept | Covered | `Weld::is_non_manifold` | `self_seams_and_non_manifold_joins_are_kept` | |
| Trim-curve chains are checked, reversal and wraparound included | Covered | `trim_curves_continue` | `a_reversed_chain_runs_backwards_around_the_loop`, `a_gap_in_a_trim_curve_chain_is_reported_and_dropped`. Falsified: a check that always passes | |
| Mesh edges index into holes | Covered | `Boundaries::of` | `mesh_edges_index_into_holes`. Falsified: one loop per face | |
| Every invalid declaration is reported and dropped | Covered | `WeldProblemKind` | `every_invalid_declaration_is_reported` (8 cases), `counts_that_do_not_partition_the_arrays_are_reported`, `a_table_without_a_namespace_is_reported`, `two_namespaces_on_one_geometry_are_reported`. Falsified: partial loops allowed | |
| Unverifiable chains are kept and flagged | Covered | `ConnectivityUnverified` | `an_unverifiable_chain_is_kept_and_flagged` | D2 |
| A weld edit marks both sides of the seam affected | Covered | `scene/mod.rs` `EdgeKind::Weld` arm | Compiler-enforced exhaustive match | No dedicated test yet |
| A table round-trips through a stream | Covered | -- | `nsi-parse/tests/roundtrip.rs::a_weld_table_round_trips` | |
| Mesh-edge chain connectivity | Open | -- | -- | Needs vertex identity; the tessellator's (D3) |
| Occurrence scope under instancing | Open | -- | -- | The draft leaves it open |
