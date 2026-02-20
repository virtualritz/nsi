//! Test utilities for NSI image-based testing.
//!
//! This module provides utilities for rendering test scenes and comparing
//! the output images with expected results.

use anyhow::{Context as _, Result};
use nsi_ffi_wrap as nsi;
use png;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// Standard test image resolution.
pub const TEST_IMAGE_WIDTH: usize = 320;
pub const TEST_IMAGE_HEIGHT: usize = 240;

/// Default number of shading samples for tests.
pub const DEFAULT_SAMPLES: u32 = 16;

/// Global test arguments, parsed once and reused across all tests.
static TEST_ARGS: OnceLock<TestArgs> = OnceLock::new();

/// Command line arguments for image-based tests.
#[derive(Debug, Clone)]
pub struct TestArgs {
    /// Update expected images instead of comparing.
    pub update: bool,
    /// Path to the expected images directory.
    pub expected_dir: PathBuf,
    /// Path to the output directory for test images.
    pub output_dir: PathBuf,
}

impl Default for TestArgs {
    fn default() -> Self {
        Self {
            update: false,
            expected_dir: PathBuf::from("tests/expected_images"),
            output_dir: PathBuf::from("target/test_images"),
        }
    }
}

impl TestArgs {
    /// Parse command line arguments for image-based tests.
    ///
    /// Use RUST_TEST_UPDATE=1 to update expected images.
    pub fn parse() -> Self {
        let update = env::var("RUST_TEST_UPDATE")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        Self {
            update,
            ..Default::default()
        }
    }

    /// Get the path for an expected image file.
    pub fn expected_path(&self, test_name: &str) -> PathBuf {
        self.expected_dir.join(format!("{}.png", test_name))
    }

    /// Get the path for an output image file.
    pub fn output_path(&self, test_name: &str) -> PathBuf {
        self.output_dir.join(format!("{}.png", test_name))
    }

    /// Ensure output directory exists.
    pub fn ensure_output_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.output_dir)?;
        Ok(())
    }

    /// Ensure expected directory exists.
    pub fn ensure_expected_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.expected_dir)?;
        Ok(())
    }
}

/// Result of an image comparison test.
#[derive(Debug)]
pub enum ImageTestResult {
    /// Images match within tolerance.
    Match,
    /// Expected image was created/updated.
    Updated,
    /// Images don't match.
    Mismatch { diff_percentage: f64 },
    /// Expected image doesn't exist.
    MissingExpected,
    /// Error occurred during comparison.
    Error(String),
}

/// Data captured during rendering.
#[derive(Default)]
pub struct RenderData {
    pub width: usize,
    pub height: usize,
    pub pixel_data: Vec<f32>,
    pub quantized_data: Vec<u8>,
}

impl RenderData {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixel_data: vec![0.0; width * height * 4], // RGBA
            quantized_data: vec![0; width * height * 4], // RGBA u8
        }
    }
}

/// Run a render test with the given scene setup.
pub fn run_render_test<F>(
    test_name: &str,
    scene_setup: F,
    resolution: Option<(usize, usize)>,
    samples: Option<u32>,
) -> Result<ImageTestResult>
where
    F: FnOnce(&nsi::Context),
{
    let args = test_args();
    args.ensure_output_dir()?;

    let (width, height) =
        resolution.unwrap_or((TEST_IMAGE_WIDTH, TEST_IMAGE_HEIGHT));
    let samples = samples.unwrap_or(DEFAULT_SAMPLES);

    // Render the scene
    let render_data =
        render_scene(test_name, scene_setup, width, height, samples)?;

    // Save the output image
    let output_path = args.output_path(test_name);
    save_png(&output_path, &render_data)?;

    if args.update {
        args.ensure_expected_dir()?;
        let expected_path = args.expected_path(test_name);
        fs::copy(&output_path, &expected_path)?;
        return Ok(ImageTestResult::Updated);
    }

    let expected_path = args.expected_path(test_name);
    if !expected_path.exists() {
        return Ok(ImageTestResult::MissingExpected);
    }

    // Compare images
    match compare_png_files(&output_path, &expected_path) {
        Ok(diff) => {
            if diff < 0.001 {
                // 0.1% tolerance
                Ok(ImageTestResult::Match)
            } else {
                Ok(ImageTestResult::Mismatch {
                    diff_percentage: diff * 100.0,
                })
            }
        }
        Err(e) => {
            Ok(ImageTestResult::Error(format!("Comparison failed: {}", e)))
        }
    }
}

