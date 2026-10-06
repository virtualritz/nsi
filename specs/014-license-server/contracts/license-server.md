# Contract: 3Delight License Server Helper

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| `delight_root()` reads `DELIGHT` | Covered | `license.rs` `delight_root` | `installed_licensed_and_down_requests` passes an explicit root through the policy | `None` when unset |
| `license_source()` classifies `RLM_LICENSE` | Covered | `license_source_from_rlm` | `a_port_at_host_rlm_entry_is_a_server`, `an_rlm_list_prefers_a_server_over_a_file`, `an_existing_rlm_file_entry_is_a_file`, `an_rlm_entry_that_is_neither_is_none` | A server entry wins |
| `license_source()` falls back to the installed, per-user and config sources | Covered | `installed_license`, `per_user_license`, `configured_server` | Manual QA 2026-10-06: `$DELIGHT/license.dat` is found | A zero-byte `licenses/3delight_license.dat` is skipped |
| `license_file()` is the file case only | Covered | `license_file` | `installed_licensed_and_down_requests` (policy), `an_existing_rlm_file_entry_is_a_file` | `None` for a server |
| `is_license_server_running()` treats `cannot connect` as down | Covered | `is_license_server_running` | `the_environment_probes_do_not_panic`; manual QA 2026-10-06: down reports `cannot connect`, up reports `Server status: running` | Both directions seen |
| `start_license_server()` spawns `licserver -d <license>` with null stdio | Covered | `start_license_server` | Manual QA 2026-10-06: `licutils serverstatus` then names `$DELIGHT/license.dat`, and `serverlicenses` lists one 3Delight seat to 2027 | Passes `license_file()` |
| A zero-byte license placeholder is skipped | Covered | `is_license_file` | `an_empty_file_is_not_a_license` | The server leaves one after a start with no file |
| The policy is `Some` iff root and license file are present and the server is down | Covered | `license_server_request_for` | `no_root_no_request`, `no_license_no_request`, `running_server_no_request`, `installed_licensed_and_down_requests` | The whole decision, as a pure function |
| A configured server suppresses the offer | Covered | `license_file` returns `None` for `Server`; `license_server_request_for` needs a file | `no_license_no_request` (The `None` it passes is what a server source yields) | `port@host` and `license.server` alike |
| No function panics when `DELIGHT`, `HOME` or the binaries are absent | Covered | `?` and `let ... else` throughout | `the_environment_probes_do_not_panic` | |

## Required Evidence Before Marking Complete

- `cargo test -p nsi-3delight --lib license`
- `cargo build -p nsi-3delight --no-default-features`
- Manual, done 2026-10-06: with a commercial license at
  `$DELIGHT/license.dat` and the server down, `license_server_request()`
  is `Some`; `start_license_server()` returns `Ok`; `licutils
  serverstatus` then reports `using license file:
  '.../license.dat'` and `licutils serverlicenses` lists one 3Delight
  seat.
