# Quickstart: Parsing A Stream In Parallel

```toml
nsi-parse = { version = "0.1", features = ["parallel"] }
```

```rust,ignore
let bytes = std::fs::read("scene.nsi")?;
let context = nsi::Context::new(None).expect("context");
nsi_parse::parse_stream_parallel(&bytes, &context)?;
```

## Gates And Figures

```sh
cargo test -p nsi-parse --features parallel --test parallel
cargo test -p nsi-parse --features parallel --test throughput -- --ignored --nocapture --test-threads 1
```

`--test-threads 1` so the three measurements do not compete for cores.
`throughput_into_3delight` needs 3Delight.
