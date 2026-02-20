//! Material rendering tests for NSI.

use nsi_ffi_wrap as nsi;

mod common;
mod test_utils;

use test_utils::assert_render_test;

#[test]
fn diffuse() {
    assert_render_test("material_diffuse", |ctx| {
        // Create three spheres with different roughness values
        common::add_test_sphere(ctx, "sphere1", &[-2.0, 0.0, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere1", &[0.8, 0.3, 0.3], 0.0);

        common::add_test_sphere(ctx, "sphere2", &[0.0, 0.0, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere2", &[0.3, 0.8, 0.3], 0.5);

        common::add_test_sphere(ctx, "sphere3", &[2.0, 0.0, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere3", &[0.3, 0.3, 0.8], 1.0);

        // Add ground
        common::add_ground_plane(ctx, -1.0);

        // Add lighting
        common::add_area_light(ctx, "light1", &[0.0, 4.0, 2.0], 3.0, 100.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.3);
    });
}

#[test]
fn metal() {
    assert_render_test("material_metal", |ctx| {
        // Create metallic spheres with different roughness
        common::add_test_sphere(ctx, "sphere1", &[-2.0, 0.0, 0.0], 0.8);
        common::add_metal_material(ctx, "sphere1", &[0.9, 0.9, 0.9], 0.0);

        common::add_test_sphere(ctx, "sphere2", &[0.0, 0.0, 0.0], 0.8);
        common::add_metal_material(ctx, "sphere2", &[0.9, 0.7, 0.3], 0.1);

        common::add_test_sphere(ctx, "sphere3", &[2.0, 0.0, 0.0], 0.8);
        common::add_metal_material(ctx, "sphere3", &[0.6, 0.3, 0.1], 0.3);

        // Add ground
        common::add_ground_plane(ctx, -1.0);

        // Add lighting
        common::add_area_light(ctx, "light1", &[0.0, 4.0, 2.0], 3.0, 100.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.3);
    });
}

#[test]
fn glass() {
    assert_render_test("material_glass", |ctx| {
        // Create glass sphere
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 1.0);

        // Glass material
        let attrib_name = "sphere1_attrib";
        let shader_name = "sphere1_shader";

        ctx.create(attrib_name, nsi::ATTRIBUTES, None);
        ctx.connect(attrib_name, None, "sphere1", "geometryattributes", None);

        ctx.create(shader_name, nsi::SHADER, None);
        ctx.connect(shader_name, None, attrib_name, "surfaceshader", None);

        ctx.set_attribute(
            shader_name,
            &[
                nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
                nsi::color!("i_color", &[1.0, 1.0, 1.0]),
                nsi::float!("roughness", 0.0),
                nsi::float!("specular_level", 1.0),
                nsi::float!("metallic", 0.0),
                nsi::float!("glass", 1.0),
                nsi::float!("glass_ior", 1.5),
            ],
        );

        // Add some objects behind to see refraction
        common::add_test_sphere(ctx, "sphere2", &[-1.5, 0.0, -3.0], 0.5);
        common::add_diffuse_material(ctx, "sphere2", &[0.8, 0.2, 0.2], 0.2);

        common::add_test_sphere(ctx, "sphere3", &[1.5, 0.0, -3.0], 0.5);
        common::add_diffuse_material(ctx, "sphere3", &[0.2, 0.2, 0.8], 0.2);

        // Add ground
        common::add_ground_plane(ctx, -1.5);

        // Add lighting
        common::add_area_light(ctx, "light1", &[0.0, 4.0, 2.0], 3.0, 150.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.5);
    });
}

#[test]
fn emissive() {
    assert_render_test("material_emissive", |ctx| {
        // Create emissive sphere
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 0.8);

        // Emissive material
        let attrib_name = "sphere1_attrib";
        let shader_name = "sphere1_shader";

        ctx.create(attrib_name, nsi::ATTRIBUTES, None);
        ctx.connect(attrib_name, None, "sphere1", "geometryattributes", None);

        ctx.create(shader_name, nsi::SHADER, None);
        ctx.connect(shader_name, None, attrib_name, "surfaceshader", None);

        ctx.set_attribute(
            shader_name,
            &[
                nsi::string!("shaderfilename", "${DELIGHT}/osl/emitter"),
                nsi::float!("intensity", 10.0),
                nsi::color!("tint", &[1.0, 0.8, 0.5]),
            ],
        );

        // Add other spheres to be lit
        common::add_test_sphere(ctx, "sphere2", &[-2.0, 0.0, 0.0], 0.6);
        common::add_diffuse_material(ctx, "sphere2", &[0.8, 0.8, 0.8], 0.3);

        common::add_test_sphere(ctx, "sphere3", &[2.0, 0.0, 0.0], 0.6);
        common::add_metal_material(ctx, "sphere3", &[0.9, 0.9, 0.9], 0.05);

        // Add ground
        common::add_ground_plane(ctx, -1.0);

        // Very dim environment light
        common::add_constant_environment(ctx, &[0.02, 0.02, 0.03], 0.1);
    });
}

#[test]
fn anisotropic() {
    assert_render_test("material_anisotropic", |ctx| {
        // Create sphere with anisotropic material
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 1.2);

        // Anisotropic metal material
        let attrib_name = "sphere1_attrib";
        let shader_name = "sphere1_shader";

        ctx.create(attrib_name, nsi::ATTRIBUTES, None);
        ctx.connect(attrib_name, None, "sphere1", "geometryattributes", None);

        ctx.create(shader_name, nsi::SHADER, None);
        ctx.connect(shader_name, None, attrib_name, "surfaceshader", None);

        ctx.set_attribute(
            shader_name,
            &[
                nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
                nsi::color!("i_color", &[0.9, 0.7, 0.3]),
                nsi::float!("roughness", 0.3),
                nsi::float!("specular_level", 1.0),
                nsi::float!("metallic", 1.0),
                nsi::float!("anisotropy", 0.8),
                nsi::color!("anisotropy_direction", &[1., 0., 0.]),
            ],
        );

        // Add ground
        common::add_ground_plane(ctx, -1.5);

        // Add lighting
        common::add_area_light(ctx, "light1", &[2.0, 3.0, 2.0], 2.0, 100.0);
        common::add_area_light(ctx, "light2", &[-2.0, 3.0, 2.0], 2.0, 100.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.3);
    });
}

#[test]
fn subsurface_scattering() {
    assert_render_test("material_sss", |ctx| {
        // Create sphere with subsurface scattering
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 1.0);

        // SSS material (like wax or skin)
        let attrib_name = "sphere1_attrib";
        let shader_name = "sphere1_shader";

        ctx.create(attrib_name, nsi::ATTRIBUTES, None);
        ctx.connect(attrib_name, None, "sphere1", "geometryattributes", None);

        ctx.create(shader_name, nsi::SHADER, None);
        ctx.connect(shader_name, None, attrib_name, "surfaceshader", None);

        ctx.set_attribute(
            shader_name,
            &[
                nsi::string!("shaderfilename", "${DELIGHT}/osl/dlPrincipled"),
                nsi::color!("i_color", &[0.9, 0.7, 0.6]),
                nsi::float!("roughness", 0.4),
                nsi::float!("specular_level", 0.5),
                nsi::float!("metallic", 0.0),
                nsi::float!("sss_weight", 1.0),
                nsi::color!("sss_color", &[0.8, 0.4, 0.3]),
                nsi::float!("sss_scale", 0.5),
            ],
        );

        // Add additional objects
        common::add_test_sphere(ctx, "sphere2", &[-2.0, 0.0, 0.0], 0.6);
        common::add_diffuse_material(ctx, "sphere2", &[0.8, 0.8, 0.8], 0.3);

        common::add_test_sphere(ctx, "sphere3", &[2.0, 0.0, 0.0], 0.6);
        common::add_metal_material(ctx, "sphere3", &[0.9, 0.9, 0.9], 0.1);

        // Add ground
        common::add_ground_plane(ctx, -1.5);

        // Add lighting - strong backlight to show SSS
        common::add_area_light(ctx, "light1", &[0.0, 2.0, -4.0], 3.0, 200.0);
        common::add_area_light(ctx, "light2", &[0.0, 3.0, 2.0], 2.0, 50.0);
        common::add_constant_environment(ctx, &[0.05, 0.05, 0.08], 0.2);
    });
}
