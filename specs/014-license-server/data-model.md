# Data Model: 3Delight License Server Helper

```text
$DELIGHT/
  bin/licutils              serverstatus answers, or says `cannot connect`
  bin/licserver             -d starts the daemon
  license.dat               the commercial licence
  licenses/
    3delight_license.dat    installed by the server; empty until one is
$HOME/.config/3delight/
  license.dat               per-user license the renderer reads
```

| Type | Fields |
| --- | --- |
| `LicenseSource` | `File(PathBuf)` or `Server(String)` |
| `LicenseServerRequest` | `delight_root: PathBuf`, `license_file: PathBuf` |

Functions, not state. Nothing is cached in the module, because the
server can be started and stopped outside the process; each call reads
the environment and the tool afresh.
