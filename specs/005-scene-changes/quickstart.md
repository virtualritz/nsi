# Quickstart: What Changed

## Verify

```bash
cargo test -p nsi-intermediate --lib scene::tests
cargo test -p nsi-intermediate --lib every_changed_answer_is_named_in_the_affected_set
```

The second is the gate: every single edit on every handle of a fixture
carrying nested transforms, an instancer with matrices, a `set`, rival
shaders and an output chain, then 256 seeded pairs, brute-forcing every
resolved answer before and after.

## Use

```rust,ignore
// Once per synchronise, under the caller's own lock.
let changes = scene.take_changes();
let affected = scene.affected(&changes);

if affected.everything {
    // `.root` or `.global` moved: re-read the scene.
} else {
    for root in &affected.roots {
        // Everything at or below `root` may have moved. A backend
        // walking its own objects compares against these; one that
        // wants the list calls `scene.descendants(root)`.
    }
}
for shader in &affected.shaders {
    // A material parameter. No geometry work.
}
if affected.outputs {
    // Re-read `scene.render_outputs()`.
}
```

## Manual QA

The `synchronize` semantics this is built on were driven against
3Delight interactively: edits without a synchronise report nothing, and
the synchronise reports `Restarted` then `Synchronized` once for the
whole batch. Re-run that by driving a live context through
`nsi-ffi-wrap` with `DELIGHT` set and a licence server up, editing one
attribute between synchronises, and reading the status callback.
