use colorspace as cs;
use exr::prelude::rgba_image::*;
use nsi::output::Error;
use png;
use polyhedron_ops as p_ops;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};

mod render;

fn write_exr(name: &str, width: usize, height: usize, pixel_length: usize, pixel_data: &[f32]) {
    let sample = |position: Vec2<usize>| {
        let index = pixel_length * (position.x() + position.y() * width);

        Pixel::rgba(
            pixel_data[index + 0],
            pixel_data[index + 1],
            pixel_data[index + 2],
            pixel_data[index + 3],
        )
    };

    let image_info = ImageInfo::rgba((width, height), SampleType::F32);

    image_info
        .write_pixels_to_file(
            name.clone(),
            // This will actually suck the pixels from our buffer in
            // parallel on all cores.
            write_options::high(),
            &sample,
        )
        .unwrap();
}

pub fn main() {
    let mut dimensions = (0u32, 0u32);
    let quantized_pixel_data = Arc::new(Mutex::new(Vec::new()));

    {
        let open = nsi::output::OpenCallback::new(
            |_name: &str, width: usize, height: usize, format: &mut nsi::output::PixelFormat| {
                // Reserve out
                let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();
                *quantized_pixel_data = vec![0u8; width * height * format.len()];
                Error::None
            },
        );

        // Source and target spaces.
        let model_aces_cg = &cs::color_space_rgb::model_f32::ACES_CG;
        let model_srgb = &cs::color_space_rgb::model_f32::SRGB;

        let write = nsi::output::WriteCallback::new(
            |_name: &str,
             width: usize,
             _height: usize,
             x_min: usize,
             x_max_plus_one: usize,
             y_min: usize,
             y_max_plus_one: usize,
             pixel_format: &[String],
             pixel_data: &[f32]| {
                let mut quantized_pixel_data = quantized_pixel_data.lock().unwrap();

                for scanline in y_min..y_max_plus_one {
                    let y_offset = scanline * width;
                    for index in y_offset + x_min..y_offset + x_max_plus_one {
                        let index = index * pixel_format.len();

                        let alpha = pixel_data[index + 3];
                        // We ignore pixels with zero alpha.
                        if 0.0 != alpha {
                            let mut color = [cs::rgb::RGBf::new(0f32, 0., 0.)];
                            cs::rgb_to_rgb(
                                model_aces_cg,
                                model_srgb,
                                &[cs::rgb::RGBf::new(
                                    pixel_data[index + 0] / alpha,
                                    pixel_data[index + 1] / alpha,
                                    pixel_data[index + 2] / alpha,
                                )],
                                &mut color,
                            );

                            let color: cs::rgb::RGBu8 = model_srgb
                                .encode(cs::rgb::RGBf::new(color[0].r, color[0].g, color[0].b))
                                .into();

                            quantized_pixel_data[index + 0] = (color.r as f32 * alpha) as _;
                            quantized_pixel_data[index + 1] = (color.g as f32 * alpha) as _;
                            quantized_pixel_data[index + 2] = (color.b as f32 * alpha) as _;
                            quantized_pixel_data[index + 3] = (alpha * 255.0) as _;
                        }
                    }
                }

                Error::None
            },
        );

        let finish = nsi::output::FinishCallback::new(
            |name: &str,
             width: usize,
             height: usize,
             pixel_format: Vec<String>,
             pixel_data: Vec<f32>| {
                write_exr(
                    (String::from(name) + ".exr").as_str(),
                    width,
                    height,
                    pixel_format.len(),
                    &pixel_data,
                );
                // Remember the dimensions for writingb out our 8bit PNG
                dimensions = (width as _, height as _);
                Error::None
            },
        );

        let mut polyhedron = p_ops::Polyhedron::tetrahedron();
        polyhedron.meta(None, None, None, false, true);
        polyhedron.normalize();
        polyhedron.gyro(Some(1. / 3.), Some(0.1), true);
        polyhedron.normalize();
        polyhedron.kis(Some(-0.2), None, true, true);
        polyhedron.normalize();

        render::nsi_render(&polyhedron, 1, false, open, write, finish);
    }

    let quantized_pixel_data = Arc::<_>::try_unwrap(quantized_pixel_data)
        .unwrap()
        .into_inner()
        .unwrap();

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
