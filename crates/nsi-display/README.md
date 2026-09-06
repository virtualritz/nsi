# `nsi-display`

<!-- cargo-rdme start -->

Write ɴsɪ display drivers in safe Rust.

3Delight's display drivers are shared libraries the renderer loads
and calls into through four C entry points (`DspyImageOpen`,
`DspyImageData`, `DspyImageClose`, `DspyImageQuery` -- the "ndspy"
ABI). This crate turns that into a safe trait:

1. Implement `DisplayDriver` for a type of your own.
2. Invoke `declare_display_driver!` once, at the crate root, to
   export the four symbols.
3. Build the crate as a `cdylib`.

## A complete driver

```rust
use nsi_display::{Bucket, DisplayDriver, Error, Params, PixelFormat, Result};

struct Ppm {
    path: String,
    width: usize,
    height: usize,
    channels: usize,
    pixels: Vec<u8>,
}

impl DisplayDriver for Ppm {
    type Pixel = u8;

    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self> {
        let channels = format.channels();
        Ok(Ppm {
            path: params.string("imagefilename").unwrap_or("render").to_owned(),
            width,
            height,
            channels,
            pixels: vec![0u8; width * height * channels],
        })
    }

    fn write(&mut self, bucket: Bucket<'_, u8>) -> Result<()> {
        for y in bucket.y_min()..bucket.y_max() {
            for x in bucket.x_min()..bucket.x_max() {
                let src = ((y - bucket.y_min()) * bucket.width()
                    + (x - bucket.x_min())) * self.channels;
                let dst = (y * self.width + x) * self.channels;
                self.pixels[dst..dst + self.channels]
                    .copy_from_slice(&bucket.pixels()[src..src + self.channels]);
            }
        }
        Ok(())
    }

    fn close(self) -> Result<()> {
        let file = std::fs::File::create(format!("{}.ppm", self.path))
            .map_err(|_| Error::NoResource)?;
        // ... write a PPM header and `self.pixels` ...
        Ok(())
    }
}

nsi_display::declare_display_driver!(Ppm);
```

## How the renderer finds your driver

This is the single most important practical thing about running a
driver, and it is documented nowhere in 3Delight's own materials --
not `nsi.pdf`, not `3delight.config`.

`drivername` (the NSI attribute you set on the `outputdriver` node)
is **not a path**. 3Delight resolves it against a search path, the
same mechanism it uses for shaders, textures, archives and
procedurals. The default display search path is
`.:$DELIGHT/displays:$DELIGHT/lib`, searched in that order, with
`.` -- the renderer's current working directory at render time --
searched *first*.

Unlike every other resource kind, there is **no way to override
this path**. Shaders, textures, archives, procedurals and generic
resources each have a `DL_<TYPE>S_PATH` environment variable
(`DL_SHADERS_PATH`, `DL_TEXTURES_PATH`, `DL_ARCHIVES_PATH`,
`DL_PROCEDURALS_PATH`, `DL_RESOURCES_PATH`); `display` has none.
`3delight.config` documents seven keys, none of them
display-related. `nsi.pdf`'s index of the `.global` node's
attributes has no search-path attribute of any kind. So there are
exactly two places a driver can go: the renderer's working
directory, or `$DELIGHT/displays` (often not writable by an
unprivileged build).

The artefact must be named `<drivername>.dpy`. Cargo produces
`libfoo.so` (or `.dll`/`.dylib`), so it has to be renamed or
symlinked before the renderer can find it.

**The trap:** `drivername` can collide with a format 3Delight
implements internally. Naming a driver `"png"` silently resolves to
3Delight's own built-in PNG driver (`dspy_png` internally) instead
of yours -- you get a valid, correct-looking file, written by
entirely the wrong code path, with no error of any kind. Pick a
name unlikely to collide. If you need certainty, use the
negative-control pattern in `tests/render.rs`: render once with the
driver *not* staged and confirm nothing gets written, then stage it
and render again.

## What the shims give you

`declare_display_driver!` generates the four `extern "C"`
functions; each forwards, through a thin shim, to your
`DisplayDriver` implementation. The shims are what make the trait
safe to implement:

- A panic in your code is caught and converted to an ndspy error
  code rather than unwinding into the renderer's C stack.
- The image handle is heap-allocated once in `open` and reclaimed
  exactly once in `close`; there is no way to leak it or free it
  twice from safe code.
- Every pointer the renderer hands you (parameters, pixel data) is
  borrowed for the duration of that one call, never stored past it.
- `Params` checks the ndspy type tag before reading a value, so
  asking for the wrong type returns `None` instead of reinterpreting
  bytes.

`write` takes `&mut self`, and the shims answer `PkThreadQuery` with
`multithread = 0`, so the renderer serialises buckets -- this crate
does not support concurrent bucket delivery.

## Scope

This crate does not implement `DspyImageReopen`,
`DspyImageActiveRegion` or `DspyImageDelayClose`. If your driver
needs any of those, this crate isn't (yet) for you.

## Dependencies

`declare_display_driver!` expands to code that refers to
`::ndspy_sys` directly, so any crate invoking it must itself depend
on `ndspy-sys`.

<!-- cargo-rdme end -->
