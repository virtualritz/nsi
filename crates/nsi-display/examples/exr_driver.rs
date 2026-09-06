//! An OpenEXR display driver, with optional OIDN denoising.
//!
//! Build as a cdylib and give it the name the renderer looks for:
//!
//! ```text
//! RUSTFLAGS="-C link-arg=-Wl,-rpath,$DELIGHT/lib/oidn/lib" \
//!   OIDN_DIR=$DELIGHT/lib/oidn \
//!   cargo build -p nsi-display --features exr-driver --example exr_driver
//! cp target/debug/examples/libexr_driver.so rust_exr.dpy
//! ```
//!
//! The rpath matters: the renderer `dlopen`s this driver, so OIDN has
//! to be findable without cargo's environment. Do not use
//! `LD_LIBRARY_PATH` for it -- 3Delight's OIDN directory also holds its
//! own TBB, and `mold` loads TBB itself, so the linker picks up the
//! wrong one and fails.
//!
//! Then render with `nsi::string!("drivername", "rust_exr")`. See the
//! `png_driver` example for the full writeup of how 3Delight resolves
//! `drivername` to a `.dpy` on disk -- and why the name here is
//! `rust_exr` rather than `exr`, which would silently resolve to
//! 3Delight's own built-in driver.
//!
//! # Why denoise here at all
//!
//! 3Delight applies OIDN to *interactive* renders only. A batch render
//! writes its file undenoised, so a driver that denoises on the way out
//! is the place to get it -- which is what this one does, in `close()`,
//! once the last bucket has landed and a full frame exists.
//!
//! # Attributes
//!
//! 3Delight's built-in EXR driver takes `exrcompression`,
//! `exrlineorder` and `exrheader_<name>`. Those names predate the ɴsɪ
//! [naming convention](https://nsi.readthedocs.io/en/latest/naming-convention.html),
//! and this driver follows the convention instead:
//!
//! | 3Delight          | Here               | Why |
//! |-------------------|--------------------|-----|
//! | `exrcompression`  | `compression`      | The node type already says this is an EXR driver, so the name must not repeat it (rule 4). |
//! | `exrlineorder`    | `line-order`       | Same, plus hyphens between words -- "no concatenated words" (rule 6). |
//! | `exrheader_<name>`| `header.<name>`     | A dot separates hierarchy levels; two or more related attributes justify the group (rules 1, 2). |
//! | --                | `denoise`          | Off by default. |
//! | --                | `denoise.quality`  | Two related attributes, so the dot group is earned (rule 2). |
//!
//! - `compression` -- `none`, `rle`, `zips`, `zip`, `piz`, `pxr24`,
//!   `b44`, `b44a`, `dwaa`, `dwab`. Default `zips`.
//! - `line-order` -- `increasing`, `decreasing`, `any`. Default
//!   `increasing`.
//! - `header.<name>` -- any string attribute, written into the EXR
//!   header under `<name>`.
//! - `denoise` -- `1` to denoise the beauty layer through OIDN.
//! - `denoise.quality` -- `default`, `fast`, `balanced` or `high`.
//!
//! # Utility passes
//!
//! OIDN's ray-tracing filter takes an **albedo** and a **normal**
//! alongside the beauty, and is materially worse without them. This
//! driver looks for output layers named `albedo`, `N`/`normal` and
//! `depth`/`Z`, and warns on `stderr` at `open()` -- before the render
//! rather than after it -- naming each one it did not get.
//!
//! Depth is warned about for the same reason, though note OIDN's
//! `RayTracing` filter has no depth input: it is checked because a
//! denoising setup is expected to carry it, not because this driver
//! feeds it to the filter.
use nsi_display::{Bucket, DisplayDriver, Error, Params, PixelFormat, Result};

use exr::prelude::*;

/// Where one ɴsɪ output layer sits in the flat pixel, and what it is
/// called.
struct LayerSpan {
    name: String,
    offset: usize,
    channels: usize,
}

struct Exr {
    path: String,
    width: usize,
    height: usize,
    channels: usize,
    layers: Vec<LayerSpan>,
    compression: Compression,
    line_order: LineOrder,
    /// `header.<name>` attributes, as `(name, value)`.
    header: Vec<(String, String)>,
    denoise: bool,
    denoise_quality: oidn::Quality,
    /// The whole frame, `width * height * channels` scalars.
    pixels: Vec<f32>,
}

