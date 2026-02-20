//! Geometry rendering tests for NSI.

use bytemuck;
use nsi_ffi_wrap as nsi;

mod common;
mod test_utils;

use test_utils::assert_render_test;

#[test]
fn sphere() {
    assert_render_test("sphere", |ctx| {
        // Add a simple sphere
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 1.5);
        common::add_diffuse_material(ctx, "sphere1", &[0.8, 0.3, 0.3], 0.2);

        // Add lighting
        common::add_area_light(ctx, "light1", &[3.0, 3.0, 3.0], 2.0, 50.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.2], 0.5);
    });
}

#[test]
fn multiple_spheres() {
    assert_render_test("multiple_spheres", |ctx| {
        // Three spheres with different materials
        common::add_test_sphere(ctx, "sphere1", &[-2.0, 0.0, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere1", &[0.8, 0.2, 0.2], 0.1);

        common::add_test_sphere(ctx, "sphere2", &[0.0, 0.0, 0.0], 0.8);
        common::add_metal_material(ctx, "sphere2", &[0.8, 0.8, 0.8], 0.05);

        common::add_test_sphere(ctx, "sphere3", &[2.0, 0.0, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere3", &[0.2, 0.2, 0.8], 0.5);

        // Add ground plane
        common::add_ground_plane(ctx, -1.0);

        // Add lighting
        common::add_area_light(ctx, "light1", &[0.0, 4.0, 2.0], 3.0, 100.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.3);
    });
}

#[test]
fn cube() {
    assert_render_test("cube", |ctx| {
        // Create a cube
        ctx.create("cube", nsi::MESH, None);
        ctx.connect("cube", None, nsi::ROOT, "objects", None);

        let positions = [
            -1., -1., -1., 1., -1., -1., 1., 1., -1., -1., 1., -1., -1., -1.,
            1., 1., -1., 1., 1., 1., 1., -1., 1., 1.,
        ];

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
            "cube",
            &[
                nsi::points!("P", points),
                nsi::integers!("P.indices", &face_indices),
                nsi::integers!("nvertices", &[4; 6]),
            ],
        );

        common::add_diffuse_material(ctx, "cube", &[0.6, 0.6, 0.2], 0.3);

        // Add lighting
        common::add_area_light(ctx, "light1", &[3.0, 3.0, 3.0], 2.0, 80.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.4);
    });
}

#[test]
fn dodecahedron() {
    assert_render_test("dodecahedron", |ctx| {
        // Create a dodecahedron
        let face_index: [i32; 60] = [
            0, 16, 2, 10, 8, 0, 8, 4, 14, 12, 16, 17, 1, 12, 0, 1, 9, 11, 3,
            17, 1, 12, 14, 5, 9, 2, 13, 15, 6, 10, 13, 3, 17, 16, 2, 3, 11, 7,
            15, 13, 4, 8, 10, 6, 18, 14, 5, 19, 18, 4, 5, 19, 7, 11, 9, 15, 7,
            19, 18, 6,
        ];

        let positions = [
            1., 1., 1., 1., 1., -1., 1., -1., 1., 1., -1., -1., -1., 1., 1.,
            -1., 1., -1., -1., -1., 1., -1., -1., -1., 0., 0.618, 1.618, 0.,
            0.618, -1.618, 0., -0.618, 1.618, 0., -0.618, -1.618, 0.618, 1.618,
            0., 0.618, -1.618, 0., -0.618, 1.618, 0., -0.618, -1.618, 0.,
            1.618, 0., 0.618, 1.618, 0., -0.618, -1.618, 0., 0.618, -1.618, 0.,
            -0.618,
        ];

        ctx.create("dodecahedron", nsi::MESH, None);
        ctx.connect("dodecahedron", None, nsi::ROOT, "objects", None);

        let points: &[[f32; 3]] = bytemuck::cast_slice(&positions);

        ctx.set_attribute(
            "dodecahedron",
            &[
                nsi::points!("P", points),
                nsi::integers!("P.indices", &face_index),
                nsi::integers!("nvertices", &[5; 12]),
                nsi::string!("subdivision.scheme", "catmull-clark"),
                nsi::integer!("subdivision.level", 2),
            ],
        );

        common::add_metal_material(ctx, "dodecahedron", &[0.9, 0.7, 0.3], 0.1);

        // Add lighting
        common::add_area_light(ctx, "light1", &[3.0, 3.0, 3.0], 2.0, 80.0);
        common::add_constant_environment(ctx, &[0.05, 0.05, 0.1], 0.3);
    });
}

#[test]
fn plane() {
    assert_render_test("plane", |ctx| {
        // Add a ground plane at origin
        common::add_ground_plane(ctx, 0.0);

        // Add a few spheres on the plane
        common::add_test_sphere(ctx, "sphere1", &[-1.0, 0.8, 0.0], 0.8);
        common::add_diffuse_material(ctx, "sphere1", &[0.8, 0.3, 0.3], 0.2);

        common::add_test_sphere(ctx, "sphere2", &[1.0, 0.8, 0.0], 0.8);
        common::add_metal_material(ctx, "sphere2", &[0.8, 0.8, 0.8], 0.02);

        // Add lighting
        common::add_area_light(ctx, "light1", &[0.0, 4.0, 2.0], 3.0, 100.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.2);
    });
}

#[test]
fn subdivision_surface() {
    assert_render_test("subdivision_surface", |ctx| {
        // Create a simple control mesh
        ctx.create("subdiv_mesh", nsi::MESH, None);
        ctx.connect("subdiv_mesh", None, nsi::ROOT, "objects", None);

        // Pyramid-like control mesh
        let positions = [
            -1., 0., -1., 1., 0., -1., 1., 0., 1., -1., 0., 1., 0., 2., 0.,
        ];

        let face_indices: [i32; 20] = [
            0, 1, 2, 3, // base
            0, 4, 1, 1, // side 1 (degenerate quad)
            1, 4, 2, 2, // side 2 (degenerate quad)
            2, 4, 3, 3, // side 3 (degenerate quad)
            3, 4, 0, 0, // side 4 (degenerate quad)
        ];

        let points: &[[f32; 3]] = bytemuck::cast_slice(&positions);

        ctx.set_attribute(
            "subdiv_mesh",
            &[
                nsi::points!("P", points),
                nsi::integers!("P.indices", &face_indices),
                nsi::integers!("nvertices", &[4, 3, 3, 3, 3]),
                nsi::string!("subdivision.scheme", "catmull-clark"),
                nsi::integer!("subdivision.level", 3),
            ],
        );

        common::add_diffuse_material(ctx, "subdiv_mesh", &[0.3, 0.6, 0.8], 0.2);

        // Add lighting
        common::add_area_light(ctx, "light1", &[2.0, 4.0, 3.0], 2.0, 80.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.15], 0.4);
    });
}
