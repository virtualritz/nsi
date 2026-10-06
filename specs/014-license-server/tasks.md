# Tasks: 3Delight License Server Helper

- [x] T1 `delight_root()` and `license_source()` (RLM, installed, per-user, config). *Evidence:* the `RLM` classification tests and `cargo test -p nsi-3delight --lib license`.
- [x] T2 `license_file()` as the file case. *Evidence:* the policy tests.
- [x] T3 `is_license_server_running()`. *Evidence:* `the_environment_probes_do_not_panic`; the up case is still Open in the contract.
- [x] T4 `start_license_server()`. *Evidence:* compiles and is clippy-clean; the manual QA below is owed.
- [x] T5 `license_server_request()` over the pure `license_server_request_for`. *Evidence:* the four policy tests.
- [x] T6 `pub mod license` in `nsi-3delight` and the umbrella's `delight` re-export. *Evidence:* `cargo build -p nsi-3delight --no-default-features` and `cargo check -p nsi --no-default-features --features delight`.
- [x] T7 Manual QA on a licensed machine: request is `Some`, start makes `licutils serverstatus` name `$DELIGHT/license.dat` and `serverlicenses` list one 3Delight seat. Done 2026-10-06.
