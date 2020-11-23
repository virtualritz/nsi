#![cfg_attr(feature = "nightly", doc(cfg(feature = "jupyter")))]
//! # Jupyter Notebook Support
//!
//! This module adds a [`as_jupyter()`](crate::Context::as_jupyter())
//! method to a [`Context`](crate::Context).
//!
//! This allows visualizing a camera inside a notebook.
use crate as nsi;
use crate::ArgSlice;
use image;
use rayon::prelude::*;

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
             pixel_format: Vec<String>,
             pixel_data: Vec<f32>| {
                assert!(4 <= pixel_format.len());
                {
                    let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();
                    *quantized_pixel_data = vec![0u8; width * height * 4];
                }
                buffer_rgba_f32_to_rgba_u8(
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
        image::png::PngEncoder::new(&mut buffer)
            .encode(
                quantized_pixel_data.lock().unwrap().as_slice(),
                *pixel_data_width.lock().unwrap() as _,
                *pixel_data_height.lock().unwrap() as _,
                image::ColorType::Rgba8,
            )
            .unwrap();

        println!("EVCXR_BEGIN_CONTENT image/png");
        println!("{}", base64::encode(&buffer));
        println!("EVCXR_END_CONTENT");
    }
}


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


use std::sync::{Arc, Mutex};
/// Multi-threaded color profile application & quantization to 8bit.
fn buffer_rgba_f32_to_rgba_u8(
    width: usize,
    height: usize,
    pixel_size: usize,
    pixel_data: &[f32],
    quantized_pixel_data: &Arc<Mutex<Vec<u8>>>,
) {
    (0..height).into_par_iter().for_each(|scanline| {
        let y_offset = scanline * width * pixel_size;
        for index in
            (y_offset..y_offset + width * pixel_size).step_by(pixel_size)
        {
            let alpha = pixel_data[index + 3];
            // We ignore pixels with zero alpha.
            if 0.0 != alpha {

                let r: u8 = (linear_to_srgb(pixel_data[index + 0] / alpha) * 255.0) as _;
                let g: u8 = (linear_to_srgb(pixel_data[index + 1] / alpha) * 255.0) as _;
                let b: u8 = (linear_to_srgb(pixel_data[index + 2] / alpha) * 255.0) as _;
                let a: u8 = (alpha * 255.0) as _;

                let mut quantized_pixel_data =
                    quantized_pixel_data.lock().unwrap();

                quantized_pixel_data[index + 0] = r;
                quantized_pixel_data[index + 1] = g;
                quantized_pixel_data[index + 2] = b;
                quantized_pixel_data[index + 3] = a;
            }
        }
    });
}
