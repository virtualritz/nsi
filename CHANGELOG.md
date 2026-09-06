# Changelog

## Unreleased

### `nsi-trait` 0.3.0 -> 0.4.0 (breaking)

- The `where Self: 'call` bound is dropped from the `Nsi::Arg` GAT. An
  implementor whose `Arg<'call>` borrows from `Self` would have relied
  on it; none does, and keeping it made `impl Nsi for Context<'a>`
  unprovable. Breaking for any implementor that repeated the bound,
  hence the minor bump.

### `nsi-ffi-wrap` 0.9.0 -> 0.10.0 (breaking)

- `impl ParamValue for Arg` and `impl Nsi for Context`, which is what
  lets a generic ɴsɪ consumer drive a live context, a recorder or a
  parser through one trait.
- Depends on `nsi-trait 0.4`, which is in its public API, so this is
  breaking too.

**These two bumps are the release chain.** The crates.io copies still
carry the old code under `0.3.0` and `0.9.0`, so `nsi-intermediate` and
`nsi-parse` cannot be published until they are released, in the order
trait -> ffi-wrap -> intermediate -> parse.

### `nsi-parse` (new crate)

- Reads ɴsɪ scenes and drives a `nsi_trait::Nsi` sink, so the same
  parser feeds a live renderer context, an `nsi-intermediate`
  `Recorder`, or a backend's own implementation. It produces no scene
  type of its own; one would force every consumer to translate.
- `parse_stream` reads the `.nsi` stream. ɴsɪ publishes no grammar for
  it, so the rules were read off the renderer: the format is
  **keyword-terminated, not line-based** -- an entire scene on one line
  parses, so the newlines and indents a renderer writes are formatting
  rather than syntax.
- `parse_compressed` detects gzip and zstd from the input's leading
  bytes (`gzip`, `zstd` features).
- `run_lua` reads a Lua scene by **running** it (`lua` feature). ɴsɪ's
  Lua front end is a programming language -- a script may compute the
  scene it describes -- so an interpreter is the only correct reader,
  and that is a different trust decision from parsing a data file.
- Errors name the byte offset of the offending token and what was
  expected. A sink's own refusal is carried rather than stringified, and
  the sink keeps the statements applied before the failure.
- Gated against the renderer, not against this workspace: one test
  parses what a real `apistream` context wrote, with grouped attributes
  and wrapped continuation lines that our own writer never produces.

### `nsi-intermediate`

- **Resolution refuses the scenes that have no single answer** rather
  than returning a plausible wrong one: more than one parent, a cycle, a
  node not connected to `.root`, an instancing prototype asked for a
  world transform, and a motion-sampled transform asked for at a time it
  has no sample at.
- Attributes are **gathered along the whole path**, `.root` included,
  and every `attributes` node on it is kept -- ɴsɪ says they "will all
  be considered". The previous winner-take-all resolution silently
  dropped a shader whenever visibility sat on a nearer node.
- Motion-sampled transforms resolve: `motion_times`,
  `world_transform_at`, `world_transform_samples` answer at a recorded
  sample, and `world_transform_interpolated_at` answers between them.
  Element-wise is the renderer's own model, not an approximation:
  interpolating a transformed point gives `((1-a)M₀ + aM₁)p`, and
  3Delight's rotation blur fits component-wise (rms 0.002) far better
  than slerp (0.021). Outside the sampled range the end sample is held,
  as 3Delight holds it.
- Deforming geometry resolves: `attribute_times` and
  `attribute_samples` give the sample times of any attribute, so a mesh
  whose `P` moves under a static transform is no longer reported static.
- Instancing is usable: `relative_transform` for a prototype's subtree;
  `instance_transforms` pairing each matrix with the prototype it draws
  through `modelindices`; `instance_transforms_at` for an instancer
  whose matrices, `modelindices` or `disabledinstances` are *sampled* --
  which `instance_transforms` refuses for sampled *matrices* rather than
  reporting an empty list, since an empty list reads as "nothing to
  draw" for something 3Delight renders. Sampled `modelindices` and
  `disabledinstances` are not time-varying at all -- 3Delight applies
  the last one *defined* for the whole shutter, as an overwriting
  `SetAttribute` would -- so those are read, not refused. This crate
  applies the last by *time*, which agrees for any stream written in
  time order and is recorded as an `Open` divergence otherwise. Before this
  they were ignored, reporting every instance enabled and drawn from
  model 0.
