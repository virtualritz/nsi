use exr::prelude::*;
use nsi_ffi_wrap as nsi;
use png;
use polyhedron_ops as p_ops;
use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{Arc, Mutex},
};

mod render;
use render::*;

/// Accumulated pixel data for both EXR (f32) and PNG (u8) output.
#[derive(Debug)]
struct PixelBuffers {
    /// Linear f32 pixel data for EXR output.
    f32_data: Vec<f32>,
    /// Quantized u8 pixel data for PNG output.
    u8_data: Vec<u8>,
    /// Image width.
    width: usize,
    /// Image height.
    height: usize,
    /// Number of channels per pixel.
    channels: usize,
}

impl PixelBuffers {
    fn new() -> Self {
        Self {
            f32_data: Vec::new(),
            u8_data: Vec::new(),
            width: 0,
            height: 0,
            channels: 0,
        }
    }
}

pub fn main() {
    let pixel_buffers = Arc::new(Mutex::new(PixelBuffers::new()));

    // Open closure.
    // Called before the renderer will send any pixels via
    // WriteCallback.
    let pixel_buffers_open = Arc::clone(&pixel_buffers);
    let open = nsi::output::OpenCallback::new(
        move |_name: &str,
              width: usize,
              height: usize,
              format: &nsi::output::PixelFormat| {
            let mut buffers = pixel_buffers_open.lock().unwrap();
            let channels = format.channels();
            // Create properly sized buffers to receive our pixel data.
            buffers.f32_data = vec![0.0f32; width * height * channels];
            buffers.u8_data = vec![0u8; width * height * channels];
            buffers.width = width;
            buffers.height = height;
            buffers.channels = channels;
            nsi::output::Error::None
        },
    );

    // Write closure.
    // Called for each bucket of pixels that have been rendered.
    // With the new API, pixel_data contains ONLY the bucket data,
    // not the full image. We accumulate it ourselves.
    let pixel_buffers_write = Arc::clone(&pixel_buffers);
    let write = nsi::output::WriteCallback::<f32>::new(
        move |_name: &str,
              _full_width: usize,
              _full_height: usize,
              x_min: usize,
              x_max_plus_one: usize,
              y_min: usize,
              y_max_plus_one: usize,
              pixel_format: &nsi::output::PixelFormat,
              bucket_data: &[f32]| {
            let mut buffers = pixel_buffers_write.lock().unwrap();
            let channels = pixel_format.channels();
            let bucket_width = x_max_plus_one - x_min;
            let full_width = buffers.width;

            // Copy bucket data into the accumulated buffers.
            for bucket_y in 0..(y_max_plus_one - y_min) {
                let image_y = y_min + bucket_y;
                for bucket_x in 0..bucket_width {
                    let image_x = x_min + bucket_x;

                    // Source index in the bucket (row-major, channels interleaved)
                    let src_idx =
                        (bucket_y * bucket_width + bucket_x) * channels;
                    // Destination index in the full image
                    let dst_idx = (image_y * full_width + image_x) * channels;

                    // Copy f32 data for EXR output
                    for c in 0..channels {
                        buffers.f32_data[dst_idx + c] =
                            bucket_data[src_idx + c];
                    }

                    // Quantize to u8 with sRGB conversion for PNG output
                    let alpha = if channels > 3 {
                        bucket_data[src_idx + 3]
                    } else {
                        1.0
                    };

                    // Ignore pixels with zero alpha.
                    if alpha != 0.0 {
                        // Unpremultiply the color – this is needed
                        // or else the color profile transform will
                        // yield wrong results for pixels with
                        // non-opaque alpha. Furthermore PNG wants
                        // unpremultiplied pixels.
                        buffers.u8_data[dst_idx + 0] =
                            (linear_to_srgb(bucket_data[src_idx + 0] / alpha)
                                * 255.0) as u8;
                        buffers.u8_data[dst_idx + 1] =
                            (linear_to_srgb(bucket_data[src_idx + 1] / alpha)
                                * 255.0) as u8;
                        buffers.u8_data[dst_idx + 2] =
                            (linear_to_srgb(bucket_data[src_idx + 2] / alpha)
                                * 255.0) as u8;
                        if channels > 3 {
                            buffers.u8_data[dst_idx + 3] =
                                (alpha * 255.0) as u8;
                        }
                    }
                }
            }

            nsi::output::Error::None
        },
    );

    // We need to remember the name for writing files.
    let output_name = Arc::new(Mutex::new(String::new()));
    let output_name_finish = Arc::clone(&output_name);

    // Finish closure.
    // Called when all the pixels have been sent via WriteCallback.
    // With the new API, we don't receive pixel data here - we use
    // the data we accumulated in the write callback.
    let pixel_buffers_finish = Arc::clone(&pixel_buffers);
    let finish = nsi::output::FinishCallback::new(
        move |name: String,
              width: usize,
              height: usize,
              pixel_format: nsi::output::PixelFormat| {
            let buffers = pixel_buffers_finish.lock().unwrap();
            let channels = pixel_format.channels();

            let sample = |x: usize, y: usize| {
                let index = channels * (x + y * width);
                // We just assume the first four channels are RGBA as we set
                // this up like so above. In real world code you would
                // probably branch depending on pixel_format[n].depth()
                // (and do so outside this closure, ofc).
                (
                    buffers.f32_data[index + 0],
                    buffers.f32_data[index + 1],
                    buffers.f32_data[index + 2],
                    buffers.f32_data[index + 3],
                )
            };

            // We write the raw f32 data out as an OpenEXR.
            write_rgba_file(name.clone() + ".exr", width, height, &sample)
                .unwrap();

            // Remember the name for writing the PNG below.
            *output_name_finish.lock().unwrap() = name;

            nsi::output::Error::None
        },
    );

    // Create some geometry.
    let mut polyhedron = p_ops::Polyhedron::tetrahedron();
    polyhedron.meta(None, None, None, None, true);
    polyhedron.normalize();
    polyhedron.gyro(Some(1. / 3.), Some(0.1), true);
    polyhedron.normalize();
    polyhedron.kis(Some(-0.2), None, None, None, true);
    polyhedron.normalize();

    // The next call blocks until the render has finished.
    nsi_render(32, &polyhedron, open, write, finish);

    // Get the accumulated pixel data.
    let buffers = Arc::try_unwrap(pixel_buffers)
        .expect("Failed to unwrap pixel buffers")
        .into_inner()
        .unwrap();

    let name = Arc::try_unwrap(output_name)
        .expect("Failed to unwrap output name")
        .into_inner()
        .unwrap();

    // Write out the display-referred, u8 quantized data we prepared
    // in the write closure above as a PNG.
    let path = format!("{}.png", name);
    let file = File::create(Path::new(&path)).unwrap();
    let ref mut writer = BufWriter::new(file);

    let mut encoder =
        png::Encoder::new(writer, buffers.width as u32, buffers.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&buffers.u8_data)
        .expect("Error writing PNG.");
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
