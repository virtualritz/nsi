#![cfg_attr(feature = "nightly", doc(cfg(feature = "jupyter")))]
//! # Jupyter Notebook Support
//!
//! This module adds an
//! [`as_jupyter_notebook()`](crate::Context::as_jupyter_notebook())
//! method to a [`Context`](crate::Context).
//!
//! A [`Screen`](crate::context::NodeType::Screen) can be rendered
//! directly inside a notebook.
//!
//! Documentation on how to use Rust with Jupyter Notebooks is
//! [here](https://github.com/google/evcxr/blob/master/evcxr_jupyter/README.md).
use crate as nsi;
use crate::{argument::ArgSlice, output::PixelFormat};
use evcxr_runtime;
use image;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
// FIXME: implement this for Context instead of the single method
// below.
trait _Jupyter<'a> {
    fn camera_as_jupyter_notebook(camera: &str, args: &ArgSlice<'_, 'a>);
    fn screen_as_jupyter_notebook(screen: &str, args: &ArgSlice<'_, 'a>);
    fn output_layer_as_jupyter_notebook(
        output_layer: &str,
        args: &ArgSlice<'_, 'a>,
    );
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
    /// ctx.as_jupyter_notebook("screen");
    /// ```
    /// # Arguments
    /// * `screen` – A [`Screen`](crate::context::NodeType::Screen).
    pub fn as_jupyter_notebook(&self, screen: &str) {
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

        // Our buffer to hold quantized u8 pixel data.
        let quantized_pixel_data = Arc::new(Mutex::new(Vec::new()));

        let pixel_data_width = Arc::new(Mutex::new(0usize));
        let pixel_data_height = Arc::new(Mutex::new(0usize));

        // Callback to collect our pixels.
        let finish = nsi::output::FinishCallback::new(
            |_name: &str,
             width: usize,
             height: usize,
             pixel_format: PixelFormat,
             pixel_data: Vec<f32>| {
                assert!(4 <= pixel_format.len());

                // FIXME
                // 1. For each Layer in a PixelFormat, generate an
                //    image and put in some Vec<Image>.
                // 2. Afterwards, send all images as a matrix of
                //    PNGs to Jupyter.
                // 3. Image can be F Fa RGB RGBa or FFFF
                let mut quantized_pixel_data_unlocked =
                    quantized_pixel_data.lock().unwrap();
                *quantized_pixel_data_unlocked = vec![0u16; width * height * 4];

                buffer_rgba_f32_to_rgba_u16_be(
                    width,
                    height,
                    pixel_format.len(),
                    &pixel_data,
                    &quantized_pixel_data,
                );

                let mut pixel_data_width = pixel_data_width.lock().unwrap();
                *pixel_data_width = width;
                let mut pixel_data_height = pixel_data_height.lock().unwrap();
                *pixel_data_height = height;

                nsi::output::Error::None
            },
        );

        // Setup an output driver.
        self.create("jupyter_driver", nsi::NodeType::OutputDriver, &[]);
        self.connect(
            "jupyter_driver",
            "",
            "jupyter_beauty",
            "outputdrivers",
            &[],
        );

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
        self.render_control(&[nsi::string!("action", "wait")]);

        // Make our Context pristine again.
        self.delete("jupyter_beauty", &[nsi::integer!("recursive", 1)]);

        let width = *pixel_data_width.lock().unwrap();
        let height = *pixel_data_height.lock().unwrap();

        assert!(0 != width && 0 != height);

        let mut buffer = Vec::new();
        let quantized_pixel_data = quantized_pixel_data.lock().unwrap();
        image::png::PngEncoder::new(&mut buffer)
            .encode(
                unsafe {
                    &*core::ptr::slice_from_raw_parts(
                        quantized_pixel_data.as_ptr() as *const u8,
                        2 * quantized_pixel_data.len(),
                    )
                },
                width as _,
                height as _,
                image::ColorType::Rgba16,
            )
            .unwrap();

        evcxr_runtime::mime_type("image/png").text(&base64::encode(&buffer));
    }
}

/// Linear to (0..1 clamped) sRGB conversion – bad choice but cheap.
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

