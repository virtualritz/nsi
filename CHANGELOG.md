# Changelog

## 0.9.0

### `nsi-trait` (new crate)

- Pure-Rust trait crate, no FFI deps. Defines:
  - The canonical `Nsi` trait — `self` _is_ the context (no separate handle
    type). One instance per context; constructors are implementation-specific
    and destruction goes through `Drop`.
  - The `Attribute<T>` typed-name machinery with phantom `T` describing the
    data shape the attribute accepts. `Parameter<T>` is a public alias for
    the function-arg role (same underlying type, different naming for
    site-of-use clarity).
  - Geometric type aliases: `Point2F32`, `Point3F32`, `Point4F32`,
    `Vector2F32`, `Vector3F32`, `Normal3F32`, `Color3F32`, `Color4F32`,
    `Matrix3F32`, `Matrix4F32`, `Matrix4F64` — fixed-size arrays so attribute
    slice lengths are divisible-by-component-count at the type level.
  - Standard attribute / parameter constants. Rust identifiers are derived
    from the new ɴsɪ naming convention; the wire-side string literals each
    constant points to currently still hold the **old** wire names so the
    constants work against today's renderers.
  - Optional `.ustr()` / `.as_c_ptr()` methods on `Attribute<T>` (gated on
    the `ustr` feature) — interned, null-terminated identifiers ready for
    direct C-FFI use, no per-call `CString` dance.
  - The `ParamValue` trait (Rust value → `NSIParam_t` at the FFI boundary),
    formerly named `Parameter` — renamed to free up the `Parameter<T>`
    constant role.

### `nsi-ffi-wrap` (renamed from `nsi-core`)

- The crate is now structured as the FFI wrapper for the canonical `Nsi`
  trait. The local FFI-shape `NsiBackend` trait that briefly lived in
  `nsi-ffi-wrap::ffi_api` (and shadowed the external `::nsi_trait` crate)
  is removed.
- `FfiApiAdapter` is rewritten as a generic-over-`T: Nsi` adapter that takes
  a factory closure. Each `NSIBegin` from C constructs a fresh `T`, stored
  under a generated integer ID; subsequent calls dispatch through the trait
  methods of the looked-up instance; `NSIEnd` removes the entry. Attribute
  transfer through the adapter is zero-copy: `&[Arg<'a, 'a>]` flows straight
  into the trait method.
- All 21 parameter macros (`f32!`, `f64!`, `i32!`, `i64!`, `*_slice!`,
  `color!`, `point!`, `vector!`, `normal!`, `matrix_f32!`, `matrix_f64!`,
  `string!`, `string_slice!`, …) gain a typed-name arm: the first argument
  may be either a string literal (escape hatch) or an `Attribute<T>` /
  `Parameter<T>` constant (compile-time type-checked). Wrong-shaped data is
  rejected at the call site.
- The crate's internal `pub(crate) enum Type` is renamed to `DataType` to
  end the ambiguous-glob-reexport with `nsi_trait::Type`.
- Re-exports `nsi_trait::*` so `nsi_ffi_wrap::Nsi`, `nsi_ffi_wrap::POSITION`,
  `nsi_ffi_wrap::Point3F32`, etc. all resolve without an explicit
  `nsi-trait` dependency.
- 3DelightNSI 2.9.199: `nurbs` node support, with a new `examples/nurbs/`
  illustrating `position`, `weighted-position`, the `u/v` count/order/knot
  family, and the full `trim-curves.*` group.
- Internal `Type` variants renamed to Rust style (`Float` → `F32`,
  `Integer` → `I32`, `MatrixF32` etc.).
- Callbacks passed to a context are now **owned and freed by that
  context** instead of being leaked. Previously every `CallbackPtr::to_ptr`
  did `Box::into_raw` and nothing ever reclaimed it, so each callback --
  and everything its closure captured, typically an `Arc` to the caller's
  pixel buffer -- was pinned for the life of the process. A consumer that
  re-set callbacks on a long-lived context leaked a set every time.
  `CallbackPtr` gains a `drop_ptr` companion to `to_ptr` (both
  `#[doc(hidden)]`; implement it if you implement the trait by hand). A
  callback displaced by a later `set_attribute` is retired rather than
  freed on the spot, and reclaimed at the next `Stop`/`Wait` or when the
  context drops -- a render may still be holding the old pointer. See the
  ownership section on `Context`.
