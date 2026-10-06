# Tasks: 3Delight License Server Helper

- [ ] T1 `delight_root()` and `license_source()` (RLM, installed, per-user, config). *Evidence:* the `RLM` classification and `license` unit tests.
- [ ] T2 `license_file()` as the file case. *Evidence:* the policy tests.
- [ ] T3 `is_license_server_running()`. *Evidence:* `the_environment_probes_do_not_panic`.
- [ ] T4 `start_license_server()`. *Evidence:* compiles; manual QA owed.
- [ ] T5 `license_server_request()` over the pure `license_server_request_for`. *Evidence:* the four policy tests.
- [ ] T6 `pub mod license` in `nsi-3delight` and the umbrella's `delight` re-export. *Evidence:* `--no-default-features` build.
- [ ] T7 Manual QA on a licensed machine: request is `Some`, start makes `licutils serverstatus` answer.
