#[cfg(test)]
#[test]
fn live_edit() {
    // # Compile the shaders.
    // os.system("oslc emitter.osl")
    // os.system("oslc matte.osl")
    // os.system("oslc waves.osl")

    // create rendering context.
    let c = nsi::Context::new(&vec![nsi::arg!("streamfilename", nsi::string!("stdout"))])
        .expect("Could not create NSI context.");

    // Setup a camera transform.
    c.create("cam1_trs", &nsi::Node::Transform, nsi::no_arg!());
    c.connect("cam1_trs", "", ".root", "objects", nsi::no_arg!());

    c.set_attribute(
        "cam1_trs",
        &vec![nsi::arg!(
            "transformationmatrix",
            nsi::double_matrix!(&[
                1.0f64, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,
            ])
        )],
    );

    // Setup a camera.
    c.create("cam1", &nsi::Node::PerspectiveCamera, nsi::no_arg!());

    c.set_attribute("cam1", &vec![nsi::arg!("fov", nsi::float!(35f32))]);
    c.connect("cam1", "", "cam1_trs", "objects", nsi::no_arg!());

    // Setup a screen.
    c.create("s1", &nsi::Node::Screen, nsi::no_arg!());
    c.connect("s1", "", "cam1", "screens", nsi::no_arg!());
    c.set_attribute(
        "s1",
        &vec![
            nsi::arg!("resolution", nsi::integers!(&[1280, 720])).array_len(2),
            nsi::arg!("oversampling", nsi::integer!(16i32)),
        ],
    );

    // Setup an output layer.
    c.create("beauty", &nsi::Node::OutputLayer, nsi::no_arg!());
    c.set_attribute(
        "beauty",
        &vec![
            nsi::arg!("variablename", nsi::string!("Ci")),
            nsi::arg!("withalpha", nsi::integer!(1)),
            nsi::arg!("scalarformat", nsi::string!("half")),
            nsi::arg!("some_color", nsi::color!(&[0.1f32, 0.2, 0.3])),
        ],
    );
    c.connect("beauty", "", "s1", "outputlayers", nsi::no_arg!());

    let mut foo = 42u64;
    // Setup an output driver.
    c.create("driver1", &nsi::Node::OutputDriver, nsi::no_arg!());
    c.connect("driver1", "", "beauty", "outputdrivers", nsi::no_arg!());
    c.set_attribute(
        "driver1",
        &vec![
            arg!(
                "drivername",
                nsi::string!("/Users/moritz/code/r-display/target/debug/libr_display.dylib")
            ),
            nsi::arg!(
                "blblabla_______",
                nsi::pointer!(
                    &unsafe { std::mem::transmute::<_, f64>(&mut foo as *mut _) } as *const f64
                        as _
                )
            ),
        ],
    );

    // Add a plane.
    c.create("mesh1", &nsi::Node::Mesh, nsi::no_arg!());
    c.connect("mesh1", "", ".root", "objects", nsi::no_arg!());
    c.set_attribute(
        "mesh1",
        &vec![
            arg!("nvertices", nsi::integer!(4i32)),
            arg!(
                "P",
                nsi::points!(&[-1.0f32, -0.8, -1., -1., -0.8, 1., 1., -0.8, 1., 1., -0.8, -1.0])
            ),
        ],
    );

    // Add a basic shader for the plane.
    c.create("shader1", &nsi::Node::Shader, nsi::no_arg!());
    c.set_attribute(
        "shader1",
        &vec![nsi::arg!("shaderfilename", nsi::string!("matte"))],
    );
    c.create("plane_attribs", &nsi::Node::Attributes, nsi::no_arg!());
    c.connect(
        "plane_attribs",
        "",
        "mesh1",
        "geometryattributes",
        nsi::no_arg!(),
    );
    c.connect(
        "shader1",
        "",
        "plane_attribs",
        "surfaceshader",
        nsi::no_arg!(),
    );

    // Add a triangular light, with shader.
    c.create("light1_trs", &nsi::Node::Transform, nsi::no_arg!());
    c.connect("light1_trs", "", ".root", "objects", nsi::no_arg!());

    c.set_attribute(
        "light1_trs",
        &vec![nsi::arg!(
            "transformationmatrix",
            nsi::double_matrix!(&[
                0.1f64, 0., 0., 0., 0., 0.1, 0., 0., 0., 0., 0.1, 0., 0., 4., 0., 1.,
            ])
        )],
    );

    c.create("light1", &nsi::Node::Mesh, nsi::no_arg!());
    // This one is connected to the transform instead of the mesh
    // itself. Because we can.
    c.connect("light1", "", "light1_trs", "objects", nsi::no_arg!());
    c.set_attribute(
        "light1",
        &vec![
            nsi::arg!("nvertices", nsi::integer!(3i32)),
            nsi::arg!(
                "P",
                nsi::points!(&vec![-1.0f32, 0., 0., 0., 0., 1., 1., 0., 0.0])
            ),
        ],
    );

    c.create("light1_shader", &nsi::Node::Shader, nsi::no_arg!());
    c.set_attribute(
        "light1_shader",
        &vec![
            nsi::arg!("shaderfilename", nsi::string!("emitter")),
            nsi::arg!("power", nsi::float!(80f32)),
        ],
    );

    c.create("light1_attribs", &nsi::Node::Attributes, nsi::no_arg!());
    c.connect(
        "light1_attribs",
        "",
        "light1_trs",
        "geometryattributes",
        nsi::no_arg!(),
    );
    c.connect(
        "light1_shader",
        "",
        "light1_attribs",
        "surfaceshader",
        nsi::no_arg!(),
    );

    // Start interactive render.
    c.render_control(&vec![nsi::arg!("action", nsi::string!("start"))]); //, interactive=1)

    // Let it render a while.
    //thread::sleep(time::Duration::from_secs(5));

    c.render_control(&vec![nsi::arg!("action", nsi::string!("wait"))]);
}

