use nsi_3delight as nsi_3dl;
use nsi_ffi_wrap as nsi;
use nsi_toolbelt as nsi_tb;
use std::num::NonZeroUsize;

fn nsi_camera<'a>(
    c: &nsi::Context<'a>,
    name: &str,
    open: nsi::output::OpenCallback,
    write: nsi::output::WriteCallback<'a, f32>,
    finish: nsi::output::FinishCallback<'a>,
) {
    // Setup a camera TRANSFORM.
    c.create("camera_xform", nsi::TRANSFORM, None);
    c.connect("camera_xform", None, nsi::ROOT, "objects", None);
    c.set_attribute(
        "camera_xform",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,
            ]
        )],
    );

    // Setup a camera.
    c.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    c.connect("camera", None, "camera_xform", "objects", None);
    c.set_attribute("camera", &[nsi::f32!("fov", 35.)]);

    // Setup a screen.
    c.create("screen", nsi::SCREEN, None);
    c.connect("screen", None, "camera", "screens", None);
    c.set_attribute(
        "screen",
        &[
            nsi::i32_slice!("resolution", &[128, 128])
                .array_len(const { NonZeroUsize::new(2).unwrap() }),
            nsi::i32!("oversampling", 32),
        ],
    );

    // RGB layer.
    c.create("beauty", nsi::OUTPUT_LAYER, None);
    c.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::i32!("withalpha", 1),
            nsi::string!("scalarformat", "float"),
        ],
    );
    c.connect("beauty", None, "screen", "outputlayers", None);

    // Setup an output driver.
    c.create("driver", nsi::OUTPUT_DRIVER, None);
    c.connect("driver", None, "beauty", "outputdrivers", None);

    c.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", nsi::output::FERRIS_F32),
            nsi::string!("imagefilename", name),
            nsi::i32!("associatealpha", 1),
            nsi::callback!("callback.open", open),
            nsi::callback!("callback.write", write),
            nsi::callback!("callback.finish", finish),
        ],
    );
}

fn nsi_reflective_ground(c: &nsi::Context) {
    // Floor.
    c.create("ground_xform_0", nsi::TRANSFORM, None);
    c.connect("ground_xform_0", None, nsi::ROOT, "objects", None);
    c.set_attribute(
        "ground_xform_0",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 0., -1., 0., 0., 1., 0., 0., 0., -1., 0.,
                1.,
            ]
        )],
    );

    c.create("ground_0", nsi::PLANE, None);
    c.connect("ground_0", None, "ground_xform_0", "objects", None);

    c.create("ground_attrib", nsi::ATTRIBUTES, None);
    c.set_attribute(
        "ground_attrib",
        &[nsi::i32!("visibility.camera", false as _)],
    );
    c.connect(
        "ground_attrib",
        None,
        "ground_0",
        "geometryattributes",
        None,
    );

    // Ground shader.
    c.create("ground_shader", nsi::SHADER, None);
    c.connect(
        "ground_shader",
        None,
        "ground_attrib",
        "surfaceshader",
        None,
    );

    c.set_attribute(
        "ground_shader",
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
            nsi::color!("i_color", &[0.001, 0.001, 0.001]),
            nsi::f32!("roughness", 0.2),
            nsi::f32!("specular_level", 1.),
            nsi::f32!("metallic", 1.),
            nsi::f32!("anisotropy", 1.),
            nsi::color!("anisotropy_direction", &[1., 0., 0.]),
            nsi::f32!("sss_weight", 0.),
            nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
            nsi::f32!("sss_scale", 0.),
            nsi::color!("incandescence", &[0., 0., 0.]),
            nsi::f32!("incandescence_intensity", 0.),
        ],
    );
}

fn nsi_material(c: &nsi::Context, name: &str) {
    let attribute_name = format!("{}_attrib", name);
    c.create(&attribute_name, nsi::ATTRIBUTES, None);
    c.connect(&attribute_name, None, name, "geometryattributes", None);

    // Metal shader.
    let shader_name = format!("{}_shader", name);
    c.create(&shader_name, nsi::SHADER, None);
    c.connect(&shader_name, None, &attribute_name, "surfaceshader", None);

    c.set_attribute(
        &shader_name,
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
            nsi::color!("i_color", &[1., 0.6, 0.3]),
            nsi::f32!("roughness", 0.01),
            nsi::f32!("specular_level", 1.0),
            nsi::f32!("metallic", 1.),
            nsi::f32!("anisotropy", 0.),
            nsi::f32!("sss_weight", 0.),
            nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
            nsi::f32!("sss_scale", 0.),
            nsi::color!("incandescence", &[0., 0., 0.]),
            nsi::f32!("incandescence_intensity", 0.),
        ],
    );
}