/// Multi-threaded color profile application & quantization to 8bit.
fn buffer_rgba_f32_to_rgba_u16_be(
    width: usize,
    height: usize,
    pixel_size: usize,
    pixel_data: &[f32],
    quantized_pixel_data: &Arc<Mutex<Vec<u16>>>,
) {
    let one = std::u16::MAX as f32;

    (0..height).into_par_iter().for_each(|scanline| {
        let y_offset = scanline * width * pixel_size;
        for index in
            (y_offset..y_offset + width * pixel_size).step_by(pixel_size)
        {
            let alpha = pixel_data[index + 3];
            // We ignore pixels with zero alpha.
            if 0.0 != alpha {
                // FIXME: add dithering.
                let r: u16 =
                    (linear_to_srgb(pixel_data[index + 0] / alpha) * one) as _;
                let g: u16 =
                    (linear_to_srgb(pixel_data[index + 1] / alpha) * one) as _;
                let b: u16 =
                    (linear_to_srgb(pixel_data[index + 2] / alpha) * one) as _;
                let a: u16 = (alpha * one) as _;

                let mut quantized_pixel_data =
                    quantized_pixel_data.lock().unwrap();

                #[cfg(target_endian = "little")]
                {
                    quantized_pixel_data[index + 0] = r.to_be();
                    quantized_pixel_data[index + 1] = g.to_be();
                    quantized_pixel_data[index + 2] = b.to_be();
                    quantized_pixel_data[index + 3] = a.to_be();
                }
                #[cfg(target_endian = "big")]
                {
                    quantized_pixel_data[index + 0] = r;
                    quantized_pixel_data[index + 1] = g;
                    quantized_pixel_data[index + 2] = b;
                    quantized_pixel_data[index + 3] = a;
                }
            }
        }
    });
}

/// Multi-threaded color profile application & quantization to 16bit.
fn _buffer_rgb_f32_to_rgb_u16_be(
    width: usize,
    height: usize,
    pixel_size: usize,
    pixel_data: &[f32],
    quantized_pixel_data: &Arc<Mutex<Vec<u16>>>,
) {
    let one = std::u16::MAX as f32;

    (0..height).into_par_iter().for_each(|scanline| {
        let y_offset = scanline * width * pixel_size;
        for index in
            (y_offset..y_offset + width * pixel_size).step_by(pixel_size)
        {
            // FIXME: add dithering.
            let r: u16 = (linear_to_srgb(pixel_data[index + 0]) * one) as _;
            let g: u16 = (linear_to_srgb(pixel_data[index + 1]) * one) as _;
            let b: u16 = (linear_to_srgb(pixel_data[index + 2]) * one) as _;

            let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();

            #[cfg(target_endian = "little")]
            {
                quantized_pixel_data[index + 0] = r.to_be();
                quantized_pixel_data[index + 1] = g.to_be();
                quantized_pixel_data[index + 2] = b.to_be();
            }
            #[cfg(target_endian = "big")]
            {
                quantized_pixel_data[index + 0] = r;
                quantized_pixel_data[index + 1] = g;
                quantized_pixel_data[index + 2] = b;
            }
        }
    });
}

/// Multi-threaded color profile application & quantization to 16bit.
fn _buffer_fa_f32_to_fa_u16_be(
    width: usize,
    height: usize,
    pixel_size: usize,
    pixel_data: &[f32],
    quantized_pixel_data: &Arc<Mutex<Vec<u16>>>,
) {
    let one = std::u16::MAX as f32;

    (0..height).into_par_iter().for_each(|scanline| {
        let y_offset = scanline * width * pixel_size;
        for index in
            (y_offset..y_offset + width * pixel_size).step_by(pixel_size)
        {
            let alpha = pixel_data[index + 1];
            // We ignore pixels with zero alpha.
            if 0.0 != alpha {
                // FIXME: add dithering.
                let f: u16 = ((pixel_data[index + 0] / alpha) * one) as _;
                let a: u16 = (alpha * one) as _;

                let mut quantized_pixel_data =
                    quantized_pixel_data.lock().unwrap();

                #[cfg(target_endian = "little")]
                {
                    quantized_pixel_data[index + 0] = f.to_be();
                    quantized_pixel_data[index + 3] = a.to_be();
                }
                #[cfg(target_endian = "big")]
                {
                    quantized_pixel_data[index + 0] = r;
                    quantized_pixel_data[index + 3] = a;
                }
            }
        }
    });
}

/// Multi-threaded color profile application & quantization to 16bit.
fn _buffer_f32_to_u16_be(
    width: usize,
    height: usize,
    pixel_size: usize,
    pixel_data: &[f32],
    quantized_pixel_data: &Arc<Mutex<Vec<u16>>>,
) {
    let one = std::u16::MAX as f32;

    (0..height).into_par_iter().for_each(|scanline| {
        let y_offset = scanline * width * pixel_size;
        for index in
            (y_offset..y_offset + width * pixel_size).step_by(pixel_size)
        {
            // FIXME: add dithering.
            let f: u16 = (pixel_data[index + 0] * one) as _;

            let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();

            #[cfg(target_endian = "little")]
            {
                quantized_pixel_data[index + 0] = f.to_be();
            }
            #[cfg(target_endian = "big")]
            {
                quantized_pixel_data[index + 0] = f;
            }
        }
    });
}
