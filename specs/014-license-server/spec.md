# Feature: 3Delight License Server Helper In `nsi-3delight`

## Problem

3Delight's free license is one seat per machine, held in process: a
second 3Delight process on the same host is refused with `Cannot run
more than one 3Delight free license at once`. A machine with a
commercial license runs `licserver`, a daemon that hands out seats; with
it up, several renderers coexist.

Nothing in this workspace starts that daemon. An application embedding
`nsi-3delight` -- akatela's viewport is the first -- would reimplement
the same three checks (is 3Delight installed, is a license present, is
the server up) and the same spawn, per application.

## User Stories

1. **As an application embedding `nsi-3delight`**, I ask one function
   whether a license server should be offered, and one function to start
   it; I do not touch `DELIGHT` paths or the vendor tools.
2. **As a user with a commercial license**, my application offers to
   start `licserver` when it is not running, and stays quiet when only
   the free tier is present.
3. **As the crate's test suite**, the decision is a pure function over
   the three environment answers, so it is exercised with no 3Delight
   installed.

## Acceptance Criteria

- `delight_root()` returns `$DELIGHT`, or `None` when it is unset.
- `license_source()` returns the configured license, in order:
  `RLM_LICENSE` (an entry containing `@` is a [`LicenseSource::Server`],
  an existing entry otherwise is a [`LicenseSource::File`]; a server
  anywhere in the list wins), then `$DELIGHT/license.dat` or the
  server's installed `$DELIGHT/licenses/3delight_license.dat`, then the
  per-user `$HOME/.config/3delight/license.dat`, then the
  `license.server` key of `$DELIGHT/3delight.config`. A zero-byte file
  is skipped, because starting the server leaves such a placeholder
  behind.
- `license_file()` returns the file when `license_source()` is a file,
  and `None` for a server or no license.
- `is_license_server_running()` is true when `$DELIGHT/bin/licutils
  serverstatus` answers and false when it reports `cannot connect`. The
  exit code is `0` in both cases, so the tool's output is the signal.
- `start_license_server()` runs `$DELIGHT/bin/licserver -d <license>`
  with the file [`license_file`] finds, and no inherited stdio, and
  returns once the process is spawned.
- `license_server_request()` is `Some` exactly when a `DELIGHT` root and
  a local license file are present and the server is not running, and
  carries both paths. A configured `port@host` or `license.server` is
  `None`: a server is already the setup, and it is not necessarily the
  local one.
- No function panics when `DELIGHT`, `HOME` or the binaries are absent.

## Non-Goals

- **Automatic start, stop or scheduling.** The caller decides. This
  crate only reports and spawns; a server is machine-wide and outlives
  any one application, so stopping it is never this module's call.
- **Managing the license file**, or the server's own lifecycle
  (`-autostart`, `-unschedule`, `serverinstalllicense`).
- **The free tier.** It needs no server. `license_server_request()` is
  `None` for it because no license file is present, not because the
  tier is detected.

## Risks

- **`licutils` output is not an interface.** The only stable signal is
  the `cannot connect` line, which goes to stderr; the exit code is `0`
  either way. It is read in one function.
- **`licserver -d` daemonises.** The spawned child exits promptly, so
  the call does not wait. On a platform where it falls back to console
  mode the child stays; the helper still does not wait.
- **The paths are 3Delight's, not standardised.** They live in one
  function, so a move is local.
