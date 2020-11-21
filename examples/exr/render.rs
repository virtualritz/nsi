use polyhedron_ops as p_ops;
use std::{env, path::PathBuf};

fn nsi_camera<'a>(
    c: &nsi::Context<'a>,
    name: &str,
    camera_xform: &[f64; 16],
    samples: u32,
    open: nsi::output::OpenCallback,
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
            nsi::integer!("oversampling", 1),
        ],
    );

    c.set_attribute(
        ".global",
        &[
            nsi::integer!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "spiral"),
            nsi::unsigned!("quality.shadingsamples", samples),
            nsi::integer!("maximumraydepth.reflection", 6),
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
            //nsi::string!("filter", "box"),
            nsi::double!("filterwidth", 1.),
        ],
    );
    c.connect("beauty", "", "screen", "outputlayers", &[]);

    /*
    // Normal layer.
    c.create("albedo", nsi::NodeType::OutputLayer, &[]);
    c.set_attribute(
        "albedo",
        &[
            nsi::string!("variablename", "albedo"),
            nsi::string!("variablesource", "shader"),
            nsi::string!("layertype", "color"),
            nsi::string!("scalarformat", "float"),
            nsi::string!("filter", "box"),
            nsi::double!("filterwidth", 1.),
        ],
    );
    c.connect("albedo", "", "screen", "outputlayers", &[]);

    // Normal layer.
    c.create("normal", nsi::NodeType::OutputLayer, &[]);
    c.set_attribute(
        "normal",
        &[
            nsi::string!("variablename", "N.world"),
            nsi::string!("variablesource", "builtin"),
            nsi::string!("layertype", "vector"),
            nsi::string!("scalarformat", "float"),
            nsi::string!("filter", "box"),
            nsi::double!("filterwidth", 1.),
        ],
    );
    c.connect("normal", "", "screen", "outputlayers", &[]);*/

    // Setup an output driver.
    c.create("driver", nsi::NodeType::OutputDriver, &[]);
    c.connect("driver", "", "beauty", "outputdrivers", &[]);
    //c.connect("driver", "", "albedo", "outputdrivers", &[]);
    //c.connect("driver", "", "normal", "outputdrivers", &[]);

    c.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "ferris"),
            nsi::string!("imagefilename", name),
            nsi::callback!("callback.open", open),
            nsi::callback!("callback.finish", finish),
        ],
    );

    /*
    c.create("driver2", nsi::NodeType::OutputDriver, &[]);
    c.connect("driver2", "", "beauty", "outputdrivers", &[]);
    c.set_attribute("driver2", &[nsi::string!("drivername", "ioutput")]);*/
}

fn nsi_environment(c: &nsi::Context) {
    if let Ok(path) = &env::var("DELIGHT") {
        // Set up an environment light.
        c.create("env_xform", nsi::NodeType::Transform, &[]);
        c.connect("env_xform", "", ".root", "objects", &[]);

        c.create("environment", nsi::NodeType::Environment, &[]);
        c.connect("environment", "", "env_xform", "objects", &[]);

        c.create("env_attrib", nsi::NodeType::Attributes, &[]);
        c.connect("env_attrib", "", "environment", "geometryattributes", &[]);

        c.set_attribute("env_attrib", &[nsi::integer!("visibility.camera", 0)]);

        c.create("env_shader", nsi::NodeType::Shader, &[]);
        c.connect("env_shader", "", "env_attrib", "surfaceshader", &[]);

        // Environment light attributes.
        c.set_attribute(
            "env_shader",
            &[
                nsi::string!(
                    "shaderfilename",
                    PathBuf::from(path)
                        .join("osl")
                        .join("environmentLight")
                        .to_string_lossy()
                        .into_owned()
                ),
                nsi::float!("intensity", 1.),
            ],
        );

        c.set_attribute(
            "env_shader",
            &[nsi::string!("image", "assets/wooden_lounge_1k.tdl")],
        );
    }
}

