# Contract: Recording And Resolving A Change

## Scope

What `Scene::take_changes` records, and what `Scene::affected` makes of
it. All tests named here are in `nsi-intermediate`.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| The record is net, not a log | Covered | `scene/mod.rs` `Changes`, `record_edge` keyed on `(from, to, kind)` | `scene::tests::the_record_is_net_not_a_log` (forty sets, one entry) and `re_arming_one_edge_repeatedly_is_one_entry` (forty re-connects, one entry carrying the last call's arguments) | -- |
| A create undone by a delete reports only the delete | Covered | `scene/mod.rs` `delete`, `delete_recursive` remove the handle from `created` | `the_record_is_net_not_a_log` | -- |
| A delete records the edges it takes with it | Covered | `scene/mod.rs` `delete` copies matching edges before `retain` | `scene::tests::a_delete_records_the_edges_it_took_with_it` | -- |
| A `.all` disconnect records the edges it removed, not the pattern | Covered | `scene/mod.rs` `disconnect` collects inside the `retain` closure | `scene::tests::a_wildcard_disconnect_records_what_it_removed` | -- |
| A repeated `connect` that replaces arguments is recorded | Covered | `scene/mod.rs` `connect_with_args` records `edges_rearmed` | `scene::tests::a_connect_rearmed_in_place_is_recorded`; dropping the record reddens it. This is the quietest way a scene changes meaning: ɴsɪ's `"priority"` rides on those arguments, and no edge is added or removed | -- |
| Pending changes are outside scene equality | Covered | `scene/mod.rs` hand-written `PartialEq` | `scene::tests::changes_are_not_part_of_scene_equality`; adding `changes` to the comparison reddens it | -- |
| A transform edit affects everything below it | Covered | `scene/mod.rs` `affected`, `descend` | `scene::tests::a_moved_transform_dirties_its_subtree`; truncating the descent to one level reddens it and the property gate | -- |
| An `attributes` edit reaches what it binds, through a `set` | Covered | `scene/mod.rs` `through_bindings`, `set_members` | `scene::tests::a_shader_edit_reaches_the_geometry_bound_through_it`; dropping the members hop reddens it and the property gate | -- |
| A shader parameter costs no geometry work | Covered | `scene/mod.rs` `affected` files a `shader` node under `shaders` | `scene::tests::a_shader_parameter_edit_is_not_geometry_work`; descending from it instead reddens this test. **The property gate cannot see this** -- naming extra nodes is over-approximation, which `changed ⊆ affected` permits -- so this named test is the only thing holding it | -- |
| A prototype's ancestor reaches the instancer | Covered | `scene/mod.rs` `descend` follows `sourcemodels` outward | `scene::tests::a_prototypes_ancestor_reaches_the_instancer`. Conservative by choice: rendered on 3Delight, a prototype's own chain does **not** move what an instancer draws, so this names a node the renderer would not -- and the property gate stays green without it | -- |
| A severed `sourcemodels` names the instancer | Covered | `scene/mod.rs` `affected`, `EdgeKind::InstanceSource` arm inserts `edge.to` | `every_changed_answer_is_named_in_the_affected_set`; dropping the insert reddens it. Found by review: the instancer is the *other end* of that edge and below nothing, so descending from the source never named it, and a backend kept drawing instances of a prototype the scene no longer had | -- |
| `.root` or `.global` affects everything | Covered | `scene/mod.rs` `affected` sets `everything` and leaves `roots` empty | `scene::tests::a_global_edit_dirties_everything` | -- |
| A new `EdgeKind` cannot land silently | Covered | `scene/mod.rs` `affected` matches on `EdgeKind` with no wildcard | Compile-time: removing an arm fails the build. It matched `to_attr()` strings before, where a new variant fell into the `_` arm and a `ShaderNetwork` port named `objects` took the child arm | -- |
| **`changed ⊆ affected`** | Covered | The whole of `affected` | `scene::tests::every_changed_answer_is_named_in_the_affected_set`: every one of eleven edits on every handle of a fixture with nested transforms, an instancer with matrices, a `set`, rival shaders and an output chain, then 256 seeded pairs, brute-forcing `world_transform`, `geometry_binding`, `attribute_value`, `placements`, `instance_transforms`, `instance_sources` and `render_outputs` before and after. Reddens on: truncating the descent, ignoring re-armed edges, ignoring removed edges, dropping the set-members hop, not propagating from an `attributes` node, and dropping the instancer arm | -- |
| The answer costs what the edit costs | Covered | `scene/mod.rs` `affected` names roots; `Scene::descendants` expands | Measured, one transform edit, release: 5.2 us at 50 000 nodes and 1.85 us at 200 000 -- constant -- against 29 ms and 145 ms to enumerate the same answer | -- |
| Over-approximation is bounded | Partial | Roots are inserted without checking whether an ancestor is already a root | None | Two overlapping roots expand to the same union, so this is correctness-neutral and costs a consumer a repeated walk. Measure whether a real edit batch produces overlapping roots often enough to matter before adding the check. |
| A same-time re-set that erases an unreadable sample | Open | Inherited from `003`: `resolve/motion.rs` keeps the call log, so this one is closed there | See `003`'s `contracts/resolution.md` | Nothing owed here; listed so the boundary between the two surfaces is explicit. |

## Invariants

- **Over-approximate, never under.** Naming a node that did not move
  costs a consumer a re-resolution; missing one renders the old state
  with no error and no warning.
- **Candidates, not answers.** `placements`, `attribute_value_along`
  and the rest give the new truth; this says where to ask.
- **Keyed by handle.** One geometry under two parents is one entry.

## Failure Modes

- **Changed but not named.** The under-approximation above. The
  property gate is the only thing that can find the rule nobody thought
  of, and it **failed to discriminate three times** before it could:
  its `transformationmatrix` was an `f32`, which `matrix_of` refuses, so
  "move a transform" moved nothing; its priorities were `f32`, which
  3Delight does not read as priorities, so precedence was never
  exercised; and its instancer had no matrices, so
  `instance_transforms` was `Ok([])` before and after every edit. Each
  was found by mutating the walk and watching the gate stay green.
- **A gate that cannot fail.** Any new rule here needs a mutation that
  reddens it, recorded in the row.
