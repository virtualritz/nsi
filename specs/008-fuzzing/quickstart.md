# Quickstart: Fuzzing The Untrusted Boundaries

Needs the nightly toolchain, which this workspace uses already, and
`cargo install cargo-fuzz`.

```sh
cd crates/nsi-parse

# Build only -- what CI does.
cargo fuzz build parse_stream --target x86_64-unknown-linux-gnu

# Run. The first directory is where new inputs go; the second seeds it.
cargo fuzz run parse_stream --target x86_64-unknown-linux-gnu \
    fuzz/corpus/parse_stream fuzz/seeds/parse_stream \
    -- -max_total_time=300 -rss_limit_mb=1024
```

Keep the memory limit: an unbounded run on a machine that is also
building was killed by the OOM killer. Pass `--target` where the host's
default is musl; see `research.md` D5.

## After A Crash

The input is written under `fuzz/artifacts/parse_stream/`. Reduce it
with `cargo fuzz tmin parse_stream <file>`, add the bytes to an
ordinary test in `crates/nsi-parse/tests/`, watch that test fail, then
fix the parser.