/// The EXR channel suffixes for a layer of `channels` channels.
///
/// EXR names channels by role, so a three-channel layer is `R`,`G`,`B`
/// unless it is a normal, in which case OpenEXR's convention is still
/// `X`,`Y`,`Z`. A one-channel layer is `Z` for depth and `Y`
/// (luminance) otherwise.
fn channel_suffixes(name: &str, channels: usize) -> Vec<&'static str> {
    let is_depth = matches!(name, "depth" | "Z" | "z");
    let is_vector = matches!(name, "N" | "normal" | "Ns" | "P");
    match (channels, is_depth, is_vector) {
        (1, true, _) => vec!["Z"],
        (1, _, _) => vec!["Y"],
        (2, ..) => vec!["Y", "A"],
        (3, _, true) => vec!["X", "Y", "Z"],
        (3, ..) => vec!["R", "G", "B"],
        (4, _, true) => vec!["X", "Y", "Z", "A"],
        (4, ..) => vec!["R", "G", "B", "A"],
        // Five is `quad` with alpha; anything longer cannot occur, but
        // naming them positionally keeps the file writable.
        _ => vec!["R", "G", "B", "A", "Y"],
    }
}

impl Exr {
    /// Finds a layer by any of `names`, returning its span.
    fn layer(&self, names: &[&str]) -> Option<&LayerSpan> {
        self.layers
            .iter()
            .find(|l| names.contains(&l.name.as_str()))
    }

    /// Copies a layer's first three channels out as a tightly packed
    /// RGB buffer, which is the only shape OIDN accepts.
    fn rgb_of(&self, span: &LayerSpan) -> Vec<f32> {
        let mut out = vec![0.0f32; self.width * self.height * 3];
        for pixel in 0..self.width * self.height {
            for c in 0..3 {
                // A layer with fewer than three channels repeats its
                // last one, so a scalar reads as grey rather than as
                // two zeroes.
                let src = span.offset + c.min(span.channels - 1);
                out[pixel * 3 + c] = self.pixels[pixel * self.channels + src];
            }
        }
        out
    }
}

impl DisplayDriver for Exr {
    type Pixel = f32;

    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self> {
        let compression = match params.string("compression").unwrap_or("zips") {
            "none" => Compression::Uncompressed,
            "rle" => Compression::RLE,
            "zips" => Compression::ZIP1,
            "zip" => Compression::ZIP16,
            "piz" => Compression::PIZ,
            "pxr24" => Compression::PXR24,
            "b44" => Compression::B44,
            "b44a" => Compression::B44A,
            "dwaa" => Compression::DWAA(None),
            "dwab" => Compression::DWAB(None),
            _ => return Err(Error::BadParameters),
        };

        let line_order =
            match params.string("line-order").unwrap_or("increasing") {
                "increasing" => LineOrder::Increasing,
                "decreasing" => LineOrder::Decreasing,
                "any" => LineOrder::Unspecified,
                _ => return Err(Error::BadParameters),
            };

        let header = params
            .strings()
            .filter_map(|(name, value)| {
                name.strip_prefix("header.")
                    .map(|key| (key.to_owned(), value.to_owned()))
            })
            .collect();

        let layers: Vec<LayerSpan> = format
            .iter()
            .map(|layer| LayerSpan {
                name: layer.name().to_owned(),
                offset: layer.offset(),
                channels: layer.channels(),
            })
            .collect();

        let denoise = params.i32("denoise").unwrap_or(0) != 0;
        let denoise_quality =
            match params.string("denoise.quality").unwrap_or("default") {
                "default" => oidn::Quality::Default,
                "fast" => oidn::Quality::Fast,
                "balanced" => oidn::Quality::Balanced,
                "high" => oidn::Quality::High,
                _ => return Err(Error::BadParameters),
            };

        if denoise {
            // Warn before the render, not after it: this is the last
            // moment the answer is still useful.
            for (label, names) in [
                ("albedo", &["albedo"][..]),
                ("normal", &["N", "normal", "Ns"][..]),
                ("depth", &["depth", "Z", "z"][..]),
            ] {
                if !layers.iter().any(|l| names.contains(&l.name.as_str())) {
                    eprintln!(
                        "rust_exr: denoise is on but no `{label}` output \
                         layer was connected to this driver -- denoising \
                         will be worse without it"
                    );
                }
            }
        }

        let channels = format.channels();
        Ok(Exr {
            path: params
                .string("imagefilename")
                .unwrap_or("render")
                .to_owned(),
            width,
            height,
            channels,
            layers,
            compression,
            line_order,
            header,
            denoise,
            denoise_quality,
            pixels: vec![0.0f32; width * height * channels],
        })
    }

