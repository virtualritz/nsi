# `nsi` -- To Do

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.

## Known defects

### PixelFormat layer boundary detection

The heuristic for detecting layer boundaries in `PixelFormat::new()` has two known defects:

1. **Indexed channels (ndspy `.000` suffix) self-trigger boundaries**: The suffix "s" (scalar) appears in both the layer-ender set ["b","z","s","a"] and layer-starter set ["r","x","s"], causing a single scalar channel to match both patterns and incorrectly emit duplicate layers. A single `"beauty.000"` produces 2 duplicate layers instead of 1.

2. **Layers starting with certain channels are silently dropped**: The boundary heuristic only recognizes layer starts when a channel matches ["r","x","s"]. Layers starting with other channels (e.g., "z" for vector components) never trigger a boundary and get merged into the preceding layer, silently dropping their channels.

See `crates/nsi-ffi-wrap/tests/pixel_format_public.rs` for documented test cases (marked `#[ignore]`) and expected correct behavior. Fixing the layer boundary heuristic is out of scope for this task (see commit b37f83b8).
