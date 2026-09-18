# Quickstart: Procedurals In Rust

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
nsi-ffi-wrap = "0.10"
nsi-procedural = "0.1"
```

Implement `Procedural`, invoke `nsi_procedural::declare_procedural!` once
at the crate root, and build. Then from any scene:

```text
Evaluate
    "type" "string" 1 "dynamiclibrary"
    "filename" "string" 1 "path/to/libmy_procedural.so"
```

or a `procedural` node with the same `type` and `filename`.

## Run The Example

```sh
cargo build --example hello_procedural
cargo test -p nsi-procedural
```

`tests/evaluate.rs` builds the example itself and evaluates it in
3Delight.

## Link One Into A Rust Renderer

```rust,ignore
let procedural = MyProcedural::load(&report, "my renderer 1.0")?;
nsi_procedural::execute(&procedural, &my_nsi, &report, &args)?;
```
