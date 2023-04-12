use exr::prelude::*;
use nsi_core as nsi;
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

pub fn main() {
    let quantized_pixel_data = Arc::new(Mutex::new(Vec::new()));

    // Open closure.
    // Called before the renderer will send any pixels via
    // WriteCallback.
    // If you decide to write data directly into a file in
    // WriteCallback.
    let open = nsi::output::OpenCallback::new(
        |_name: &str,
         width: usize,
         height: usize,
         format: &nsi::output::PixelFormat| {
            let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();
            // Create a properly size buffer to receive our pixel data.
            *quantized_pixel_data =
                vec![0u8; width * height * format.channels()];
            nsi::output::Error::None
        },
    );

    // Write closure.
    // Called for each bucket or scanline of pixels that have been
    // rendered.
    // Bucket size is commonly 16x16 pixels but this is not guaranteed
    // by the API.
    // The pixel_data will contain a full buffer of all the pixel that
    // were finished so far.
    let write = nsi::output::WriteCallback::new(
        |_name: &str,
         width: usize,
         _height: usize,
         x_min: usize,
         x_max_plus_one: usize,
         y_min: usize,
         y_max_plus_one: usize,
         pixel_format: &nsi::output::PixelFormat,
         pixel_data: &[f32]| {
            let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();

            for scanline in y_min..y_max_plus_one {
                let y_offset = scanline * width;

                for index in y_offset + x_min..y_offset + x_max_plus_one {
                    let index = index * pixel_format.channels();
                    let alpha = pixel_data[index + 3];

                    // Ignore pixels with zero alpha.
                    if 0.0 != alpha {
                        // Unpremultiply the color – this is needed
                        // or else the color profile transform will
                        // yield wrong results for pixels with
                        // non-opaque alpha. Furthermore PNG wants
                        // unpremultiplied pixels and that is what
                        // we will write the 8bit data to, at the end.
                        quantized_pixel_data[index + 0] =
                            (linear_to_srgb(pixel_data[index + 0] / alpha)
                                * 255.0) as _;
                        quantized_pixel_data[index + 1] =
                            (linear_to_srgb(pixel_data[index + 1] / alpha)
                                * 255.0) as _;
                        quantized_pixel_data[index + 2] =
                            (linear_to_srgb(pixel_data[index + 2] / alpha)
                                * 255.0) as _;
                        quantized_pixel_data[index + 3] = (alpha * 255.0) as _;
                    }
                }
            }

            nsi::output::Error::None
        },
    );

    // We need to remember the pixel buffer dimensions to write the
    // PNG out below.
    let mut dimensions = (0u32, 0u32);

    // Finish closure.
    // Called when the all the pixels have been sent via WriteCallback.
    let finish = nsi::output::FinishCallback::new(
        |name: String,
         width: usize,
         height: usize,
         pixel_format: nsi::output::PixelFormat,
         pixel_data: Vec<f32>| {
            let sample = |x: usize, y: usize| {
                let index = pixel_format.channels() * (x + y * width);
                // We just assume the 1st four channels are rgba as we set
                // this up like so above. In real world code you would
                // probably branch depending on pixel_format[n].depth()
                // (and do so outside this closure, ofc).
                (
                    pixel_data[index + 0],
                    pixel_data[index + 1],
                    pixel_data[index + 2],
                    pixel_data[index + 3],
                )
            };

            // We write the raw f32 data out as an OpenEXR.
            write_rgba_f32_file(name + ".exr", Vec2(width, height), &sample)
                .unwrap();

            // Remember the dimensions for writingb out our 8bit PNG below.
            dimensions = (width as _, height as _);
            nsi::output::Error::None
        },
    );

    // Create some geometry.
    let mut polyhedron = p_ops::Polyhedron::tetrahedron();
    polyhedron.meta(None, None, None, None, true);
    polyhedron.normalize();
    polyhedron.gyro(Some(1. / 3.), Some(0.1), true);
    polyhedron.normalize();
    polyhedron.kis(Some(-0.2), None, None, true);
    polyhedron.normalize();

    // The next call blocks until the render has finished.
    nsi_render(32, &polyhedron, open, write, finish);

    // We can shed the Arc and the Mutex now that nsi_render() is done.
    let quantized_pixel_data = Arc::<_>::try_unwrap(quantized_pixel_data)
        .unwrap()
        .into_inner()
        .unwrap();

    // Write out the display-referred, u8 quantized data we prepared
    // in the write closure above as a PNG.
    let path = Path::new("test.png");
    let file = File::create(path).unwrap();
    let ref mut writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, dimensions.0, dimensions.1);
    encoder.set_color(png::ColorType::RGBA);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&quantized_pixel_data)
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
