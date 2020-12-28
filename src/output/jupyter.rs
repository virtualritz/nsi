#![cfg_attr(feature = "nightly", doc(cfg(feature = "jupyter")))]
//! # Jupyter Notebook Support
//!
//! This module adds an
//! [`as_jupyter()`](crate::Context::as_jupyter())
//! method to a [`Context`](crate::Context).
//!
//! A [`Screen`](crate::context::NodeType::Screen) can be rendered
//! directly inside a notebook.
//!
//! Documentation on how to use Rust with Jupyter Notebooks is
//! [here](https://github.com/google/evcxr/blob/master/evcxr_jupyter/README.md).
use crate as nsi;
use crate::{
    argument::ArgSlice,
    output::{Layer, LayerDepth, PixelFormat},
};
use rayon::prelude::*;

// FIXME: implement this for Context instead of the single method
// below.
trait _Jupyter<'a> {
    fn camera_as_jupyter(camera: &str, args: &ArgSlice<'_, 'a>);
    fn screen_as_jupyter(screen: &str, args: &ArgSlice<'_, 'a>);
    fn output_layer_as_jupyter(output_layer: &str, args: &ArgSlice<'_, 'a>);
}

impl<'a> nsi::Context<'a> {
    /// Render a [`Screen`](crate::context::NodeType::Screen) inside a
    /// Jupyter Notebook.
    ///
    /// Essentially this dumps a 16bit PNG as a BASE64 encoded binary
    /// blob to `stdout`.
    ///
    /// The [`Context`](crate::Context) is unchanged after this returns.
    /// # Example
    /// ```no_run
    /// // Setup a screen.
    /// # let ctx = nsi::Context::new(&[]).unwrap();
    /// ctx.create("screen", nsi::NodeType::Screen, &[]);
    /// ctx.connect("screen", "", "my_camera", "screens", &[]);
    /// ctx.set_attribute(
    ///     "screen",
    ///     &[
    ///         // Some 2:1 wide angle view.
    ///         nsi::integers!("resolution", &[1280, 640]).array_len(2),
    ///         // 20 antialiasing samples per pixel.
    ///         nsi::integer!("oversampling", 20),
    ///     ],
    /// );
    ///
    /// // Put an image of what "my_camera" sees into our notebook.
    /// ctx.as_jupyter("screen");
    /// ```
    /// # Arguments
    /// * `screen` – A [`Screen`](crate::context::NodeType::Screen).
    pub fn as_jupyter(&self, screen: &str) {
        // RGB layer.
        self.create("jupyter_beauty", nsi::NodeType::OutputLayer, &[]);
        self.set_attribute(
            "jupyter_beauty",
            &[
                nsi::string!("variablename", "Ci"),
                nsi::integer!("withalpha", 1),
                nsi::string!("scalarformat", "float"),
            ],
        );
        self.connect("jupyter_beauty", "", screen, "outputlayers", &[]);

        // Callback to collect our pixels.
        let finish = nsi::output::FinishCallback::new(
            |_name: &str,
             width: usize,
             height: usize,
             pixel_format: PixelFormat,
             pixel_data: Vec<f32>| {
                pixel_format.0.iter().for_each(|layer| {
                    pixel_data_to_jupyter(
                        width,
                        height,
                        &layer,
                        pixel_format.channels(),
                        &pixel_data,
                    )
                });

                nsi::output::Error::None
            },
        );

        // Setup an output driver.
        self.create("jupyter_driver", nsi::NodeType::OutputDriver, &[]);
        self.connect("jupyter_driver", "", "jupyter_beauty", "outputdrivers", &[]);

        self.set_attribute(
            "jupyter_driver",
            &[
                nsi::string!("drivername", nsi::output::FERRIS),
                nsi::string!("imagefilename", "jupyter"),
                nsi::callback!("callback.finish", finish),
            ],
        );

        // And now, render it!
        self.render_control(&[nsi::string!("action", "start")]);
        // Block until render is finished.
        self.render_control(&[nsi::string!("action", "wait")]);

        // Make our Context pristine again.
        self.delete("jupyter_beauty", &[nsi::integer!("recursive", 1)]);
    }
}

