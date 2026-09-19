# Data Model: Type Names

| `nsi.h` | Stream | `Type` | Wrapper (`nsi::argument`) | Macro | Shape (root) |
| --- | --- | --- | --- | --- | --- |
| `NSITypeFloat` | `float` | `RealF32` | `RealF32`, `RealF32Slice` | `real_f32!` | `f32` |
| `NSITypeDouble` | `double` | `RealF64` | `RealF64`, `RealF64Slice` | `real_f64!` | `f64` |
| `NSITypeInteger` | `int` | `IntegerI32` | `IntegerI32`, `IntegerI32Slice` | `integer_i32!` | `i32` |
| `NSITypeInt64` | `int64` | `IntegerI64` | `IntegerI64`, `IntegerI64Slice` | `integer_i64!` | `i64` |
| `NSITypeColor` | `color` | `Color3F32` | `Color3F32`, `Color3F32Slice` | `color3_f32!` | `Color3F32` |
| `NSITypePoint` | `point` | `Point3F32` | `Point3F32`, `Point3F32Slice` | `point3_f32!` | `Point3F32` |
| `NSITypeHPoint` | `hpoint` | `Point4F32` | `Point4F32`, `Point4F32Slice` | `point4_f32!` | `Point4F32` |
| `NSITypeVector` | `vector` | `Vector3F32` | `Vector3F32`, `Vector3F32Slice` | `vector3_f32!` | `Vector3F32` |
| `NSITypeNormal` | `normal` | `Normal3F32` | `Normal3F32`, `Normal3F32Slice` | `normal3_f32!` | `Normal3F32` |
| `NSITypeMatrix` | `matrix` | `Matrix4F32` | `Matrix4F32`, `Matrix4F32Slice` | `matrix4_f32!` | `Matrix4F32` |
| `NSITypeDoubleMatrix` | `doublematrix` | `Matrix4F64` | `Matrix4F64`, `Matrix4F64Slice` | `matrix4_f64!` | `Matrix4F64` |
| `NSITypeString` | `string` | `String` | `String`, `StringSlice` (root) | `string!` | -- |
| `NSITypePointer` | `pointer` | `Reference` | `Reference`, `ReferenceSlice` (root) | `reference!` | -- |

Each macro has a `_slice` form. The old names -- `Type::F32`, `nsi::F32`,
`nsi::f32!` and so on -- are deprecated aliases of the new ones.
