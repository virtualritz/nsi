#[cfg(test)]
#[test]
fn live_edit() {
    // # Compile the shaders.
    // "oslc emitter.osl")
    // "oslc matte.osl")
    // "oslc waves.osl")

    // create rendering context.
    let c = nsi::Context::new(&[nsi::string!("streamfilename", "stdout")])
        .expect("Could not create NSI context.");

    // Setup a camera transform.
    c.create("cam1_trs", nsi::Node::Transform, &[]);
    c.connect("cam1_trs", "", ".root", "objects", &[]);

    c.set_attribute(
        "cam1_trs",
        &[nsi::double_matrix!(
            "transformationmatrix",
            &[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,]
        )],
    );

    // Setup a camera.
    c.create("cam1", nsi::Node::PerspectiveCamera, &[]);

    c.set_attribute("cam1", &[nsi::float!("fov", 35.)]);
    c.connect("cam1", "", "cam1_trs", "objects", &[]);

    // Setup a screen.
    c.create("s1", nsi::Node::Screen, &[]);
    c.connect("s1", "", "cam1", "screens", &[]);
    c.set_attribute(
        "s1",
        &[
            nsi::integers!("resolution", &[1280, 720]).array_len(2),
            nsi::integer!("oversampling", 16),
        ],
    );

    // Setup an output layer.
    c.create("beauty", nsi::Node::OutputLayer, &[]);
    c.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::integer!("withalpha", 1),
            nsi::string!("scalarformat", "half"),
            nsi::color!("some_color", &[0.1f32, 0.2, 0.3]),
        ],
    );
    c.connect("beauty", "", "s1", "outputlayers", &[]);

    let foo = 42u64;
    // Setup an output driver.
    c.create("driver1", nsi::Node::OutputDriver, &[]);
    c.connect("driver1", "", "beauty", "outputdrivers", &[]);
    c.set_attribute(
        "driver1",
        &[
            nsi::string!("drivername", "idisplay"),
            // pass a pointer to foo to NSI
            nsi::reference!("blblabla_______", Some(&foo)),
        ],
    );

    // Add a plane.
    c.create("mesh1", nsi::Node::Mesh, &[]);
    c.connect("mesh1", "", ".root", "objects", &[]);
    c.set_attribute(
        "mesh1",
        &[
            nsi::integer!("nvertices", 4),
            nsi::points!(
                "P",
                &[-1.0f32, -0.8, -1., -1., -0.8, 1., 1., -0.8, 1., 1., -0.8, -1.0]
            ),
        ],
    );

    c.create("plane_attribs", nsi::Node::Attributes, &[]);
    c.connect("plane_attribs", "", "mesh1", "geometryattributes", &[]);

    // Add a basic shader for the plane.
    c.create("shader1", nsi::Node::Shader, &[]);
    c.set_attribute("shader1", &[nsi::string!("shaderfilename", "matte")]);
    c.connect("shader1", "", "plane_attribs", "surfaceshader", &[]);

    // Add a triangular light, with shader.
    c.create("light1_trs", nsi::Node::Transform, &[]);
    c.connect("light1_trs", "", ".root", "objects", &[]);

    c.set_attribute(
        "light1_trs",
        &[nsi::double_matrix!(
            "transformationmatrix",
            &[0.1f64, 0., 0., 0., 0., 0.1, 0., 0., 0., 0., 0.1, 0., 0., 4., 0., 1.,]
        )],
    );

    c.create("light1", nsi::Node::Mesh, &[]);
    // This one is connected to the transform instead of the mesh
    // itself. Because we can.
    c.connect("light1", "", "light1_trs", "objects", &[]);
    c.set_attribute(
        "light1",
        &[
            nsi::integer!("nvertices", 3),
            nsi::points!("P", &[-1., 0., 0., 0., 0., 1., 1., 0., 0.]),
        ],
    );

    c.create("light1_shader", nsi::Node::Shader, &[]);
    c.set_attribute(
        "light1_shader",
        &[
            nsi::string!("shaderfilename", "emitter"),
            nsi::float!("power", 80.),
        ],
    );

    c.create("light1_attribs", nsi::Node::Attributes, &[]);
    c.connect(
        "light1_attribs",
        "",
        "light1_trs",
        "geometryattributes",
        &[],
    );
    c.connect("light1_shader", "", "light1_attribs", "surfaceshader", &[]);

    // Start interactive render.
    c.render_control(&[nsi::string!("action", "start")]); //, interactive=1)

    // Let it render a while.
    //thread::sleep(time::Duration::from_secs(5));

    c.render_control(&[nsi::string!("action", "wait")]);

    drop(c);
}

// FIXME: port rest of live_edit example from Python
