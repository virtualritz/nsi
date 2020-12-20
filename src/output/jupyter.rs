#![cfg_attr(feature = "nightly", doc(cfg(feature = "jupyter")))]
//! # Jupyter Notebook Support
//!
//! This module adds a [`as_jupyter()`](crate::Context::as_jupyter())
//! method to a [`Context`](crate::Context).
//!
//! This allows visualizing a camera inside a notebook.
use crate as nsi;
use crate::{output::PixelFormat, ArgSlice};
use evcxr_runtime;
use image;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

pub trait Jupyter<'a> {
    fn camera_as_jupyter(camera: &str, args: &ArgSlice<'_, 'a>);
    fn screen_as_jupyter(screen: &str, args: &ArgSlice<'_, 'a>);
    fn output_layer_as_jupyter(output_layer: &str, args: &ArgSlice<'_, 'a>);
}

impl<'a> Jupyter<'a> for nsi::Context<'a> {
    fn camera_as_jupyter(_camera: &str, _args: &ArgSlice<'_, 'a>) {}
    fn screen_as_jupyter(_screen: &str, _args: &ArgSlice<'_, 'a>) {}
    fn output_layer_as_jupyter(_output_layer: &str, _args: &ArgSlice<'_, 'a>) {}
}

impl<'a> nsi::Context<'a> {
    /// Render the image `camera` sees into the current Jupyter Notebook.
    ///
    /// The [`Context`](crate::Context) is unchanged after this returns.
    ///
    /// # Example
    /// ```
    /// // Setup a screen.
    /// ctx.create("screen", nsi::NodeType::Screen, &[]);
    /// ctx.connect("screen", "", "camera", "screens", &[]);
    /// ctx.set_attribute(
    ///     "screen",
    ///     &[
    ///         // Some 2:1 wide angle view.
    ///         nsi::integers!("resolution", &[1280, 640]).array_len(2),
    ///         nsi::integer!("oversampling", 32),
    ///     ],
    /// );
    ///
    /// // Put a mini 16:9 image of what "camera" sees into our notebook.
    /// ctx.as_jupyter("screen");
    /// ```
    /// # Arguments
    /// * `camera` – A [`PerspectiveCamera`](crate::context::NodeType::PerspectiveCamera)
    /// * `width`, `height` – Iage dimensions.
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

        // Our buffer to hold quantized u8 pixel data.
        let quantized_pixel_data = Arc::new(Mutex::new(Vec::new()));

        let pixel_data_width = Arc::new(Mutex::new(0usize));
        let pixel_data_height = Arc::new(Mutex::new(0usize));

        // Our callback to collect pixels.
        let finish = nsi::output::FinishCallback::new(
            |_name: &str,
             width: usize,
             height: usize,
             pixel_format: PixelFormat,
             pixel_data: Vec<f32>| {
                // FIXME
                // 1. Find unique formats + no. of channels for each
                // 2. for each unique format generate an image and
                //    put in some vec<Image>
                // 3. Image can be F Fa RGB RGBa or FFFF

                assert!(4 <= pixel_format.len());
                {
                    let mut quantized_pixel_data =
                        quantized_pixel_data.lock().unwrap();
                    *quantized_pixel_data = vec![0u16; width * height * 4];
                }
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
                *pixel_data_width.lock().unwrap() as _,
                *pixel_data_height.lock().unwrap() as _,
                image::ColorType::Rgba16,
            )
            .unwrap();

        //evcxr_runtime::mime_type("image/png").bytes(&buffer);
        evcxr_runtime::mime_type("image/png").text(&base64::encode(&buffer));
        //println!("EVCXR_BEGIN_CONTENT image/png");
        //println!("{}", base64::encode(&buffer));
        //println!("EVCXR_END_CONTENT");
    }
}

/// Linear to (0..1 clamped) sRGB conversion – bad choice but cheap.
#[inline]
pub fn linear_to_srgb(x: f32) -> f32 {
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
fn buffer_rgb_f32_to_rgb_u16_be(
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
fn buffer_fa_f32_to_fa_u16_be(
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
fn buffer_f32_to_u16_be(
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