    fn write(&mut self, bucket: Bucket<'_, f32>) -> Result<()> {
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

    fn close(mut self) -> Result<()> {
        if self.denoise {
            self.run_denoise();
        }

        // One EXR channel per ɴsɪ channel, named `<layer>.<role>` --
        // except the beauty layer, which by convention is unprefixed.
        let mut channels = Vec::new();
        for span in &self.layers {
            let beauty = span.name == "Ci";
            let is_depth = matches!(span.name.as_str(), "depth" | "Z" | "z");
            for (c, suffix) in channel_suffixes(&span.name, span.channels)
                .iter()
                .enumerate()
            {
                if c >= span.channels {
                    break;
                }
                // The beauty is unprefixed by EXR convention, and so
                // is a lone depth channel -- `Z` is the name OpenEXR
                // gives depth, and 3Delight hands it to us unprefixed
                // anyway, so there is no layer name to prefix with.
                let name = if beauty || (is_depth && span.channels == 1) {
                    (*suffix).to_owned()
                } else {
                    format!("{}.{}", span.name, suffix)
                };
                let index = span.offset + c;
                let samples: Vec<f32> = (0..self.width * self.height)
                    .map(|pixel| self.pixels[pixel * self.channels + index])
                    .collect();
                channels.push(AnyChannel::new(
                    name.as_str(),
                    FlatSamples::F32(samples),
                ));
            }
        }

        let mut attributes = LayerAttributes::default();
        for (key, value) in &self.header {
            attributes.other.insert(
                Text::from(key.as_str()),
                AttributeValue::Text(Text::from(value.as_str())),
            );
        }

        let layer = Layer::new(
            (self.width, self.height),
            attributes,
            Encoding {
                compression: self.compression,
                blocks: Blocks::ScanLines,
                line_order: self.line_order,
            },
            AnyChannels::sort(channels.into_iter().collect()),
        );

        Image::from_layer(layer)
            .write()
            .to_file(format!("{}.exr", self.path))
            .map_err(|_| Error::NoResource)?;
        Ok(())
    }
}

impl Exr {
    /// Denoises the beauty layer in place, through OIDN's ray-tracing
    /// filter, using whichever of albedo and normal were connected.
    fn run_denoise(&mut self) {
        let Some(beauty) = self.layer(&["Ci"]) else {
            eprintln!("rust_exr: denoise is on but there is no beauty layer");
            return;
        };
        let (offset, span_channels) = (beauty.offset, beauty.channels);
        let color = self.rgb_of(beauty);
        let albedo = self.layer(&["albedo"]).map(|l| self.rgb_of(l));
        let normal = self.layer(&["N", "normal", "Ns"]).map(|l| self.rgb_of(l));

        // OIDN reports both of these fallibly -- no device (no
        // supported hardware, no driver) is the common one. A failure
        // must not lose the render, so it warns and writes the raw
        // beauty instead.
        let device = match oidn::Device::new() {
            Ok(device) => device,
            Err(error) => {
                eprintln!("rust_exr: no OIDN device, writing the raw beauty: {error}");
                return;
            }
        };
        let mut filter = match oidn::RayTracing::try_new(&device) {
            Ok(filter) => filter,
            Err(error) => {
                eprintln!("rust_exr: no OIDN filter, writing the raw beauty: {error}");
                return;
            }
        };
        filter
            .srgb(false)
            .hdr(true)
            .filter_quality(self.denoise_quality)
            .image_dimensions(self.width, self.height);
        if let (Some(albedo), Some(normal)) = (&albedo, &normal) {
            filter.albedo_normal(albedo, normal);
        } else if let Some(albedo) = &albedo {
            filter.albedo(albedo);
        }

        let mut denoised = vec![0.0f32; self.width * self.height * 3];
        if let Err(error) = filter.filter(&color, &mut denoised) {
            eprintln!("rust_exr: OIDN failed, writing the raw beauty: {error}");
            return;
        }

        // Back into the flat frame, leaving alpha and every other layer
        // untouched.
        for pixel in 0..self.width * self.height {
            for c in 0..span_channels.min(3) {
                self.pixels[pixel * self.channels + offset + c] =
                    denoised[pixel * 3 + c];
            }
        }
    }
}

nsi_display::declare_display_driver!(Exr);