/*
# Add something between light and plane to make some shadows.
c.create("mesh2", "mesh")
c.connect("mesh2", "", nsi.SCENE_ROOT, "objects")
c.set_attribute("mesh2",
    nvertices=3,
    P=nsi.Arg((-0.2,-0.3,0.5,  0.2,-0.3,0.5,  0,-0.3,0), type=nsi.Type.Point))

c.create("mesh2_attribs", "attributes")
c.connect("shader1", "", "mesh2_attribs", "surfaceshader")
c.connect("mesh2_attribs", "", "mesh2", "geometryattributes")

# Increase quality.
# This particular call uses a dictionary for arguments because the attribute
# name has a "." in it and
c.set_attribute(nsi.SCENE_GLOBAL, **{"quality.shadingsamples":64})

# Apply changes and let render a while.
c.RenderControl(action="synchronize")
time.sleep(5)

# Make it move. This inserts a transform node for mesh2.
c.create("mesh2_trs", "transform")
c.connect("mesh2_trs", "", nsi.SCENE_ROOT, "objects")
c.Disconnect("mesh2", "", nsi.SCENE_ROOT, "objects")
c.connect("mesh2", "", "mesh2_trs", "objects")

c.set_attributeAtTime("mesh2_trs", 0., transformationmatrix=nsi.Arg(
    (1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1),
    type=nsi.Type.DoubleMatrix))
c.set_attributeAtTime("mesh2_trs", 1., transformationmatrix=nsi.Arg(
    (1,0,0,0, 0,1,0,0, 0,0,1,0, 0.4,0,0,1),
    type=nsi.Type.DoubleMatrix))

# Must also set shutter or no motion blur will be visible.
c.set_attribute("cam1", shutterrange=(0.2, 0.8))

# Apply changes and let render a while.
c.RenderControl(action="synchronize")
time.sleep(5)

# Add an actual shader network, very basic.
c.create("wave_shader", "shader")
c.set_attribute("wave_shader", shaderfilename="waves")
c.connect("wave_shader", "outColor", "shader1", "Cs")

# Apply changes and let render a while.
c.RenderControl(action="synchronize")
time.sleep(5)

# Recursively delete the shader network.
c.Delete("shader1", recursive=1)
# Replace by something else. Note that we only connect it to plane_attribs so
# it will not apply to the small triangle creating the shadow, which no longer
# has any shader. It will render black but still be visible in the alpha
# channel.
c.create("shader2", "shader")
c.set_attribute("shader2", shaderfilename="matte", Cs=nsi.ColorArg(1,0.2,0.2))
c.connect("shader2", "", "plane_attribs", "surfaceshader")

# Apply changes and let render a while.
c.RenderControl(action="synchronize")
time.sleep(5)

# Stop the render.
c.RenderControl(action="stop")

# Add a second output driver to produce an exr image.
c.create("driver2", "outputdriver")
c.connect("driver2", "", "beauty", "outputdrivers")
c.set_attribute("driver2",
    drivername="exr",
    imagefilename="test_output.exr")

# Add a second layer to that exr image. It"s a debug AOV from the sample matte
# shader. See matte.osl.
c.create("pattern_layer", "outputlayer")
c.set_attribute("pattern_layer",
    variablename="surfacecolor",
    scalarformat="half")
c.connect("pattern_layer", "", "s1", "outputlayers")
c.connect("driver2", "", "pattern_layer", "outputdrivers")

# Do a regular (non interactive) render with the same scene.
c.RenderControl(action="start")
c.RenderControl(action="wait")

# Cleanup context.
c.End()

# vim: set softtabstop=4 expandtab shiftwidth=4:
*/