/// Build a creased-subdivision dodecahedron mesh node named `name`.
///
/// This is the same scene-graph shape that the top-level crate doc uses,
/// inlined here so the example has no third-party-geometry dependency.
fn nsi_dodecahedron(c: &nsi::Context, name: &str) {
    // 12 regular pentagon faces, 5 vertices each.
    let face_index: [i32; 60] = [
        0, 16, 2, 10, 8, 0, 8, 4, 14, 12, 16, 17, 1, 12, 0, 1, 9, 11, 3, 17, 1,
        12, 14, 5, 9, 2, 13, 15, 6, 10, 13, 3, 17, 16, 2, 3, 11, 7, 15, 13, 4,
        8, 10, 6, 18, 14, 5, 19, 18, 4, 5, 19, 7, 11, 9, 15, 7, 19, 18, 6,
    ];

    let phi: f32 = 0.5 * (1.0 + 5_f32.sqrt());
    let phi_c: f32 = phi - 1.0;

    let positions: [nsi::Point3F32; 20] = [
        [1., 1., 1.],
        [1., 1., -1.],
        [1., -1., 1.],
        [1., -1., -1.],
        [-1., 1., 1.],
        [-1., 1., -1.],
        [-1., -1., 1.],
        [-1., -1., -1.],
        [0., phi_c, phi],
        [0., phi_c, -phi],
        [0., -phi_c, phi],
        [0., -phi_c, -phi],
        [phi_c, phi, 0.],
        [phi_c, -phi, 0.],
        [-phi_c, phi, 0.],
        [-phi_c, -phi, 0.],
        [phi, 0., phi_c],
        [phi, 0., -phi_c],
        [-phi, 0., phi_c],
        [-phi, 0., -phi_c],
    ];

    // 30 unique edges of the dodecahedron, each as (start, end). Order
    // doesn't matter; matches the corresponding sharpness slice 1-to-1.
    let crease_edges: [i32; 60] = [
        0, 8, 0, 12, 0, 16, 1, 9, 1, 12, 1, 17, 2, 10, 2, 13, 2, 16, 3, 11, 3,
        13, 3, 17, 4, 8, 4, 14, 4, 18, 5, 9, 5, 14, 5, 19, 6, 10, 6, 15, 6, 18,
        7, 11, 7, 15, 7, 19, 8, 10, 9, 11, 12, 14, 13, 15, 16, 17, 18, 19,
    ];

    c.create(name, nsi::MESH, None);
    c.set_attribute(
        name,
        &[
            nsi::point_slice!(nsi::POSITION, &positions),
            nsi::i32_slice!("P.indices", &face_index),
            nsi::i32_slice!("nvertices", &[5; 12]),
            nsi::string!("subdivision.scheme", "catmull-clark"),
            nsi::i32_slice!("subdivision.creasevertices", &crease_edges),
            nsi::f32_slice!("subdivision.creasesharpness", &[4.2; 30]),
        ],
    );
}

pub(crate) fn nsi_render<'a>(
    samples: u32,
    name: &str,
    open: nsi::output::OpenCallback,
    write: nsi::output::WriteCallback<'a, f32>,
    finish: nsi::output::FinishCallback<'a>,
) {
    let ctx = nsi::Context::new(None)
        .expect("Could not create NSI rendering context.");

    ctx.set_attribute(
        ".global",
        &[
            nsi::i32!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "spiral"),
            nsi::i32!("quality.shadingsamples", samples as _),
            nsi::i32!("maximumraydepth.reflection", 6),
        ],
    );

    nsi_camera(&ctx, name, open, write, finish);

    nsi_tb::append(
        &ctx,
        nsi::ROOT,
        None,
        &nsi_3dl::environment_texture(
            &ctx,
            None,
            "assets/wooden_lounge_1k.tdl",
            None,
            None,
            Some(false),
            None,
        )
        .0,
    );

    nsi_dodecahedron(&ctx, name);
    nsi_tb::append(&ctx, nsi::ROOT, None, name);

    nsi_material(&ctx, name);
    nsi_reflective_ground(&ctx);

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
}
