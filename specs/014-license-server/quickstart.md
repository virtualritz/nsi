# Quickstart: 3Delight License Server Helper

```rust,ignore
use nsi_3delight::license::{license_server_request, start_license_server};

// The caller decides whether to ask the user; this only reports.
if let Some(request) = license_server_request() {
    println!("3Delight has {} but no running server", request.license_file.display());
    if user_says_yes() {
        start_license_server().expect("licserver");
    }
}
```
```sh
cargo test -p nsi-3delight --lib license
cargo build -p nsi-3delight --no-default-features
```
