# Research: Procedurals In Rust

## D1: the renderer's library, not ours

`NSIProceduralLoad` receives `nsi_library_path`, "the path to the nsi
implementation that is loading the procedural", so the procedural can
make its calls through the same one (`nsi.pdf` §2.7.1). A handle is
only meaningful to the library that issued it. The shim keeps the path
from load and every `execute` builds its `Context` against it, through
the same cache `Context::new`'s `"renderer"` argument uses -- a path is
taken as a library to load.

## D2: `From<NSIContext>` was the trap

`nsi-ffi-wrap` could already build a `Context` from a bare handle, and
that `Context` runs `NSIEnd` when it drops: a procedural using it would
end the renderer's context on return. `render_status` works around the
same thing with `mem::forget`. A flag on the private inner context,
settable only by the new constructor, makes the ending behaviour a
property of how the `Context` was made rather than something a caller
has to remember.

## D3: callbacks through a borrowed context leak

A `Context` frees the callbacks passed through it after `NSIEnd`, the
first moment the renderer cannot reach them. A borrowed context has no
such moment: the renderer's context outlives it by an unknown amount.
Freeing on drop would be a use-after-free in the renderer; leaking is
the only sound choice without a signal from the renderer, and is
documented on the constructor.

## D4: `NSIProcedural_t` defined here

`nsi-sys` binds it as an opaque 24-byte blob -- bindgen cannot lay it
out because the header forward-declares it before the function-pointer
typedefs that use it. The crate defines it `#[repr(C)]` and a test
pins its size to the blob's.

## D5: parameters as a view, not a conversion

`c_api::marshal_params_to_args` turns `NSIParam_t` into `Arg`, but
handles only single scalars and strings; colors, points, matrices,
arrays and every multi-value parameter vanish without a word. Rather
than grow a copy, `Params` reads the array where it lies, typed by the
parameter's own tag, and a Rust renderer produces the same array from
its `Arg`s with `as_c_param`, which every `Arg` supports.

## D6: `execute` is generic, not `dyn`

`Nsi` has a generic associated type, so it is not object-safe, and the
author's code has to be monomorphised for the context it runs on. A
generic method is exactly that; the macro instantiates it for
`nsi_ffi_wrap::Context`, a Rust renderer for its own type.

## D7: no sphere node

ɴsɪ has no sphere primitive; a `particles` node with a `width` is one
sphere per point. The example uses that.
