# Plan: 3Delight License Server Helper

## Approach

- One module, `crates/nsi-3delight/src/license.rs`, compiled without a
  feature gate, so a `default-features = false` consumer gets it.
- The decision is split. `license_server_request()` reads the
  environment; the private `license_server_request_for(root, license,
  running)` is the whole policy, and the tests call it directly.
- `is_license_server_running()` runs `licutils serverstatus` and reads
  the combined output for `cannot connect`, which is written to stderr.
- `start_license_server()` spawns `licserver -d` with stdio null and
  returns; the daemon detaches itself.

## Gates

1. The pure policy: no root, no license, a running server, and the one
   `Some` case.
2. The environment probes are callable with `DELIGHT` unset and do not
   panic.
3. The module compiles with `--no-default-features`.
4. Manual, once: with a license installed and the server down,
   `license_server_request()` is `Some` and `start_license_server()`
   makes `licutils serverstatus` answer.

## Artifacts

- `spec.md`, `research.md`, `data-model.md`, `contracts/license-server.md`,
  `quickstart.md`, `tasks.md`, `checklists/requirements.md`.
