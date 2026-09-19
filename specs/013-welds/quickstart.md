# Quickstart: Weld Declarations

```rust,ignore
let welds = scene.welds();
for problem in &welds.problems {
    log::warn!("{problem}");
}
for weld in &welds.welds {
    // weld.weld, weld.id: the identity; weld.uses: each geometry's chain.
}
```

```sh
cargo test -p nsi-intermediate --lib weld
cargo test -p nsi-parse --test roundtrip
```
