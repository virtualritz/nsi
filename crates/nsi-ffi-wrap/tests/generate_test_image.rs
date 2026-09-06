//! Helper to generate a test image.
//! Run with:
//!
//! ```text
//! cargo test --test generate_test_image -- --ignored --nocapture
//! ```
//!
//! `#[ignore]`, because this is a **tool**, not a test: it sets
//! `RUST_TEST_UPDATE` and overwrites the checked-in
//! `tests/expected_images/sphere.png` with whatever it just rendered.
//! Running by default made `geometry::sphere` unable to fail -- the
//! expectation was rewritten from the same renderer it was compared
//! against -- and worse, a misconfigured renderer wrote its failure
//! into the fixture: an 829-byte blank replaced the 13,935-byte
//! golden, and the next run compared against *that*. Regenerate
//! deliberately, look at the image, and commit it on purpose.

// Renders through the pixel-streaming API, which lives behind
// `output`; `test_utils` is gated the same way, so this target cannot
// build without it.
#![cfg(feature = "output")]

mod common;
mod test_utils;

#[test]
#[ignore = "a golden-image generator: it overwrites the checked-in expectation"]
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
