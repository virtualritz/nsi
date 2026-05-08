//! Common utilities and helpers for NSI tests.

use bytemuck;
use nsi_ffi_wrap as nsi;

/// Add a simple diffuse sphere to the scene.
pub fn add_test_sphere(
    ctx: &nsi::Context,
    name: &str,
    position: &[f64; 3],
    radius: f64,
) {
    // Transform
    let xform_name = format!("{}_xform", name);
    ctx.create(&xform_name, nsi::TRANSFORM, None);
    ctx.connect(&xform_name, None, nsi::ROOT, "objects", None);

    let transform_matrix = [
        radius,
        0.,
        0.,
        0.,
        0.,
        radius,
        0.,
        0.,
        0.,
        0.,
        radius,
        0.,
        position[0],
        position[1],
        position[2],
        1.,
    ];
    ctx.set_attribute(
        &xform_name,
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &transform_matrix
        )],
    );

    // Sphere (using a subdivided cube)
    ctx.create(name, nsi::MESH, None);
    ctx.connect(name, None, &xform_name, "objects", None);

    // Unit cube vertices
    let positions = [
        -1., -1., -1., 1., -1., -1., 1., 1., -1., -1., 1., -1., -1., -1., 1.,
        1., -1., 1., 1., 1., 1., -1., 1., 1.,
    ];

    // Cube faces
    let face_indices: [i32; 24] = [
        0, 1, 2, 3, // front
        4, 7, 6, 5, // back
        0, 4, 5, 1, // bottom
        2, 6, 7, 3, // top
        0, 3, 7, 4, // left
        1, 5, 6, 2, // right
    ];

    let points: &[[f32; 3]] = bytemuck::cast_slice(&positions);

    ctx.set_attribute(
        name,
        &[
            nsi::point_slice!("P", points),
            nsi::i32_slice!("P.indices", &face_indices),
            nsi::i32_slice!("nvertices", &[4; 6]),
            nsi::string!("subdivision.scheme", "catmull-clark"),
            nsi::i32!("subdivision.level", 4), // High subdivision for smooth sphere
        ],
    );
}

/// Add a basic material to a geometry node.
pub fn add_diffuse_material(
    ctx: &nsi::Context,
    geometry_name: &str,
    color: &[f32; 3],
    roughness: f32,
) {
    let attrib_name = format!("{}_attrib", geometry_name);
    let shader_name = format!("{}_shader", geometry_name);

    // Attributes node
    ctx.create(&attrib_name, nsi::ATTRIBUTES, None);
    ctx.connect(
        &attrib_name,
        None,
        geometry_name,
        "geometryattributes",
        None,
    );

    // Shader
    ctx.create(&shader_name, nsi::SHADER, None);
    ctx.connect(&shader_name, None, &attrib_name, "surfaceshader", None);

    ctx.set_attribute(
        &shader_name,
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
            nsi::color!("i_color", color),
            nsi::f32!("roughness", roughness),
            nsi::f32!("specular_level", 0.5),
            nsi::f32!("metallic", 0.0),
        ],
    );
}

/// Add a metallic material to a geometry node.
pub fn add_metal_material(
    ctx: &nsi::Context,
    geometry_name: &str,
    color: &[f32; 3],
    roughness: f32,
) {
    let attrib_name = format!("{}_attrib", geometry_name);
    let shader_name = format!("{}_shader", geometry_name);

    // Attributes node
    ctx.create(&attrib_name, nsi::ATTRIBUTES, None);
    ctx.connect(
        &attrib_name,
        None,
        geometry_name,
        "geometryattributes",
        None,
    );

    // Shader
    ctx.create(&shader_name, nsi::SHADER, None);
    ctx.connect(&shader_name, None, &attrib_name, "surfaceshader", None);

    ctx.set_attribute(
        &shader_name,
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
            nsi::color!("i_color", color),
            nsi::f32!("roughness", roughness),
            nsi::f32!("specular_level", 1.0),
            nsi::f32!("metallic", 1.0),
        ],
    );
}

/// Add a simple area light.
pub fn add_area_light(
    ctx: &nsi::Context,
    name: &str,
    position: &[f64; 3],
    size: f64,
    intensity: f32,
) {
    let xform_name = format!("{}_xform", name);
    let shader_name = format!("{}_shader", name);
    let attrib_name = format!("{}_attrib", name);

    // Transform
    ctx.create(&xform_name, nsi::TRANSFORM, None);
    ctx.connect(&xform_name, None, nsi::ROOT, "objects", None);

    let transform_matrix = [
        size,
        0.,
        0.,
        0.,
        0.,
        size,
        0.,
        0.,
        0.,
        0.,
        size,
        0.,
        position[0],
        position[1],
        position[2],
        1.,
    ];
    ctx.set_attribute(
        &xform_name,
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &transform_matrix
        )],
    );

    // Light geometry (quad)
    ctx.create(name, nsi::MESH, None);
    ctx.connect(name, None, &xform_name, "objects", None);

    let positions = [-1., 0., -1., 1., 0., -1., 1., 0., 1., -1., 0., 1.];

    let points: &[[f32; 3]] = bytemuck::cast_slice(&positions);

    ctx.set_attribute(
        name,
        &[nsi::i32!("nvertices", 4), nsi::point_slice!("P", points)],
    );

    // Attributes
    ctx.create(&attrib_name, nsi::ATTRIBUTES, None);
    ctx.connect(&attrib_name, None, name, "geometryattributes", None);

    // Emissive shader
    ctx.create(&shader_name, nsi::SHADER, None);
    ctx.connect(&shader_name, None, &attrib_name, "surfaceshader", None);

    ctx.set_attribute(
        &shader_name,
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/areaLight"),
            nsi::f32!("intensity", intensity),
        ],
    );
}

/// Add a simple environment light with constant color.
pub fn add_constant_environment(
    ctx: &nsi::Context,
    color: &[f32; 3],
    intensity: f32,
) {
    // Environment
    ctx.create("environment", nsi::ENVIRONMENT, None);
    ctx.connect("environment", None, nsi::ROOT, "objects", None);

    // Attributes
    ctx.create("env_attrib", nsi::ATTRIBUTES, None);
    ctx.connect(
        "env_attrib",
        None,
        "environment",
        "geometryattributes",
        None,
    );
    ctx.set_attribute(
        "env_attrib",
        &[nsi::i32!("visibility.camera", 0)], // Not visible to camera
    );

    // Shader
    ctx.create("env_shader", nsi::SHADER, None);
    ctx.connect("env_shader", None, "env_attrib", "surfaceshader", None);

    ctx.set_attribute(
        "env_shader",
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/environmentLight"),
            nsi::f32!("intensity", intensity),
            nsi::color!("i_color", color),
        ],
    );
}

/// Add a ground plane.
pub fn add_ground_plane(ctx: &nsi::Context, y_position: f64) {
    // Transform
    ctx.create("ground_xform", nsi::TRANSFORM, None);
    ctx.connect("ground_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "ground_xform",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., y_position,
                0., 1.
            ]
        )],
    );

    // Plane
    ctx.create("ground", nsi::PLANE, None);
    ctx.connect("ground", None, "ground_xform", "objects", None);

    // Material
    add_diffuse_material(ctx, "ground", &[0.8, 0.8, 0.8], 0.5);
}
