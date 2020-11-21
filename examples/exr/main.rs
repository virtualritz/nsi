use exr::prelude::rgba_image::*;
use polyhedron_ops as p_ops;
use nsi::output::Error;
use std::sync::{Arc, Mutex};

mod render;


fn write_exr(name: &str, width: usize, height: usize, pixel_length: usize, pixel_data: &[f32]) {
    //println!("Writing EXR ... {:?}", pixel_data);

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

    // write it to a file with all cores in parallel
    image_info
        //.with_encoding(encoding)
        //.remove_excess()
        .write_pixels_to_file(
            name.clone(),
            // this will actually generate the pixels in parallel on all cores
            write_options::high(),
            &sample,
        )
        .unwrap();
}

pub fn main() {
    let mut width = 0usize;
    let mut height = 0usize;
    let mut pixel_len = 0usize;

    let mut final_pixel_data = Vec::<f32>::new(); // = Arc::new(Mutex::new(Vec::<f32>::new()));

    let index = Arc::new(Mutex::new(Vec::new()));

    {
        let open = nsi::output::OpenCallback::new(
            |_name: &str, w: usize, h: usize, format: &mut Vec<&str>| {
                width = w;
                height = h;
                pixel_len = format.len();
                let mut index = index.lock().unwrap();
                *index = (0..format.len()).map(|i| i).collect::<Vec<usize>>();
                Error::None
            },
        );

        let _write = nsi::output::WriteCallback::new(
            |_name: &str,
             _width: usize,
             _height: usize,
             _x_min: usize,
             _x_max_plus_one: usize,
             _y_min: usize,
             _y_max_plus_one: usize,
             _pixel_format: &[&str],
             _pixel_data: &mut [f32]| { Error::None },
        );

        let finish = nsi::output::FinishCallback::new(
            |name: &str, width: usize, height: usize, pixel_format: Vec<&str>, pixel_data: Vec<f32>| {
                write_exr(
                    (String::from(name) + ".exr").as_str(),
                    width,
                    height,
                    pixel_format.len(),
                    &pixel_data,
                );
                final_pixel_data = pixel_data;
                println!("{:?}", index.lock().unwrap());
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

        //inspect_boxed_trait_object(&open);
        render::nsi_render(&polyhedron, &[0.0f64; 16], 1, false, open, finish);
    }

    final_pixel_data.push(0.0);
}
