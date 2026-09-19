# Research: Type Names From Role And Machine Type, And `hpoint`

## D1: role, then components, then machine type

`IntegerI32`, `RealF32`, `Color3F32`, `Point4F32`, `Matrix4F64`. The
role is the mathematical name, as `Color` and `Normal` already were;
the suffix is the storage. `nsi-trait`'s shape aliases already read this
way, so the tag and the shape now agree. The design draft on
nsi.readthedocs.io was updated to the same names (docs c744c34).

## D2: flat floats for `Pw` are refused -- rendered

A rational NURBS patch, rendered with 3Delight 2.9.210: `"Pw" "hpoint"
4 [...]` renders; the same values as `"Pw" "float" 16 [...]` warn
`E6007 wrong type for attribute 'Pw' ... (expected type 'hpoint', got
'float')` and then `E6020 node 'patch' does not have required attribute
'P or Pw'`, and the patch is gone. `Point4F32Slice` sent exactly the
refused form, so switching it to `NSITypeHPoint` is a fix, not a
rename.

## D3: deprecation, tested before relying on it

On this toolchain (rustc 1.100 nightly) `#[deprecated]` on a
`#[macro_export] macro_rules!` warns at the call site, and so does a
`#[deprecated]` associated constant -- in a `match` pattern too. So the
old `Type::F32`, `nsi::f32!` and `nsi::Color` all keep compiling with a
warning naming the replacement.

## D4: wrappers in `nsi::argument`

The shape `nsi::Color3F32 = [f32; 3]` and the wrapper that borrows a
`&[f32; 3]` for a call would both want `Color3F32`. The shapes keep the
root, because typed attribute names (`Attribute<[Point3F32]>`) are
written against them; the wrappers live in `nsi::argument`, reached
through the macros. The crate root re-exports `argument`'s other items
explicitly, not by glob, so no name is ambiguous.

## D5: `ArgData` variants have no aliases

A variant that carries data cannot be aliased. They are renamed
outright. Code builds `ArgData` through `From`, which is unaffected;
code that matches on its variants has to change.

## D6: the silent `_ => 1`

Four `components_per_element`-style matches end in `_ => 1`. The
compiler does not flag them, and without a `Point4F32 => 4` arm they
would count an `hpoint` as one scalar -- a quarter of the data. They
were found by searching for every match grouping `Normal3F32`, not by
compiling.