/// Multi-threaded color profile application & quantization to 8bit.
fn pixel_data_to_jupyter(
    width: usize,
    height: usize,
    layer: &Layer,
    channels: usize,
    pixel_data: &[f32],
) {
    let one = std::u16::MAX as f32;
    let offset = layer.offset();

    png_to_jupyter(
        width,
        height,
        layer,
        bytemuck::cast_slice(
            &(match layer.depth() {
                LayerDepth::OneChannel => {
                    (0..width * height * channels)
                        .into_par_iter()
                        .step_by(channels)
                        .map(|index| {
                            // FIXME: add dithering.
                            let v: u16 = (pixel_data[index + offset] * one) as _;

                            #[cfg(target_endian = "little")]
                            {
                                v.to_be()
                            }
                            #[cfg(target_endian = "big")]
                            {
                                v
                            }
                        })
                        .collect()
                }
                LayerDepth::OneChannelAndAlpha => (0..width * height * channels)
                    .into_par_iter()
                    .step_by(channels)
                    .flat_map(|index| {
                        let index = index + offset;

                        let alpha = pixel_data[index + 1];

                        let v: u16 = (pixel_data[index] / alpha * one) as _;
                        let a: u16 = (alpha * one) as _;

                        #[cfg(target_endian = "little")]
                        {
                            vec![v.to_be(), a.to_be()]
                        }
                        #[cfg(target_endian = "big")]
                        {
                            vec![v, a]
                        }
                    })
                    .collect(),
                LayerDepth::Color => {
                    (0..width * height * channels)
                        .into_par_iter()
                        .step_by(channels)
                        .flat_map(|index| {
                            let index = index + offset;
                            // FIXME: add dithering.
                            let r: u16 = (linear_to_srgb(pixel_data[index]) * one) as _;
                            let g: u16 = (linear_to_srgb(pixel_data[index + 1]) * one) as _;
                            let b: u16 = (linear_to_srgb(pixel_data[index + 2]) * one) as _;

                            #[cfg(target_endian = "little")]
                            {
                                vec![r.to_be(), g.to_be(), b.to_be()]
                            }
                            #[cfg(target_endian = "big")]
                            {
                                vec![r, g, b]
                            }
                        })
                        .collect()
                }
                LayerDepth::Vector => {
                    (0..width * height * channels)
                        .into_par_iter()
                        .step_by(channels)
                        .flat_map(|index| {
                            let index = index + offset;
                            // FIXME: add dithering.
                            let r: u16 = (normalize(pixel_data[index]) * one) as _;
                            let g: u16 = (normalize(pixel_data[index + 1]) * one) as _;
                            let b: u16 = (normalize(pixel_data[index + 2]) * one) as _;

                            #[cfg(target_endian = "little")]
                            {
                                vec![r.to_be(), g.to_be(), b.to_be()]
                            }
                            #[cfg(target_endian = "big")]
                            {
                                vec![r, g, b]
                            }
                        })
                        .collect()
                }
                LayerDepth::ColorAndAlpha => {
                    (0..width * height * channels)
                        .into_par_iter()
                        .step_by(channels)
                        .flat_map(|index| {
                            let index = index + offset;
                            let alpha = pixel_data[index + 3];
                            // We ignore pixels with zero alpha.
                            if 0.0 != alpha {
                                // FIXME: add dithering.
                                let r: u16 = (linear_to_srgb(pixel_data[index] / alpha) * one) as _;
                                let g: u16 =
                                    (linear_to_srgb(pixel_data[index + 1] / alpha) * one) as _;
                                let b: u16 =
                                    (linear_to_srgb(pixel_data[index + 2] / alpha) * one) as _;
                                let a: u16 = (alpha * one) as _;

                                #[cfg(target_endian = "little")]
                                {
                                    vec![r.to_be(), g.to_be(), b.to_be(), a.to_be()]
                                }
                                #[cfg(target_endian = "big")]
                                {
                                    vec![r, g, b, a]
                                }
                            } else {
                                vec![0; 4]
                            }
                        })
                        .collect()
                }
                LayerDepth::VectorAndAlpha => {
                    (0..width * height * channels)
                        .into_par_iter()
                        .step_by(channels)
                        .flat_map(|index| {
                            let index = index + offset;
                            let alpha = pixel_data[index + 3];
                            // We ignore pixels with zero alpha.
                            if 0.0 != alpha {
                                // FIXME: add dithering.
                                let r: u16 = (normalize(pixel_data[index] / alpha) * one) as _;
                                let g: u16 = (normalize(pixel_data[index + 1] / alpha) * one) as _;
                                let b: u16 = (normalize(pixel_data[index + 2] / alpha) * one) as _;
                                let a: u16 = (alpha * one) as _;

                                #[cfg(target_endian = "little")]
                                {
                                    vec![r.to_be(), g.to_be(), b.to_be(), a.to_be()]
                                }
                                #[cfg(target_endian = "big")]
                                {
                                    vec![r, g, b, a]
                                }
                            } else {
                                vec![0; 4]
                            }
                        })
                        .collect()
                }
                _ => Vec::new(),
            }),
        ),
    );
}

// Linear to (0..1 clamped) sRGB conversion – cheesy but cheap.
// FIXME: implement a proper 'filmic' tonemapper instead.
#[inline]
fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else if x < 0.0031308 {
        x * 12.92
    } else {
        x.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

// Normalize a value from -1..1 to 0..1.
// Used to map vector data to colors.
fn normalize(x: f32) -> f32 {
    0.5 + x * 0.5
}

fn png_to_jupyter(width: usize, height: usize, layer: &Layer, data: &[u8]) {
    if LayerDepth::FourChannels == layer.depth()
        || LayerDepth::FourChannelsAndAlpha == layer.depth()
    {
        return;
    }

    let mut buffer = Vec::new();
    let mut png_encoder = png::Encoder::new(&mut buffer, width as _, height as _);
    png_encoder.set_color(match layer.depth() {
        LayerDepth::OneChannel => png::ColorType::Grayscale,
        LayerDepth::OneChannelAndAlpha => png::ColorType::GrayscaleAlpha,
        LayerDepth::Color | LayerDepth::Vector => png::ColorType::RGB,
        LayerDepth::ColorAndAlpha | LayerDepth::VectorAndAlpha => png::ColorType::RGBA,
        _ => unreachable!(),
    });
    png_encoder.set_depth(png::BitDepth::Sixteen);
    png_encoder
        .write_header()
        .unwrap()
        .write_image_data(data)
        .unwrap();

    //let mut temp_png = std::fs::File::create("/tmp/jupyter.png").unwrap();
    //temp_png.write_all(&buffer).unwrap();

    evcxr_runtime::mime_type("image/png").text(&base64::encode(&buffer));
}