- ɴsɪ's lightweight instancing resolves per path: `placements` and
  `placements_at` give one placement per way a geometry is placed, each
  with the transform *and* the binding along that path -- rendered,
  `visibility 1` on one parent and `visibility 0` on another draws one
  copy, so the paths gather different attributes.
- Resolution along a path: `attribute_value_along` and
  `shader_attribute_value_along` apply ɴsɪ's full precedence to one
  placement, which the geometry-taking forms cannot do for a
  multi-parent node.
- `ATTR.priority` selects between definitions -- and a node carrying
  **only** `ATTR.priority` is one of them. 3Delight reads it as defining
  `ATTR` at its ɴsɪ default: rendered, such a node two levels up beats a
  `visibility 0` on the `attributes` node attached to the primitive
  itself, so this crate was returning a wrong winner and not merely a
  missing one. `AttributeValue::arg` is therefore an `Option` -- `None`
  meaning *the default of `AttributeValue::name`*, never *undefined* --
  and `name` carries the attribute that won, which used to be read off
  `arg.name`. The defaults themselves are not carried here: they are a
  renderer's to know, and a stale table would be worse than none.
- **An attribute resolves by the order it was set in**, which is what
  3Delight does and what `time_attrs` -- sorted by time -- cannot say.
  Rendered, with `visibility` set only through `SetAttributeAtTime`:
  `t=1 -> 0` then `t=0 -> 1` leaves the object visible, and `t=1 -> 1`
  then `t=0 -> 0` hides it. The same two times, opposite answers, and
  reading the greatest time gets both backwards. `Node::sample_order`
  records the times of each attribute in call order; `Node::effective`
  takes the last, `sampled_attr` walks them -- so an unreadable sample
  discards what was defined *before* it rather than what sits earlier
  on the timeline -- and both emitters replay them in that order, so a
  round trip through the writer no longer hands back a scene that
  resolves differently from the one it wrote.
- The record is a **call log** -- `Node::samples`, every
  `set_attribute_at_time` call per attribute in the order it arrived --
  rather than a table keyed by time. ɴsɪ's rules are stated over calls,
  so a table keyed by time cannot express them: it cannot say which
  call was last, how far back an unreadable sample reaches, or what a
  re-set at a time already recorded displaced. That last one is the
  difference between `good` replacing `good`, which sweeps, and `good`
  replacing an unreadable sample, which draws static -- rendered, and
  the crate now answers both. All three motion-sample divergences are
  closed, and the three tables that had to agree are one field that
  cannot disagree with itself.
- `delete` honours `recursive`, with both of ɴsɪ's exceptions.
- Node and connection identity follow ɴsɪ: a repeated `connect` updates
  rather than duplicates, re-`create` with a different type is an error,
  connections and attributes on unknown handles are refused, `.all`
  works in all four `disconnect` positions, and the reserved handles
  need no `create` and cannot be deleted.
- Classification names every `<connection>` attribute the specification
  declares -- five were missing, so an exporter using a lens shader or a
  background layer could not record at all -- and **carries** any other
  destination rather than refusing it. ɴsɪ's set is open: its §4.8
  connects a node to another's `visibility`. A carried connection is
  never resolved, so it cannot become a material by accident; the cost
  is that a typo now does nothing quietly instead of failing loudly.
- Replay was corrected against the renderer in several places that were
  silently wrong: strings are escaped -- control bytes as three-digit
  octal, which is what the renderer writes -- doubles are `%.17g` while
  floats take the shorter of decimal and exponent form, argument flags are
  letter prefixes inside the type name, an array is marked by a flag so
  `array_len(1)` is real, an empty slice is `[ ]`, a pointer argument's
  parameter line is omitted, and the reserved handles are never
  declared.
- New output formats behind features: `write_lua` (`lua`), and
  `write_stream_with` with `Compression` (`gzip`, `zstd`). **Only gzip
  is a format 3Delight reads**; zstd is for consumers of this workspace.
- Resolution is linear in the scene rather than quadratic: an adjacency
  index, which required `Scene`'s fields to become private. Read through
  `nodes()`, `node()`, `edges()`, `edges_from()`, `edges_to()` and
  `edges_to_attr()`; `Recorder::into_scene` hands the scene over without
  copying it.


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
