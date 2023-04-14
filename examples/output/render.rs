use crate::p_ops;
use nsi_3delight as nsi_3dl;
use nsi_core as nsi;
use nsi_toolbelt as nsi_tb;

fn nsi_camera<'a>(
    c: &nsi::Context<'a>,
    name: &str,
    open: nsi::output::OpenCallback,
    write: nsi::output::WriteCallback,
    finish: nsi::output::FinishCallback,
) {
    // Setup a camera TRANSFORM.
    c.create("camera_xform", nsi::TRANSFORM, None);
    c.connect("camera_xform", None, nsi::ROOT, "objects", None);
    c.set_attribute(
        "camera_xform",
        &[nsi::double_matrix!(
            "TRANSFORMationmatrix",
            &[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,]
        )],
    );

    // Setup a camera.
    c.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    c.connect("camera", None, "camera_xform", "objects", None);
    c.set_attribute("camera", &[nsi::float!("fov", 35.)]);

    // Setup a screen.
    c.create("screen", nsi::SCREEN, None);
    c.connect("screen", None, "camera", "screens", None);
    c.set_attribute(
        "screen",
        &[
            nsi::integers!("resolution", &[32, 32]).array_len(2),
            nsi::integer!("oversampling", 32),
        ],
    );

    // RGB layer.
    c.create("beauty", nsi::OUTPUT_LAYER, None);
    c.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::integer!("withalpha", 1),
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
            nsi::string!("drivername", nsi::output::FERRIS),
            nsi::string!("imagefilename", name),
            nsi::integer!("associatealpha", 1),
            nsi::callback!("callback.open", open),
            nsi::callback!("callback.write", write),
            nsi::callback!("callback.finish", finish),
        ],
    );

    c.create("driver2", nsi::OUTPUT_DRIVER, None);
    c.connect("driver2", None, "beauty", "outputdrivers", None);

    c.set_attribute(
        "driver2",
        &[
            nsi::string!("drivername", "exr"),
            nsi::string!("imagefilename", "foobs.exr"),
        ],
    );
}

fn nsi_reflective_ground(c: &nsi::Context) {
    // Floor.
    c.create("ground_xform_0", nsi::TRANSFORM, None);
    c.connect("ground_xform_0", None, nsi::ROOT, "objects", None);
    c.set_attribute(
        "ground_xform_0",
        &[nsi::double_matrix!(
            "TRANSFORMationmatrix",
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
        &[nsi::integer!("visibility.camera", false as _)],
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
            nsi::float!("roughness", 0.2),
            nsi::float!("specular_level", 1.),
            nsi::float!("metallic", 1.),
            nsi::float!("anisotropy", 1.),
            nsi::color!("anisotropy_direction", &[1., 0., 0.]),
            nsi::float!("sss_weight", 0.),
            nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
            nsi::float!("sss_scale", 0.),
            nsi::color!("incandescence", &[0., 0., 0.]),
            nsi::float!("incandescence_intensity", 0.),
        ],
    );
}

fn nsi_material(c: &nsi::Context, name: &str) {
    // Particle attributes.
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
            //nsi::arg!("coating_thickness", 0.1),
            nsi::float!("roughness", 0.01),
            nsi::float!("specular_level", 1.0),
            nsi::float!("metallic", 1.),
            nsi::float!("anisotropy", 0.),
            nsi::float!("sss_weight", 0.),
            nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
            nsi::float!("sss_scale", 0.),
            nsi::color!("incandescence", &[0., 0., 0.]),
            nsi::float!("incandescence_intensity", 0.),
        ],
    );
}

pub(crate) fn nsi_render<'a>(
    samples: u32,
    polyhedron: &p_ops::Polyhedron,
    open: nsi::output::OpenCallback,
    write: nsi::output::WriteCallback,
    finish: nsi::output::FinishCallback,
) {
    let ctx = nsi::Context::new(None) //&[nsi::string!("streamfilename", "stdout")])
        .expect("Could not create NSI rendering context.");

    ctx.set_attribute(
        ".global",
        &[
            nsi::integer!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "spiral"),
            nsi::integer!("quality.shadingsamples", samples as _),
            nsi::integer!("maximumraydepth.reflection", 6),
        ],
    );

    nsi_camera(&ctx, &polyhedron.name(), open, write, finish);

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

    let name = polyhedron.to_nsi(&ctx, None, None, None, None);
    nsi_tb::append(&ctx, nsi::ROOT, None, &name);

    nsi_material(&ctx, &name);

    nsi_reflective_ground(&ctx);

    // And now, render it!
    ctx.render_control(&[nsi::string!("action", "start")]);
    ctx.render_control(&[nsi::string!("action", "wait")]);
}
