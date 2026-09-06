# Quickstart: ɴsɪ Parsing

## Build

```bash
cd ~/code/crates/nsi
cargo build -p nsi-parse
```

No renderer is needed to build or to run most tests.

## Verification Commands

```bash
cargo test -p nsi-parse                              # grammar.md
cargo test -p nsi-parse --features lua               # lua.md
cargo test -p nsi-parse --features gzip,zstd
cargo clippy -p nsi-parse --all-targets -- -W warnings
```

## Manual QA Path

The meaningful gate needs 3Delight: it generates its corpus by running
`renderdl -cat` over scenes and parsing what comes back, so the parser is
held against the renderer's output rather than against this workspace's.
`DELIGHT` must be set and the licence server reachable.

```bash
cargo test -p nsi-parse --test renderdl -- --nocapture
```

**Without 3Delight this gate cannot pass, and its absence is not a
licence to mark `grammar.md` `Covered`.**

## Measuring Throughput

```bash
cargo test -p nsi-parse --release --test throughput -- --nocapture
```

Record the figure, the machine and the profile in
`contracts/performance.md`. A number without those is not a comparison.
