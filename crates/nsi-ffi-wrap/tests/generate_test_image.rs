//! Helper to generate a test image.
//! Run with: cargo test --test generate_test_image -- --nocapture

use nsi_ffi_wrap as nsi;

mod common;
mod test_utils;

#[test]
fn sphere_generation() {
    // Set update mode to generate the expected image
    // SAFETY: This test is single-threaded and no other thread reads this var.
    unsafe { std::env::set_var("RUST_TEST_UPDATE", "1") };

    test_utils::assert_render_test("sphere", |ctx| {
        // Add a simple sphere
        common::add_test_sphere(ctx, "sphere1", &[0.0, 0.0, 0.0], 1.5);
        common::add_diffuse_material(ctx, "sphere1", &[0.8, 0.3, 0.3], 0.2);

        // Add lighting
        common::add_area_light(ctx, "light1", &[3.0, 3.0, 3.0], 2.0, 50.0);
        common::add_constant_environment(ctx, &[0.1, 0.1, 0.2], 0.5);
    });

    println!("Generated test image: tests/expected_images/sphere.png");
}
