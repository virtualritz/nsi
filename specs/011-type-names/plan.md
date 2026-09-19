# Plan: Type Names From Role And Machine Type, And `hpoint`

## Approach

1. **`nsi-sys`** binds the 2.9.210 `nsi.h`: `NSIType::HPoint` = 10.
2. **`nsi-trait`**: `Type`'s variants renamed, `Point4F32` added, the
   old names kept as `#[deprecated]` associated constants -- which also
   work as patterns, so old `match` arms compile.
3. **`nsi-ffi-wrap`**: the private `DataType` follows `Type`; the
   argument wrappers take the new names in `nsi::argument`, next to the
   shapes of the same names at the root; `ArgData`'s variants follow.
   The old wrapper names are deprecated type aliases at the root, the
   old macros deprecated forwarders to the new ones. `Point4F32Slice`
   and the new `Point4F32` send `NSITypeHPoint`.
4. Every other crate is rewritten to the new names by pattern, then by
   the compiler: each exhaustive `match` on `Type` gains `Point4F32`,
   and a `_ =>` arm that would silently count an `hpoint` as one scalar
   is found by searching for the groups it belongs with.
5. **`nsi-parse`** reads `hpoint` (stream) and `nsi.TypeHPoint` (Lua);
   **`nsi-intermediate`** stores it and writes it back.

## Gates

1. Clippy `-D warnings` on every crate: a leftover old name is a
   deprecation warning, so the gate proves the rewrite is complete.
2. Deprecated names still compile: a test uses each old form and
   allows the warning.
3. `hpoint` round-trips: stream in, recorded, written, parsed again.
4. **The fix is rendered**: a rational patch sent from Rust with
   `point4_f32_slice!` renders in 3Delight 2.9.210, where the old flat
   floats were refused with `E6007`.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/type-names.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`
