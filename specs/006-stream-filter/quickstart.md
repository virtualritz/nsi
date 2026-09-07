# Quickstart: Filtering an ɴsɪ Stream

## Verify

```sh
# The filter contract, including the Lua writer.
cargo test -p nsi-parse --test filter --features lua

# The oracle: 3Delight writes a stream, the filter passes it through,
# and `renderdl -cat` reads the result. Needs 3Delight installed and
# `$DELIGHT` set.
cargo test -p nsi-parse --test renderdl

# Both writers, both handle representations.
cargo clippy -p nsi-intermediate -p nsi-parse --all-targets -- -D warnings
cargo clippy -p nsi-intermediate -p nsi-parse --all-targets \
  --features nsi-intermediate/ustr_handles -- -D warnings
```

## Manual QA

```sh
cat > /tmp/scene.nsi <<'NSI'
Create "xf" "transform"
Create "light" "attributes"
Connect "xf" "" ".root" "objects"
Connect "light" "" ".root" "lightset"
NSI

cargo run -p nsi-parse --example filter -- /tmp/scene.nsi lightset
```

The `lightset` connection is gone and every other statement is
unchanged. Feed the output to `renderdl -cat` to have 3Delight confirm
it reads.

## Writing One

```rust
use nsi_intermediate::StreamWriter;
use nsi_trait::Nsi;

struct Passthrough<N>(N);

impl<N: Nsi> Nsi for Passthrough<N> {
    type Arg<'call> = N::Arg<'call>;
    type Error = N::Error;
    // Forward the nine methods; return `Ok(())` from one to swallow
    // that call, `Err` to abort the stream.
}

let writer = StreamWriter::new(std::io::stdout());
nsi_parse::parse_stream(&source, &Passthrough(writer))?;
```

Swap `StreamWriter` for `LuaWriter` to write a script, or for
`nsi_ffi_wrap::Context` to render instead of writing.