fn nsi_reflective_ground(c: &nsi::Context) {
    if let Ok(path) = &env::var("DELIGHT") {
        // Floor.
        c.create("ground_xform_0", nsi::NodeType::Transform, &[]);
        c.connect("ground_xform_0", "", ".root", "objects", &[]);
        c.set_attribute(
            "ground_xform_0",
            &[nsi::double_matrix!(
                "transformationmatrix",
                &[1., 0., 0., 0., 0., 0., -1., 0., 0., 1., 0., 0., 0., -1., 0., 1.,]
            )],
        );

        c.create("ground_0", nsi::NodeType::Plane, &[]);
        c.connect("ground_0", "", "ground_xform_0", "objects", &[]);

        /*
        // Ceiling.
        c.create("ground_xform_1", nsi::NodeType::Transform, &[]);
        c.connect("ground_xform_1", "", ".root", "objects", &[]);
        c.set_attribute(
            "ground_xform_1",
            &[nsi::double_matrix!(
                "transformationmatrix",
                &[1., 0., 0., 0.,
                  0., 0., -1., 0.,
                  0., 1., 0., 0.,
                  0., 1., 0., 1.,]
            )],
        );

        c.create("ground_1", nsi::NodeType::Plane, &[]);
        c.connect("ground_1", "", "ground_xform_1", "objects", &[]);*/

        c.create("ground_attrib", nsi::NodeType::Attributes, &[]);
        c.set_attribute(
            "ground_attrib",
            &[nsi::unsigned!("visibility.camera", false as _)],
        );
        c.connect("ground_attrib", "", "ground_0", "geometryattributes", &[]);

        // c.connect("ground_attrib", "", "ground_1", "geometryattributes", &[]);

        // Ground shader.
        c.create("ground_shader", nsi::NodeType::Shader, &[]);
        c.connect("ground_shader", "", "ground_attrib", "surfaceshader", &[]);

        c.set_attribute(
            "ground_shader",
            &[
                nsi::string!(
                    "shaderfilename",
                    PathBuf::from(path)
                        .join("osl")
                        .join("dlPrincipled")
                        .to_string_lossy()
                        .into_owned()
                ),
                nsi::color!("i_color", &[0.001, 0.001, 0.001]),
                //nsi::arg!("coating_thickness", &0.1f32),
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
                //nsi::color!("incandescence_multiplier", &[1.0f32, 1.0, 1.0]),
            ],
        );
    }
}

fn nsi_material(c: &nsi::Context, name: &str) {
    if let Ok(path) = &env::var("DELIGHT") {
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
                nsi::string!(
                    "shaderfilename",
                    PathBuf::from(path)
                        .join("osl")
                        .join("dlPrincipled")
                        .to_string_lossy()
                        .into_owned()
                ),
                nsi::color!("i_color", &[1., 0.6, 0.3]),
                //nsi::arg!("coating_thickness", 0.1),
                nsi::float!("roughness", 0.3),
                nsi::float!("specular_level", 1.0),
                nsi::float!("metallic", 1.),
                nsi::float!("anisotropy", 0.),
                nsi::float!("sss_weight", 0.),
                nsi::color!("sss_color", &[0.5, 0.5, 0.5]),
                nsi::float!("sss_scale", 0.),
                nsi::color!("incandescence", &[0., 0., 0.]),
                nsi::float!("incandescence_intensity", 0.),
                //nsi::color!("incandescence_multiplier", &[1., 1., 1.]),
            ],
        );

        /*
        c.set_attribute(
            shader_name,
            &[
                nsi::string!(
                    "shaderfilename",
                    PathBuf::from(path)
                        .join("osl")
                        .join("dlStandard")
                        .to_string_lossy()
                        .into_owned()
                ),
                nsi::float!("base", 0.3),
                nsi::color!("base_color", &[1., 0.6, 0.3]),
                //nsi::arg!("coating_thickness", 0.1),
                //nsi::float!("roughness", 0.),
                nsi::float!("specular", 0.),
                nsi::float!("sheen", 1.),
                nsi::float!("sheen_roughness", 0.8),
                //nsi::color!("sheen_color", &[1., 0.6, 0.3]),
                //nsi::float!("metallic", 1.),
                //nsi::float!("anisotropy", 0.),
                //nsi::float!("sss_weight", 0.),
                //nsi::color!("sss_color", &[0.5f32, 0.5, 0.5]),
                //nsi::float!("sss_scale", 0.0f32),
                //nsi::color!("incandescence", &[0.0f32, 0.0, 0.0]),
                //nsi::float!("incandescence_intensity", 0.0f32),
                //nsi::color!("incandescence_multiplier", &[1.0f32, 1.0, 1.0]),
            ],
        );*/
    }
}

pub(crate) fn nsi_render<'a>(
    polyhedron: &p_ops::Polyhedron,
    camera_xform: &[f64; 16],
    samples: u32,
    cloud_render: bool,
    open: nsi::output::OpenCallback,
    finish: nsi::output::FinishCallback,
) {
    let ctx = {
        if cloud_render {
            nsi::Context::<'a>::new(&[
                nsi::integer!("cloud", 1),
                nsi::string!("software", "HOUDINI"),
            ])
        } else {
            nsi::Context::new(&[])
        }
    }
    .expect("Could not create NSI rendering context.");

    nsi_camera(
        &ctx,
        &polyhedron.name(),
        camera_xform,
        samples,
        open,
        finish,
    );

    nsi_environment(&ctx);

    let name = polyhedron.to_nsi(&ctx, Some(".root"), None, None);

    nsi_material(&ctx, &name);

    nsi_reflective_ground(&ctx);

    // And now, render it!
    ctx.render_control(&[nsi::string!("action", "start")]);
    ctx.render_control(&[nsi::string!("action", "wait")]);
}