/// Render a scene using NSI and capture the output.
fn render_scene<F>(
    test_name: &str,
    scene_setup: F,
    width: usize,
    height: usize,
    samples: u32,
) -> Result<RenderData>
where
    F: FnOnce(&nsi::Context),
{
    let render_data = Arc::new(Mutex::new(RenderData::new(width, height)));
    let render_data_clone = Arc::clone(&render_data);

    // Create callbacks - use f32 driver for floating point pixel data
    let write = nsi::output::WriteCallback::<f32>::new(
        move |_name: &str,
              width: usize,
              _height: usize,
              x_min: usize,
              x_max_plus_one: usize,
              y_min: usize,
              y_max_plus_one: usize,
              pixel_format: &nsi::output::PixelFormat,
              pixel_data: &[f32]| {
            let mut data = render_data_clone.lock().unwrap();

            // Copy pixel data
            for y in y_min..y_max_plus_one {
                for x in x_min..x_max_plus_one {
                    let src_idx = ((y - y_min) * (x_max_plus_one - x_min)
                        + (x - x_min))
                        * pixel_format.channels();
                    let dst_idx = (y * width + x) * 4; // RGBA

                    // Copy RGBA channels
                    for c in 0..4.min(pixel_format.channels()) {
                        data.pixel_data[dst_idx + c] = pixel_data[src_idx + c];
                    }

                    // Quantize to u8 with sRGB conversion
                    let alpha = if pixel_format.channels() > 3 {
                        pixel_data[src_idx + 3]
                    } else {
                        1.0
                    };

                    if alpha > 0.0 {
                        data.quantized_data[dst_idx] =
                            (linear_to_srgb(pixel_data[src_idx] / alpha)
                                * 255.0) as u8;
                        data.quantized_data[dst_idx + 1] =
                            (linear_to_srgb(pixel_data[src_idx + 1] / alpha)
                                * 255.0) as u8;
                        data.quantized_data[dst_idx + 2] =
                            (linear_to_srgb(pixel_data[src_idx + 2] / alpha)
                                * 255.0) as u8;
                        data.quantized_data[dst_idx + 3] =
                            (alpha * 255.0) as u8;
                    }
                }
            }

            nsi::output::Error::None
        },
    );

    // Create NSI context
    let ctx =
        nsi::Context::new(None).context("Could not create NSI context")?;

    // Set global rendering settings
    ctx.set_attribute(
        nsi::GLOBAL,
        &[
            nsi::integer!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "horizontal"),
            nsi::integer!("quality.shadingsamples", samples as _),
            nsi::integer!("maximumraydepth.reflection", 3),
            nsi::integer!("maximumraydepth.refraction", 3),
        ],
    );

    // Setup camera
    setup_test_camera(&ctx, width, height);

    // Setup output
    setup_test_output(&ctx, test_name, write);

    // Let the test setup the scene
    scene_setup(&ctx);

    // Render
    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);

    // Drop the context to release resources.
    drop(ctx);

    // Extract the render data.
    // Note: We use std::mem::take instead of Arc::try_unwrap because the
    // display driver callbacks may be leaked (not dropped) to prevent
    // double-free issues with the renderer. This means the Arc may still
    // have multiple strong references even after the context is dropped.
    println!("Extracting render data...");
    let data = std::mem::take(&mut *render_data.lock().unwrap());
    println!("Data extracted successfully");

    Ok(data)
}

/// Setup a standard test camera.
fn setup_test_camera(ctx: &nsi::Context, width: usize, height: usize) {
    // Camera transform
    ctx.create("camera_xform", nsi::TRANSFORM, None);
    ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "camera_xform",
        &[nsi::double_matrix!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.
            ]
        )],
    );

    // Camera
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, "camera_xform", "objects", None);
    ctx.set_attribute("camera", &[nsi::float!("fov", 35.0)]);

    // Screen
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::integers!("resolution", &[width as i32, height as i32])
            .array_len(2)],
    );
}

