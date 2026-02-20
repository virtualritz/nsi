# NSI Test Infrastructure

This directory contains comprehensive tests for the NSI (Nodal Scene Interface) crate, including image-based regression tests and safety tests for unsafe code.

## Overview

The test suite includes:

- **Geometry Tests** (`geometry.rs`) - Tests for various geometric primitives
- **Material Tests** (`materials.rs`) - Tests for different material types
- **Safety Tests** (`safety.rs`) - Regression tests for unsafe code and FFI boundaries
- **Test Utilities** (`test_utils.rs`) - Infrastructure for image-based testing
- **Common Helpers** (`common/mod.rs`) - Shared scene setup utilities

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suite

```bash
cargo test geometry
cargo test materials
cargo test safety
```

### Update Expected Images

When you make intentional changes that affect the rendered output:

```bash
RUST_TEST_UPDATE=1 cargo test
```

This will update the images in `tests/expected_images/`.

### Run a Single Test

```bash
cargo test test_sphere -- --nocapture
```

## Test Infrastructure

### Image-Based Tests

The test infrastructure renders scenes and compares the output with expected images:

1. Each test renders a scene at 320x240 resolution
2. The output is saved to `target/test_images/`
3. The output is compared with expected images in `tests/expected_images/`
4. Tests fail if images differ by more than 0.1%

### Safety Tests

Safety tests ensure that unsafe code is handled correctly:

- Callback lifetime management
- Reference passing through FFI
- Thread safety
- Error handling
- Panic safety at FFI boundaries

## Adding New Tests

### Adding a Geometry Test

```rust
#[test]
fn test_my_geometry() {
    assert_render_test("my_geometry", |ctx| {
        // Setup your scene here
        common::add_test_sphere(ctx, "sphere", &[0.0, 0.0, 0.0], 1.0);
        common::add_diffuse_material(ctx, "sphere", &[0.8, 0.3, 0.3], 0.2);

        // Add lighting
        common::add_area_light(ctx, "light", &[3.0, 3.0, 3.0], 2.0, 50.0);
    });
}
```

### Adding a Safety Test

```rust
#[test]
fn test_my_safety_feature() {
    let ctx = nsi::Context::new(None).expect("Could not create context");

    // Test unsafe operations
    // Verify they don't cause undefined behavior
}
```

## Common Helpers

The `common` module provides utilities for:

- `add_test_sphere()` - Add a sphere with subdivision
- `add_diffuse_material()` - Add a basic diffuse material
- `add_metal_material()` - Add a metallic material
- `add_area_light()` - Add an area light
- `add_constant_environment()` - Add environment lighting
- `add_ground_plane()` - Add a ground plane

## CI Integration

The tests are designed to run in CI environments:

- Tests use small resolutions (320x240) for speed
- Expected images are stored in the repository
- Failed tests generate difference images
- Test outputs are saved as CI artifacts

## Requirements

- 3Delight renderer must be installed
- Set `DELIGHT` environment variable if not in default location
- PNG and EXR libraries for image I/O

## Troubleshooting

### Tests Fail with "Expected image not found"

Run with `RUST_TEST_UPDATE=1` to generate expected images.

### Tests Fail with "Failed to create NSI context"

Ensure 3Delight is installed and the `DELIGHT` environment variable is set correctly.

### Tests Pass Locally but Fail in CI

Check that:

- 3Delight version matches between environments
- Expected images are committed to the repository
- Platform-specific rendering differences are acceptable
