# Research: 3Delight License Server Helper

## D1: `licutils`, not a socket

Measured on 3Delight 2.9.210, Linux: `licutils serverstatus` exits `0`
with no server, printing `cannot connect to server 'localhost'` on
**stderr**; stdout is empty. No port is documented for the commercial
server, and the free tier's UDP 7419 broadcast is not it. So the vendor
tool's output is the interface, and the exit code cannot be used.

## D2: a daemon, not a child to babysit

`licserver -d` prints nothing and daemonises; `-showpid` is the way to
learn the daemon's PID, and `-autostart` installs a service. This helper
starts only, and does not track the process: the caller starts and
forgets, and the server outlives it.

## D3: licence at two paths

`licserver`'s strings name `/licenses/3delight_license.dat` relative to
the installation, which is `$DELIGHT/licenses/3delight_license.dat`.
`$HOME/.config/3delight/license.dat` is the per-user file the renderer
reads. Either existing means "has a license". The free tier has neither,
which is why its absence is the free-tier test.

## Rejected

- **Checking for the server's port.** Undocumented, and the free tier's
  UDP 7419 would answer for the wrong thing.
- **`pgrep`/process scan.** Platform-specific and racy against a daemon
  the caller does not own.
- **Starting the server for the caller.** A machine-wide daemon
  outliving the application is the caller's decision, per the spec's
  non-goals.

## D4: `RLM_LICENSE` is the environment variable, found in the binary

3Delight's renderer embeds the **RLM** (Reprise) client through
Foundry's `fnRlm.cpp`, and `lib3delight.so` names the variable it
reads in two places: the diagnostics environment list (`RLM_LICENSE`
alongside `RLM_ROAM`, `RLM_QUEUE`, `RLM_PROJECT`, `RLM_PATH_RANDOMIZE`,
`RLM_CONNECT_TIMEOUT`, `RLM_LICENSE_PASSWORD`) and the unlicensed error
`No license file has been specified via RLM_LICENSE, and there are no
*.lic files in the current directory`. The `%s_LICENSE` string in the
same block is the display pattern that builds `RLM_LICENSE` from
`%s = "reprise"`, not a second variable.

There is no 3Delight-specific name: `3DELIGHT`, `DL_LICENSE` and
`DELIGHT_LICENSE` appear nowhere in `lib3delight.so`, `renderdl`,
`licserver` or `licutils`. `nsi.pdf` documents `license.server` only as
an ɴsɪ global attribute; `3delight.config` takes it as a key.

`RLM_LICENSE` entries are `:`-separated on Unix (RLM also accepts `;`),
each a license file or `port@host`. A server entry means the machine is
already pointed at a server, which may be remote, so it suppresses the
offer rather than triggering a local one. A file entry is served by the
local `licserver` exactly like the two fixed paths.