- **Breaking:** `Arg::array_len` takes a `NonZeroUsize` instead of a `usize`.
  The renderer is handed `data.len() / array_length` elements, so a zero
  array length divided by zero at the FFI boundary. It is now unrepresentable
  rather than checked. Call sites pass
  `const { NonZeroUsize::new(N).unwrap() }`.

### `nsi` (root facade crate)

- Re-exports everything from `nsi-ffi-wrap` (which in turn re-exports
  `nsi-trait`) so a typical user only needs to depend on `nsi`.
- Crate-level docs reorganised: examples come right after the intro, then
  _Crate Organization_, then _Typed Attribute Names_, then _Getting Pixels_,
  _Cargo Features_, and _Linking Style_. The doctest example now uses the
  typed `nsi::POSITION` (`Attribute<[Point3F32]>`) and a typed
  `[Point3F32; 20]` array.
- Edition 2024 across the workspace.
- `ahash` bumped 0.8.6 → 0.8.12 to compile on current toolchains.
- README is now generated from the crate's module-level docs via
  `cargo-rdme`; the example in the README stays in sync with `lib.rs`
  automatically.

### Naming convention rename (constants only; wire strings unchanged)

Rust constant identifiers are derived from the new ɴsɪ naming convention
(see the `naming-convention.md` chapter in the ɴsɪ spec). The wire-side
string literals each constant points to currently still hold the **old**
wire names; only the literals will change when the renderer ships the new
names. Public Rust API stays stable across that transition.

| Old Rust                | New Rust                  | Wire string (unchanged)  |
| ----------------------- | ------------------------- | ------------------------ |
| `FOV`                   | `FIELD_OF_VIEW`           | `"fov"`                  |
| `TRANSFORMATION_MATRIX` | `MATRIX`                  | `"transformationmatrix"` |
| `SHADER_FILENAME`       | `FILENAME`                | `"shaderfilename"`       |
| `P`                     | `POSITION`                | `"P"`                    |
| `PW`                    | `WEIGHTED_POSITION`       | `"Pw"`                   |
| `COUNT_U` / `COUNT_V`   | `U_COUNT` / `V_COUNT`     | `"nu"` / `"nv"`          |
| `ORDER_U` / `ORDER_V`   | `U_ORDER` / `V_ORDER`     | `"uorder"` / `"vorder"`  |
| `KNOT_U` / `KNOT_V`     | `U_KNOT` / `V_KNOT`       | `"uknot"` / `"vknot"`    |
| `TRIM_CURVE_*`          | `TRIM_CURVES_*` (plural)  | `"trimcurves.*"`         |
| `TRIM_CURVE_COUNT`      | `TRIM_CURVES_CURVE_COUNT` | `"trimcurves.ncurves"`   |
| `STREAM_FILE_NAME`      | `STREAM_FILENAME`         | `"streamfilename"`       |
| `STREAM_PATH_REPLACE`   | `STREAM_PATH_REPLACEMENT` | `"streampathreplace"`    |
| `ERROR_HANDLER`         | `CALLBACK_ERROR`          | `"errorhandler"`         |
| `STOPPED_CALLBACK`      | `CALLBACK_STOP`           | `"stoppedcallback"`      |
| `FACESET` (node)        | `FACE_SET`                | `"faceset"`              |

## 0.8.0

### `nsi-core`

- `Context::render_control()` now takes `nsi::Action` as fist
  parameter.
  This change was made to reflect the fact that the action cannot
  be omitted. The 2nd parameter is the familiar `Option<&ArgSlice>`.

  I.e. this:

  ```rust
  ctx.render_control(&[nsi::string!("action", "start")]);
  ```

  changes to:

  ```rust
  ctx.render_control(nsi::Action::Start, None);
  ```

- All `Arg` types now implement `Clone`.

- `Pointer` & `Pointers` have been deprecated. You should be able to do
  everything via `Reference` & `Refererences`.

- The following types are now `Send` & `Sync`:
  - Callbacks with static lifetimes (`Callback<'static>`).

  - References with static lifetimes (`Reference<'static>`).

  - `Strings`

## 0.7.0

### `nsi-core`

- `Context` is now `Send`, `Sync`, `Copy` & `Clone`.

- All `Context` methods that have optional arguments now take `Option<&ArgSlice>` (instead of `&ArgSlice`).

  I.e. this:

  ```rust
  let ctx = nsi::Context::new(&[]).unwrap();
  ```

  changes to:

  ```rust
  let ctx = nsi::Context::new(None).unwrap();
  ```
