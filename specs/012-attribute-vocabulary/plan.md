# Plan

1. **Generate the table.** Parse the draft's mapping into
   `crates/nsi-intermediate/src/names/table.rs`: the 182 attribute
   pairs and the node-type pairs, sorted, `const`.
   -> verify: the generator reports the same counts as `research.md`,
   and the committed file compiles.

2. **The lookup module.** `names::{draft_name, legacy_name,
   draft_node_type, legacy_node_type}` over that table, plus
   warn-once.
   -> verify: unit tests for a common row, a node-scoped row, an
   unknown name, and the reverse direction.

3. **Canonicalize on the way in.** `Scene::create` maps the node type;
   `Scene::set_attribute` and the connection path map the name, warning
   once. The consolidated `trim-curves.position*` forms are reported.
   -> verify: a recorded legacy scene answers by draft names, and
   `problems` names the consolidated form.

4. **Edge kinds by node type.** `objects` into an `instances` node is
   `InstanceSource`; elsewhere `SceneMember`. The draft's renamed
   destinations classify as their legacy ones did.
   -> verify: the instances tests pass with both spellings.

5. **Writers emit legacy.** The stream and Lua writers map draft names
   and node types back.
   -> verify: a legacy stream round-trips byte for byte; a scene built
   with draft names writes legacy.

6. **Update this repository's readers.** `nsi-tessellate` reads
   `u.count`, `trim-curves.*`; `P`/`Pw` are unchanged.
   -> verify: the cube and the real STEP part still weld watertight and
   render closed.