/// Setup test output driver.
fn setup_test_output(
    ctx: &nsi::Context,
    test_name: &str,
    write: nsi::output::WriteCallback<f32>,
) {
    // Output layer
    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::integer!("withalpha", 1),
            nsi::string!("scalarformat", "float"),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);

    // Output driver
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", nsi::output::FERRIS_F32),
            nsi::string!("imagefilename", test_name),
            nsi::callback!("callback.write", write),
        ],
    );
}

/// Save rendered image as PNG.
fn save_png(path: &Path, render_data: &RenderData) -> Result<()> {
    use std::{fs::File, io::BufWriter};

    let file = File::create(path)?;
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(
        w,
        render_data.width as u32,
        render_data.height as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&render_data.quantized_data)?;

    Ok(())
}

/// Compare two PNG files and return the average pixel difference.
fn compare_png_files(path1: &Path, path2: &Path) -> Result<f64> {
    use std::fs::File;

    let decoder1 = png::Decoder::new(File::open(path1)?);
    let mut reader1 = decoder1.read_info()?;
    let mut buf1 = vec![0; reader1.output_buffer_size()];
    reader1.next_frame(&mut buf1)?;

    let decoder2 = png::Decoder::new(File::open(path2)?);
    let mut reader2 = decoder2.read_info()?;
    let mut buf2 = vec![0; reader2.output_buffer_size()];
    reader2.next_frame(&mut buf2)?;

    if buf1.len() != buf2.len() {
        return Err(anyhow::anyhow!("Image sizes don't match"));
    }

    let total_diff: f64 = buf1
        .iter()
        .zip(buf2.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).abs() / 255.0)
        .sum();

    Ok(total_diff / buf1.len() as f64)
}

/// Get global test arguments.
pub fn test_args() -> &'static TestArgs {
    TEST_ARGS.get_or_init(|| TestArgs::parse())
}

/// Assert that a render test passes.
pub fn assert_render_test<F>(test_name: &str, scene_setup: F)
where
    F: FnOnce(&nsi::Context),
{
    assert_render_test_with_params(test_name, scene_setup, None, None);
}

/// Assert that a render test passes with custom parameters.
pub fn assert_render_test_with_params<F>(
    test_name: &str,
    scene_setup: F,
    resolution: Option<(usize, usize)>,
    samples: Option<u32>,
) where
    F: FnOnce(&nsi::Context),
{
    match run_render_test(test_name, scene_setup, resolution, samples) {
        Ok(ImageTestResult::Match) => {
            println!("✓ Render test '{}' passed", test_name);
        }
        Ok(ImageTestResult::Updated) => {
            println!("✓ Render test '{}' updated expected image", test_name);
        }
        Ok(ImageTestResult::MissingExpected) => {
            panic!(
                "❌ Render test '{}' failed: Expected image not found. Run with RUST_TEST_UPDATE=1 to create it.",
                test_name
            );
        }
        Ok(ImageTestResult::Mismatch { diff_percentage }) => {
            panic!(
                "❌ Render test '{}' failed: Images differ by {:.3}%",
                test_name, diff_percentage
            );
        }
        Ok(ImageTestResult::Error(msg)) => {
            panic!("❌ Render test '{}' failed: {}", test_name, msg);
        }
        Err(e) => {
            panic!("❌ Render test '{}' failed: {}", test_name, e);
        }
    }
}

/// Linear to sRGB conversion.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parsing() {
        let args = TestArgs::parse();
        assert_eq!(args.expected_dir, PathBuf::from("tests/expected_images"));
        assert_eq!(args.output_dir, PathBuf::from("target/test_images"));
    }

    #[test]
    fn linear_to_srgb_conversion() {
        assert_eq!(linear_to_srgb(0.0), 0.0);
        assert_eq!(linear_to_srgb(1.0), 1.0);
        assert!((linear_to_srgb(0.5) - 0.735).abs() < 0.01);
    }
}
