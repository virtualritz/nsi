# `nsi-display-exr`

[![Documentation](https://docs.rs/nsi-display-exr/badge.svg)](https://docs.rs/nsi-display-exr)
[![Crate](https://img.shields.io/crates/v/nsi-display-exr.svg)](https://crates.io/crates/nsi-display-exr)

<!-- cargo-rdme start -->

An ɴsɪ OpenEXR display driver, with optional OIDN denoising.

A worked example of `nsi-display` that is meant to grow into a
production driver, which is why it is a crate of its own rather than
an example inside `nsi-display`: it has real dependencies (OpenEXR
and OIDN) that no other user of `nsi-display` should have to build.

Build it and give the artefact the name the renderer looks for:

```text
cargo build -p nsi-display-exr --features denoise
cp target/debug/libnsi_display_exr.so rust_exr.dpy
```

## Denoising is optional

`denoise` is off by default, because `oidn`'s build script needs
OpenImageDenoise present -- through `OIDN_DIR` or `pkg-config` --
and *panics* when it is not. docs.rs has neither, and no network for
`oidn`'s `bundled` route either, so a default-on feature would mean
this crate publishes with no documentation at all. Writing EXRs is
the core; denoising is the extra that needs a system library.

With the feature off the `denoise` attribute is still accepted, and
reported on `stderr` rather than ignored -- a scene that asked for a
denoised render should not quietly receive an undenoised one.

With it on, set `OIDN_DIR` to an OpenImageDenoise install; this
crate's `build.rs` turns that into an rpath, so the `.dpy` finds
OIDN when the *renderer* loads it, without cargo's environment. Do
not reach for `LD_LIBRARY_PATH` instead: an OIDN directory may also
hold its own TBB, and `mold` loads TBB itself, so putting one on
`LD_LIBRARY_PATH` makes the linker pick up the wrong TBB and die on
an undefined symbol.

## Why denoise here at all

3Delight applies OIDN to *interactive* renders only. A batch render
writes its file undenoised, so a driver that denoises on the way out
is the place to get it -- which is what this one does, in `close()`,
once the last bucket has landed and a full frame exists.

## Attributes

3Delight's built-in EXR driver takes `exrcompression`,
`exrlineorder` and `exrheader_<name>`. Those names predate the ɴsɪ
[naming convention](https://nsi.readthedocs.io/en/latest/naming-convention.html),
and this driver follows the convention instead:

| 3Delight          | Here               | Why |
|-------------------|--------------------|-----|
| `exrcompression`  | `compression`      | The node type already says this is an EXR driver, so the name must not repeat it (rule 4). |
| `exrlineorder`    | `line-order`       | Same, plus hyphens between words -- "no concatenated words" (rule 6). |
| `exrheader_<name>`| `header.<name>`     | A dot separates hierarchy levels; two or more related attributes justify the group (rules 1, 2). |
| --                | `denoise`          | Off by default. |
| --                | `denoise.quality`  | Two related attributes, so the dot group is earned (rule 2). |

- `compression` -- `none`, `rle`, `zips`, `zip`, `piz`, `pxr24`,
  `b44`, `b44a`, `dwaa` or `dwab`. Default `zips`. All ten are
  verified to reach the written file's header, not merely to return
  `Ok`; see `tests/render.rs`.

  `dwaa` and `dwab` need **`exr` 1.74.2 or newer** -- that is the
  release which added the `dwa` module, and so the first that can
  write them. Earlier 1.x accepts the enum variants and then fails
  at write time with "yet unimplemented compression method", which
  would surface only once a render had finished. The manifest
  requires that minimum rather than `"1"` for exactly this reason.
- `line-order` -- `increasing`, `decreasing`, `any`. Default
  `increasing`.
- `header.<name>` -- any string attribute, written into the EXR
  header under `<name>`.

The output layer's `colorprofile` reaches the driver too, when the
scene sets one, and is recorded in the header under that name.
OpenEXR has no standard attribute for it and this driver does not
transform pixels the renderer already wrote, so it is metadata, not
a conversion.
- `denoise` -- `1` to denoise the beauty layer through OIDN.
- `denoise.quality` -- `default`, `fast`, `balanced` or `high`.
- `denoise.albedo` -- the name of the output layer to take OIDN's
  albedo input from. Defaults to `albedo`, which is what 3Delight
  uses: 14 of its shaders emit that AOV via
  `outputconstant("albedo")`.
- `denoise.normal` -- likewise for the normal, defaulting to `N`,
  3Delight's built-in.
- `denoise.depth` -- likewise for depth, defaulting to `z`. Reported
  when missing, but not fed to the filter: OIDN's `RayTracing` has
  no depth input.

## Utility passes

OIDN's ray-tracing filter takes an **albedo** and a **normal**
alongside the beauty, and is materially worse without them. Rather
than guess which output layers those are, the driver is told:
`denoise.albedo` and `denoise.normal` name them, defaulting to
3Delight's own `albedo` and `N`. It warns on `stderr` at `open()` --
before the render, while the answer is still useful -- naming the
attribute, the layer it was told to look for, and the layers that
were actually connected.

The names are the renderer's own, not guesses off the channel list.
3Delight describes its output layers in the parameter array
positionally -- a `layer` index, then that layer's `variablename`,
`layername` and `layertype` -- and this driver reads those. It has
to: channel names cannot identify a layer, because built-in
variables arrive with no layer name at all. Measured from a real
four-layer render:

```text
["r", "g", "b", "a",                                 <- Ci, bare
 "albedo.000.r", "albedo.001.g", "albedo.002.b",     <- prefixed
 "N.000.x", "N.001.y", "N.002.z",                    <- prefixed
 "z"]                                                <- depth, bare
```

Only the custom AOV layers carry their `layername`. Reading the
declared layers instead means depth is identifiable too, and a
`layername` that differs from the variable it outputs is honoured.

<!-- cargo-rdme end -->

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE),
[MIT license](LICENSE-MIT) or [Zlib license](LICENSE-ZLIB) at your option.
