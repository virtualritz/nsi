# Contract: Names

| # | Given | Then | Evidence |
| - | ----- | ---- | -------- |
| N1 | A `nurbs` node with `nu` | stored as `u.count`; `nu` is gone | `shipped_names_are_stored_as_the_drafts` |
| N2 | `nicename` on any node | stored as `nice-name` | `shipped_names_are_stored_as_the_drafts` |
| N3 | `sourcemodels` on a `mesh` node | carried as written: the row is scoped to `instances` | `a_scoped_row_renames_only_on_its_node` |
| N4 | The same legacy name twice | one warning | `a_deprecated_name_warns_once` |
| N5 | An unknown name | carried as written, no warning | `unknown_and_kept_names_are_carried_as_written` |
| N6 | `P` on a `mesh` node | carried as written, no warning | `unknown_and_kept_names_are_carried_as_written` |
| N7 | `trimcurves.inside` 0 on a `nurbs` node | `nsi-tessellate` refuses it, rather than tessellate the outside as the inside | `nsi-tessellate`'s `read` refuses it |
| N8 | A node created as `outputdriver` | `node_type()` is `output-driver` | `node_types_are_stored_as_the_drafts` |
| N9 | A connection to `objects` on `instances` | `EdgeKind::InstanceSource` | `objects_classifies_by_the_node_it_reaches` |
| N10 | A connection to `objects` elsewhere | `EdgeKind::SceneMember` | `objects_classifies_by_the_node_it_reaches` |
| N11 | A legacy stream read and written | byte for byte the same | `recorder_replays_what_3delight_writes` |
| N12 | A scene built with draft names | written as legacy | `the_stream_carries_the_shipped_names` |
