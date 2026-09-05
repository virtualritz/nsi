//! A display driver that writes a PNG.
//!
//! This is a straight port of `ndspy-sys/examples/r-display` onto
//! `nsi-display`. It keeps the original's behaviour -- accumulate
//! buckets into a full-frame buffer, disassociate alpha, write a PNG --
//! while dropping every unsafe mechanism the original needed to get
//! there (see the module docs on [`nsi_display::DisplayDriver`] for what
//! the shims now own).
//!
//! Build as a cdylib and give it the name the renderer looks for:
//!
//! ```text
//! cargo build --example png_driver
//! cp target/debug/examples/libpng_driver.so rust_png.dpy
//! ```
//!
//! Then render with `nsi::string!("drivername", "rust_png")`.
//!
//! # How 3Delight finds the driver
//!
//! `drivername` is not a path -- 3Delight resolves it against a search
//! path, the same mechanism it uses for shaders, textures, archives and
//! procedurals. There is no `DELIGHT_DISPLAYS` environment variable:
//! `strings` on `$DELIGHT/lib/lib3delight.so` shows an env-var override
//! (`DL_<TYPE>S_PATH`, e.g. `DL_SHADERS_PATH`, `DL_TEXTURES_PATH`,
//! `DL_ARCHIVES_PATH`, `DL_PROCEDURALS_PATH`, `DL_RESOURCES_PATH`) next
//! to every other resource kind's default search path -- except
//! `display`, which has a default (`.:$DELIGHT/displays:$DELIGHT/lib`)
//! but no corresponding `DL_DISPLAYS_PATH` string anywhere in the
//! library. The default path is searched in order, and `"."` -- the
//! renderer's current working directory at render time -- is first.
//!
//! The artefact name is plain `<drivername>.dpy` -- confirmed by
//! staging a driver under that exact name in a directory, `chdir`-ing a
//! host process into it, and observing `DspyImageOpen` get called (the
//! binary also contains a `.64.dpy` string, but that is not what gets
//! looked up here). In practice this means: run the renderer with its
//! current working directory set to wherever `rust_png.dpy` was copied
//! (renamed from `libpng_driver.so`), since `$DELIGHT/displays` and
//! `$DELIGHT/lib` are typically not writable by an unprivileged build.
//! `tests/render.rs` does exactly this.
//!
//! One more gotcha: `drivername` can collide with a format 3Delight
//! implements internally. Naming this driver `"png"` silently resolves
//! to 3Delight's *own* built-in PNG driver instead of ours -- same
//! `.png` output, but written by 3Delight (its own metadata is embedded
//! in the file), never touching this code. Hence `"rust_png"`.
use nsi_display::{Bucket, DisplayDriver, Error, Params, PixelFormat, Result};

struct Png {
    path: String,
    width: usize,
    height: usize,
    channels: usize,
    pixels: Vec<u8>,
}

impl DisplayDriver for Png {
    type Pixel = u8;

    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self> {
        let channels = format.channels();
        if channels != 4 {
            // The original assumed RGBA throughout (alpha
            // disassociation, PNG color type); so do we.
            return Err(Error::BadParameters);
        }
        Ok(Png {
            path: params
                .string("imagefilename")
                .unwrap_or("render")
                .to_owned(),
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
                    + (x - bucket.x_min()))
                    * self.channels;
                let dst = (y * self.width + x) * self.channels;
                self.pixels[dst..dst + self.channels].copy_from_slice(
                    &bucket.pixels()[src..src + self.channels],
                );
            }
        }
        Ok(())
    }

    fn close(self) -> Result<()> {
        let mut pixels = self.pixels;
        // PNG needs disassociated alpha.
        for pixel in pixels.chunks_mut(4) {
            let alpha = pixel[3];
            if alpha != 0 {
                for c in pixel[..3].iter_mut() {
                    let channel = *c as u32;
                    // channel * 256 / alpha
                    *c = (((channel << 8) - channel) / alpha as u32) as u8;
                }
            }
        }

        let file = std::fs::File::create(format!("{}.png", self.path))
            .map_err(|_| Error::NoResource)?;
        let writer = std::io::BufWriter::new(file);
        let mut encoder =
            png::Encoder::new(writer, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer =
            encoder.write_header().map_err(|_| Error::Undefined)?;
        writer
            .write_image_data(&pixels)
            .map_err(|_| Error::Undefined)?;
        Ok(())
    }
}

nsi_display::declare_display_driver!(Png);
