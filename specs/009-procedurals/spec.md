# Feature: Procedurals In Rust

## Problem

An ɴsɪ procedural is code the renderer runs to contribute to the scene:
a dynamic library exporting `NSIProceduralLoad`, handed a context to
make ɴsɪ calls on (`nsi.pdf` §2.7). Writing one in Rust today means
writing the C side by hand -- the entry point, the descriptor, the
function pointers, reading `NSIParam_t` arrays -- and then finding that
`Context::from(NSIContext)` calls `NSIEnd` on the renderer's own
context when it drops.

`nsi-procedural` is the crate for this, as `nsi-display` is for display
drivers. It is a `cargo new` stub.

There are two ways a procedural reaches a renderer, and the author
should not have to care which:

- **Standalone**, as a `cdylib` any ɴsɪ renderer loads through the C
  entry point. Rust has no stable ABI, so a library loaded at runtime
  can only be spoken to through C.
- **Compiled in**, linked at build time into a renderer written against
  the `Nsi` trait -- one built on `nsi-intermediate`, say. There the
  calls should stay Rust: no C strings, no parameter marshalling.

## User Stories

1. **As a procedural author**, I implement one trait and invoke one
   macro, and get a library 3Delight loads with `NSIEvaluate` or a
   `procedural` node.
2. **As a procedural author**, I read the parameters the renderer
   passes -- any type, arrays included -- without touching a raw
   pointer.
3. **As a renderer author** linking a procedural in, I call it with my
   own `Nsi` implementation and no C in between.
4. **As a procedural author**, a panic or an error in my code is
   reported through the renderer, not an abort of the render.

## Acceptance Criteria

- A `Procedural` trait: constructed once per load, executed any number
  of times with an `Nsi` implementation and the parameters, dropped on
  unload.
- `declare_procedural!` exports `NSIProceduralLoad` with the ABI of
  `nsi_procedural.h`, and each `execute` makes its calls on the context
  it was handed -- never the one from load (§2.7.2).
- The calls go through the ɴsɪ implementation named by
  `nsi_library_path`, not whichever library this process would load by
  default (§2.7.1).
- `nsi-ffi-wrap` can wrap a context the renderer owns without ending
  it.
- Parameters are read losslessly: every `NSIType_t`, arrays, and
  `arraylength`.
- Errors and panics from the author's code reach the renderer's error
  reporting; neither unwinds into C.
- Proven against 3Delight: a built procedural is evaluated, runs, reads
  its parameters, and its calls reach the renderer.

## Non-Goals

- **C procedurals in a Rust renderer.** The procedural would open
  `nsi_library_path` for the ɴsɪ C API, so the renderer would have to
  ship one routing back into its Rust `Nsi` implementation.
  `FfiApiAdapter` is half of that; the exported library is the other
  half, and it is its own feature.
- **Loading Rust procedurals as Rust at runtime.** No stable ABI.
- **Executing procedurals in `nsi-intermediate`.** It records scenes;
  it does not run code. A renderer on top of it does.

## Risks

- A wrapped context that runs `NSIEnd` on drop ends the renderer's
  context mid-render. The wrapper has to make that unrepresentable, not
  merely documented.
- Callbacks handed to the renderer from inside a procedural outlive the
  call; the wrapped context cannot free them when it drops.
- `link_lib3delight` resolves ɴsɪ at build time and refuses any other
  library, so a procedural built with it cannot honour
  `nsi_library_path`.
