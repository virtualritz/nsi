# Data Model: Procedurals In Rust

## What The Renderer Holds

```text
NSIProceduralLoad ──► *mut Descriptor ═══ *mut Loaded<P>
                       ┌─────────────────────────────┐
                       │ Descriptor (NSIProcedural_t)│  nsi_version, unload, execute
                       │ library: String             │  nsi_library_path, from load
                       │ procedural: P               │  the author's value
                       └─────────────────────────────┘
```

`#[repr(C)]` with the descriptor first, so the renderer's pointer is the
allocation's. `unload` reclaims the box, dropping `P`.

## Per `execute`

| Value | Lifetime | From |
| --- | --- | --- |
| `Context<'static>` | the call | `from_renderer_context(ctx, library)`; never ends `ctx` |
| `Report` | the call | the renderer's `NSIReport_t`, bound to this call's `ctx` |
| `Params<'_>` | the call | the renderer's `NSIParam_t` array, borrowed |

## `Param` Accessors

| Accessor | Types | Length |
| --- | --- | --- |
| `f32s` | float, color, point, vector, normal, matrix | count × array length × components |
| `f64s` | double, doublematrix | same |
| `i32s` / `i64s` | int / int64 | same |
| `strings` | string | same, as `&CStr` |
| `pointers` | pointer | same |

Components: 3 for color, point, vector and normal; 16 for either
matrix; otherwise 1. Array length is `arraylength` when `IS_ARRAY` is
set and 1 otherwise.
