use crate::p_ops;
use nsi_3delight as nsi_3dl;

fn nsi_camera<'a>(
    c: &nsi::Context<'a>,
    name: &str,
    open: nsi::output::OpenCallback,
    write: nsi::output::WriteCallback,
    finish: nsi::output::FinishCallback,
) {
    // Setup a camera transform.
    c.create("camera_xform", nsi::NodeType::Transform, &[]);
    c.connect("camera_xform", "", ".root", "objects", &[]);
    c.set_attribute(
        "camera_xform",
        &[nsi::double_matrix!(
            "transformationmatrix",
            &[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,]
        )],
    );

    // Setup a camera.
    c.create("camera", nsi::NodeType::PerspectiveCamera, &[]);
    c.connect("camera", "", "camera_xform", "objects", &[]);
    c.set_attribute("camera", &[nsi::float!("fov", 35.)]);

    // Setup a screen.
    c.create("screen", nsi::NodeType::Screen, &[]);
    c.connect("screen", "", "camera", "screens", &[]);
    c.set_attribute(
        "screen",
        &[
            nsi::integers!("resolution", &[128, 128]).array_len(2),
            nsi::integer!("oversampling", 32),
        ],
    );

    // RGB layer.
    c.create("beauty", nsi::NodeType::OutputLayer, &[]);
    c.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::integer!("withalpha", 1),
            nsi::string!("scalarformat", "float"),
        ],
    );
    c.connect("beauty", "", "screen", "outputlayers", &[]);

    // Setup an output driver.
    c.create("driver", nsi::NodeType::OutputDriver, &[]);
    c.connect("driver", "", "beauty", "outputdrivers", &[]);

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

    c.create("driver2", nsi::NodeType::OutputDriver, &[]);
    c.connect("driver2", "", "beauty", "outputdrivers", &[]);

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
    c.create("ground_xform_0", nsi::NodeType::Transform, &[]);
    c.connect("ground_xform_0", "", ".root", "objects", &[]);
    c.set_attribute(
        "ground_xform_0",
        &[nsi::double_matrix!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 0., -1., 0., 0., 1., 0., 0., 0., -1., 0.,
                1.,
            ]
        )],
    );

    c.create("ground_0", nsi::NodeType::Plane, &[]);
    c.connect("ground_0", "", "ground_xform_0", "objects", &[]);

    c.create("ground_attrib", nsi::NodeType::Attributes, &[]);
    c.set_attribute(
        "ground_attrib",
        &[nsi::unsigned!("visibility.camera", false as _)],
    );
    c.connect("ground_attrib", "", "ground_0", "geometryattributes", &[]);

    // Ground shader.
    c.create("ground_shader", nsi::NodeType::Shader, &[]);
    c.connect("ground_shader", "", "ground_attrib", "surfaceshader", &[]);

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
    c.create(attribute_name.clone(), nsi::NodeType::Attributes, &[]);
    c.connect(attribute_name.clone(), "", name, "geometryattributes", &[]);

    // Metal shader.
    let shader_name = format!("{}_shader", name);
    c.create(shader_name.clone(), nsi::NodeType::Shader, &[]);
    c.connect(
        shader_name.clone(),
        "",
        attribute_name,
        "surfaceshader",
        &[],
    );

    c.set_attribute(
        shader_name,
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
    let ctx = nsi::Context::new(&[]) //&[nsi::string!("streamfilename", "stdout")])
        .expect("Could not create NSI rendering context.");

    ctx.set_attribute(
        ".global",
        &[
            nsi::integer!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "spiral"),
            nsi::unsigned!("quality.shadingsamples", samples),
            nsi::integer!("maximumraydepth.reflection", 6),
        ],
    );

    nsi_camera(&ctx, &polyhedron.name(), open, write, finish);

    ctx.append(
        ".root",
        None,
        &nsi_3dl::environment_texture(
            &ctx,
            None,
            "assets/wooden_lounge_1k.tdl",
            None,
            None,
            Some(false),
            &[],
        )
        .0,
    );

    let name = polyhedron.to_nsi(&ctx, None, None, None, None);
    ctx.append(".root", None, &name);

    nsi_material(&ctx, &name);

    nsi_reflective_ground(&ctx);

    // And now, render it!
    ctx.render_control(&[nsi::string!("action", "start")]);
    ctx.render_control(&[nsi::string!("action", "wait")]);
}
