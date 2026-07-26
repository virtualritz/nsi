# Quickstart: Shading Profile

**Current status: spec only.** `crates/nsi-profile` does not exist yet; the
commands below are the verification path once tasks in `tasks.md` land.
Nothing here is evidence until it has actually been run.

## Build And Test

```bash
# Registry, resolution, validator, layout tests (no renderer required).
cargo test -p nsi-profile

# MaterialX interchange rows.
cargo test -p nsi-profile --features materialx

# Parity harness (requires 3Delight installed and DELIGHT set).
cargo test -p nsi-profile --features parity
```

Never use `--release` (repo rule, see `AGENTS.md`). Expected-image updates
(`RUST_TEST_UPDATE=1`) require explicit human approval.

## Manual QA Path

1. Validate a fixture scene:

   ```bash
   cargo run -p nsi-profile --bin nsi-profile-validate -- \
       tests/fixtures/conforming.nsi
   ```

   Confirm empty report, exit code 0; repeat with
   `tests/fixtures/violating.nsi` and confirm the report names the shader
   node handle, construct, and profile version.
2. Render a parity fixture both ways and inspect the comparison output:

   ```bash
   cargo test -p nsi-profile --features parity parity_standard_surface -- --nocapture
   ```

3. For emission parity, run the light-rig fixture and confirm each light
   pattern's illumination difference is within its declared threshold.

Record 3Delight version, OS, and thresholds when citing this path as
contract evidence.
