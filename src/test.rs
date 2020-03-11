#[cfg(test)]
use super::*;
use crate as nsi;

#[test]
fn live_edit() {
    // # Compile the shaders.
    // os.system('oslc emitter.osl')
    // os.system('oslc matte.osl')
    // os.system('oslc waves.osl')

    // Create rendering context.
    let c = nsi::Context::new(&vec![nsi::Arg::new(
        "streamfilename",
        &String::from("stdout"),
    )]);

    // Setup a camera transform.
    c.create("cam1_trs", &nsi::Node::Transform, &nsi::ArgVec::new());
    c.connect("cam1_trs", "", ".root", "objects", &nsi::ArgVec::new());
    c.set_attribute(
        "cam1_trs",
        &vec![nsi::Arg::new(
            "transformationmatrix",
            &vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0, 0.0,
            ],
        )
        .type_of(nsi::Type::DoubleMatrix)],
    );
}
/*
# Setup a camera.
c.Create('cam1', 'perspectivecamera')
c.SetAttribute('cam1', fov=nsi.FloatArg(35))
c.Connect('cam1', '', 'cam1_trs', 'objects')

# Setup a screen.
c.Create('s1', 'screen')
c.Connect('s1', '', 'cam1', 'screens')
c.SetAttribute('s1',
    resolution=nsi.Arg((1280,720), arraylength=2),
    oversampling=16)

# Setup an output layer.
c.Create('beauty', 'outputlayer')
c.SetAttribute('beauty', variablename='Ci', withalpha=1, scalarformat='half')
c.Connect('beauty', '', 's1', 'outputlayers')

# Setup an output driver.
c.Create('driver1', 'outputdriver')
c.Connect('driver1', '', 'beauty', 'outputdrivers')
c.SetAttribute('driver1',
    drivername='idisplay')

# Add a plane.
c.Create('mesh1', 'mesh')
c.Connect('mesh1', '', nsi.SCENE_ROOT, 'objects')
c.SetAttribute('mesh1',
    nvertices=4,
    P=nsi.Arg(
        (-1,-0.8,-1,  -1,-0.8,1,  1,-0.8,1,  1,-0.8,-1),
        type=nsi.Type.Point))

# Add a basic shader for the plane.
c.Create('shader1', 'shader')
c.SetAttribute('shader1', shaderfilename='matte')
c.Create('plane_attribs', 'attributes')
c.Connect('plane_attribs', '', 'mesh1', 'geometryattributes')
c.Connect('shader1', '', 'plane_attribs', 'surfaceshader')

# Add a triangular light, with shader.
c.Create('light1_trs', 'transform')
c.Connect('light1_trs', '', nsi.SCENE_ROOT, 'objects')
c.SetAttribute('light1_trs', transformationmatrix=nsi.Arg(
    (0.1,0,0,0, 0,0.1,0,0, 0,0,0.1,0, 0,4,0,1),
    type=nsi.Type.DoubleMatrix))

c.Create('light1', 'mesh')
# This one is connected to the transform instead of the mesh itself. Because we can.
c.Connect('light1', '', 'light1_trs', 'objects')
c.SetAttribute('light1',
    nvertices=3,
    P=nsi.Arg((-1,0,0, 0,0,1, 1,0,0), type=nsi.Type.Point))

c.Create('light1_shader', 'shader')
c.SetAttribute('light1_shader',
    shaderfilename='emitter',
    power=nsi.FloatArg(80))

c.Create('light1_attribs', 'attributes')
c.Connect('light1_attribs', '', 'light1_trs', 'geometryattributes')
c.Connect('light1_shader', '', 'light1_attribs', 'surfaceshader')

# Start interactive render.
c.RenderControl(action='start', interactive=1)

# Let it render a while.
time.sleep(5)

# Add something between light and plane to make some shadows.
c.Create('mesh2', 'mesh')
c.Connect('mesh2', '', nsi.SCENE_ROOT, 'objects')
c.SetAttribute('mesh2',
    nvertices=3,
    P=nsi.Arg((-0.2,-0.3,0.5,  0.2,-0.3,0.5,  0,-0.3,0), type=nsi.Type.Point))

c.Create('mesh2_attribs', 'attributes')
c.Connect('shader1', '', 'mesh2_attribs', 'surfaceshader')
c.Connect('mesh2_attribs', '', 'mesh2', 'geometryattributes')

# Increase quality.
# This particular call uses a dictionary for arguments because the attribute
# name has a '.' in it and
c.SetAttribute(nsi.SCENE_GLOBAL, **{'quality.shadingsamples':64})

# Apply changes and let render a while.
c.RenderControl(action='synchronize')
time.sleep(5)

# Make it move. This inserts a transform node for mesh2.
c.Create('mesh2_trs', 'transform')
c.Connect('mesh2_trs', '', nsi.SCENE_ROOT, 'objects')
c.Disconnect('mesh2', '', nsi.SCENE_ROOT, 'objects')
c.Connect('mesh2', '', 'mesh2_trs', 'objects')

c.SetAttributeAtTime('mesh2_trs', 0.0, transformationmatrix=nsi.Arg(
    (1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1),
    type=nsi.Type.DoubleMatrix))
c.SetAttributeAtTime('mesh2_trs', 1.0, transformationmatrix=nsi.Arg(
    (1,0,0,0, 0,1,0,0, 0,0,1,0, 0.4,0,0,1),
    type=nsi.Type.DoubleMatrix))

# Must also set shutter or no motion blur will be visible.
c.SetAttribute('cam1', shutterrange=(0.2, 0.8))

# Apply changes and let render a while.
c.RenderControl(action='synchronize')
time.sleep(5)

# Add an actual shader network, very basic.
c.Create('wave_shader', 'shader')
c.SetAttribute('wave_shader', shaderfilename='waves')
c.Connect('wave_shader', 'outColor', 'shader1', 'Cs')

# Apply changes and let render a while.
c.RenderControl(action='synchronize')
time.sleep(5)

# Recursively delete the shader network.
c.Delete('shader1', recursive=1)
# Replace by something else. Note that we only connect it to plane_attribs so
# it will not apply to the small triangle creating the shadow, which no longer
# has any shader. It will render black but still be visible in the alpha
# channel.
c.Create('shader2', 'shader')
c.SetAttribute('shader2', shaderfilename='matte', Cs=nsi.ColorArg(1,0.2,0.2))
c.Connect('shader2', '', 'plane_attribs', 'surfaceshader')

# Apply changes and let render a while.
c.RenderControl(action='synchronize')
time.sleep(5)

# Stop the render.
c.RenderControl(action='stop')

# Add a second output driver to produce an exr image.
c.Create('driver2', 'outputdriver')
c.Connect('driver2', '', 'beauty', 'outputdrivers')
c.SetAttribute('driver2',
    drivername='exr',
    imagefilename='test_output.exr')

# Add a second layer to that exr image. It's a debug AOV from the sample matte
# shader. See matte.osl.
c.Create('pattern_layer', 'outputlayer')
c.SetAttribute('pattern_layer',
    variablename='surfacecolor',
    scalarformat='half')
c.Connect('pattern_layer', '', 's1', 'outputlayers')
c.Connect('driver2', '', 'pattern_layer', 'outputdrivers')

# Do a regular (non interactive) render with the same scene.
c.RenderControl(action='start')
c.RenderControl(action='wait')

# Cleanup context.
c.End()

# vim: set softtabstop=4 expandtab shiftwidth=4:
*/
