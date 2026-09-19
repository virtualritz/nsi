//! A rational NURBS patch sent from Rust renders in 3Delight 2.9.210.
//!
//! 3Delight 2.9.210 requires a `nurbs` node's `Pw` to be an `hpoint`.
//! Before `Point4F32Slice` sent `NSITypeHPoint`, it sent 16 flat floats,
//! which 3Delight refuses (`E6007`) before dropping the patch (`E6020`).
//!
//! Needs 3Delight 2.9.210 or later; renders a 64×64 PNG with the
//! built-in `png` driver.
use nsi_ffi_wrap as nsi;
use std::path::Path;

/// Renders a flat order-2 patch spanning ±1, red, and returns how many
/// pixels are red.
fn red_pixels(image: &Path, control_points: nsi::Arg<'_, '_>) -> usize {
    let ctx = nsi::Context::new(None).expect("context");

    ctx.create("cam_xf", nsi::TRANSFORM, None);
    ctx.connect("cam_xf", None, nsi::ROOT, "objects", None);
    #[rustfmt::skip]
    ctx.set_attribute(
        "cam_xf",
        &[nsi::matrix4_f64!("transformationmatrix", &[
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 5.0, 1.0,
        ])],
    );
    ctx.create("cam", nsi::PERSPECTIVE_CAMERA, None);
    ctx.set_attribute("cam", &[nsi::real_f32!("fov", 40.0)]);
    ctx.connect("cam", None, "cam_xf", "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "cam", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::integer_i32_slice!("resolution", &[64, 64])
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    );
    ctx.create("layer", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "layer",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "uint8"),
        ],
    );
    ctx.connect("layer", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "png"),
            nsi::string!("imagefilename", image.to_str().unwrap()),
        ],
    );
    ctx.connect("driver", None, "layer", "outputdrivers", None);

    let delight = std::env::var("DELIGHT").expect("$DELIGHT");
    ctx.create("red", nsi::SHADER, None);
    ctx.set_attribute(
        "red",
        &[
            nsi::string!(
                "shaderfilename",
                format!("{delight}/osl/dlConstant.oso").as_str()
            ),
            nsi::color3_f32!("i_color", &[1.0, 0.0, 0.0]),
        ],
    );
    ctx.create("look", nsi::ATTRIBUTES, None);
    ctx.connect("red", None, "look", "surfaceshader", None);

    ctx.create("patch", nsi::NURBS, None);
    ctx.connect("patch", None, nsi::ROOT, "objects", None);
    ctx.connect("look", None, "patch", "geometryattributes", None);
    ctx.set_attribute(
        "patch",
        &[
            nsi::integer_i32!("nu", 2),
            nsi::integer_i32!("nv", 2),
            nsi::integer_i32!("uorder", 2),
            nsi::integer_i32!("vorder", 2),
            nsi::real_f32_slice!("uknot", &[0.0, 0.0, 1.0, 1.0]),
            nsi::real_f32_slice!("vknot", &[0.0, 0.0, 1.0, 1.0]),
            control_points,
        ],
    );

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
    drop(ctx);

    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(image).expect("the render wrote an image"),
    ));
    let mut reader = decoder.read_info().expect("png");
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).expect("frame");
    let channels = info.color_type.samples();
    pixels[..info.buffer_size()]
        .chunks(channels)
        .filter(|pixel| pixel[0] > 128 && pixel[1] < 64)
        .count()
}

#[test]
fn a_rational_patch_renders_from_rust() {
    let directory = std::env::temp_dir().join("nsi_hpoint_test");
    std::fs::create_dir_all(&directory).unwrap();

    let points = [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    let plain = red_pixels(
        &directory.join("p.png"),
        nsi::point3_f32_slice!("P", &points),
    );

    // The same corners with weight 2, premultiplied: the patch the
    // renderer sees is half the size.
    let weighted = points.map(|[x, y, z]| [x, y, z, 2.0]);
    let rational = red_pixels(
        &directory.join("pw.png"),
        nsi::point4_f32_slice!("Pw", &weighted),
    );

    assert!(plain > 500, "the plain patch renders: {plain} red pixels");
    assert!(rational > 0, "the rational patch renders at all");
    // Half the width, a quarter of the area -- allow for edge pixels.
    let ratio = plain as f64 / rational as f64;
    assert!(
        (3.0..5.0).contains(&ratio),
        "a weight of 2 halves the patch: {plain} vs {rational} pixels"
    );
}
