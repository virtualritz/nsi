# Feature: Type Names From Role And Machine Type, And `hpoint`

## Problem

Two problems, one change.

**3Delight 2.9.210 added `NSITypeHPoint`, and the Rust crates cannot
send it.** `Point4F32Slice` sends a NURBS surface's `Pw` as 16 flat
`float`s and leaves the grouping to the renderer. Rendered with 2.9.210,
that is rejected -- `E6007 wrong type for attribute 'Pw' ... (expected
type 'hpoint', got 'float')` -- and the whole patch is then dropped
(`E6020`). The one renderer that has a `nurbs` node cannot receive a
rational one from Rust. The parser cannot read an `hpoint` either.

**The type names say different things in different ways.** `Type::F32`
names a machine type, `Type::Color` a role, `Type::MatrixF64` both.
`nsi-trait`'s shaped aliases already read the other way round --
`Color3F32`, `Point4F32`, `Matrix4F64` -- so the same library names one
type two ways. The documentation now proposes one scheme
([Type Rename](https://nsi.readthedocs.io/) in the design drafts):
the *role* in mathematical terms, then the component count, then the
machine type of one scalar.

## User Stories

1. **As a user writing a NURBS surface in Rust**, I pass `Pw` as
   `&[[f32; 4]]` and 3Delight renders it.
2. **As a user reading a stream**, a `Pw "hpoint"` parses and
   round-trips.
3. **As a user reading code**, `Type::RealF32`, `Type::Color3F32` and
   `nsi::real_f32!` say what a value is and how it is stored, the same
   way everywhere.
4. **As an existing user**, my code keeps compiling after the upgrade,
   with deprecation warnings naming the replacement.

## Acceptance Criteria

- `nsi_trait::Type` variants: `RealF32`, `RealF64`, `IntegerI32`,
  `IntegerI64`, `String`, `Color3F32`, `Point3F32`, `Vector3F32`,
  `Normal3F32`, `Matrix4F32`, `Matrix4F64`, `Reference`, and the new
  `Point4F32` = 10 for `NSITypeHPoint`.
- The old variant names are `#[deprecated]` associated constants, so
  `Type::F32` still compiles, in expressions and in patterns.
- `nsi-ffi-wrap`'s argument types and macros take the same names:
  `RealF32`/`real_f32!`, `Color3F32`/`color3_f32!`,
  `Point4F32`/`point4_f32!`, and so on, each with a slice form. The old
  types are deprecated aliases and the old macros deprecated wrappers.
- `point4_f32!`/`point4_f32_slice!` send `NSITypeHPoint`, one element
  per point, and a rational patch renders in 3Delight 2.9.210.
- `nsi-parse` reads `hpoint`; `nsi-intermediate` records it and writes
  it back as `hpoint` in a stream and `nsi.TypeHPoint` in Lua.
- `nsi-sys` binds the 2.9.210 header, which defines `NSITypeHPoint`.
- Every crate in the workspace uses the new names; clippy is clean with
  `-D warnings`, which a leftover deprecated use would fail.

## Non-Goals

- **`ArgData` variant aliases.** `ArgData`'s variants carry data, and a
  data-carrying variant cannot be aliased. They are renamed outright;
  code builds `ArgData` through `From`, which is unaffected.
- **`Reference` to `Pointer`.** The design draft leaves this open, and
  Rust named it `Reference` on purpose.
- **Renaming the stream's type spellings** (`float`, `hpoint`). They
  are the renderer's; the attribute vocabulary is spec 012.

## Risks

- **Older renderers.** `NSITypeHPoint` is 10, a value 3Delight 2.9.208
  does not know. A `Pw` sent to it is refused where the flat floats
  were -- and were refused by 2.9.210. Only 2.9.210 has `nurbs`, so no
  working scene is lost.
- **A breaking release chain.** A new `Type` variant breaks exhaustive
  matches, so `nsi-sys`, `nsi-trait`, `nsi-ffi-wrap`, `nsi` and the
  crates using them take a minor-version bump.
