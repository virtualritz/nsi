# `nsi-procedural`

<!-- cargo-rdme start -->

Write [ɴsɪ](https://nsi.readthedocs.io/) procedurals in safe Rust.

A procedural is code the renderer runs to contribute to the scene: a
dynamic library exporting `NSIProceduralLoad`, handed a context to
make ɴsɪ calls on. This crate turns that into a trait:

1. Implement `Procedural` for a type of your own.
2. Invoke `declare_procedural!` once, at the crate root, to export
   the entry point.
3. Build the crate as a `cdylib`.

## A complete procedural

```rust
use nsi_ffi_wrap as nsi;
use nsi_procedural::{Error, Params, Procedural, Report};

struct Spheres;

impl Procedural for Spheres {
    fn load(_: &Report<'_>, _: &str) -> Result<Self, Error> {
        Ok(Spheres)
    }

    fn execute<N>(
        &self,
        nsi: &N,
        _: &Report<'_>,
        params: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
    {
        let count = params.get("count").and_then(|p| p.i32()).unwrap_or(1);
        for index in 0..count {
            let handle = format!("sphere{index}");
            nsi.create(&handle, nsi::PARTICLES, None)?;
            nsi.connect(&handle, None, nsi::ROOT, "objects", None)?;
        }
        Ok(())
    }
}

nsi_procedural::declare_procedural!(Spheres);
```

## Two ways in, one trait

**Standalone.** Built as a `cdylib`, the procedural is loaded by any
ɴsɪ renderer through the C entry point -- `NSIEvaluate` with
`"type" "dynamiclibrary"`, or a `procedural` node. Rust has no stable
ABI, so a library loaded at runtime can only be spoken to through C.
Its calls go through the ɴsɪ implementation that loaded it, which
the renderer names when it does (`nsi.pdf` §2.7.1) -- not whichever
library this process would load by default.

**Compiled in.** A renderer written against [`Nsi`](https://docs.rs/nsi-trait/latest/nsi_trait/trait.Nsi.html)
-- one built on `nsi-intermediate`, say -- links the procedural's
crate and calls it with `execute`, passing its own `Nsi`
implementation. The calls stay Rust: no C strings, no parameter
marshalling.

`Procedural::execute` is generic over the context, so the same
code serves both.

## Errors and panics

Both are reported through the renderer, at error level, and the
render goes on. Neither unwinds into C.

## `link_lib3delight`

Do not build a procedural with `nsi-ffi-wrap`'s `link_lib3delight`
feature. It resolves ɴsɪ at build time and refuses any other library,
so the procedural could not make its calls through the renderer
that loaded it.

<!-- cargo-rdme end -->
