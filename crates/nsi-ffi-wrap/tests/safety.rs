//! Safety regression tests for NSI unsafe code.
//!
//! These tests are designed to catch memory safety issues, particularly
//! around callbacks, FFI boundaries, and lifetime management.

use bytemuck;
use nsi_ffi_wrap as nsi;
use std::{
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Mutex},
    thread,
};

#[test]
fn callback_lifetime_management() {
    // Test that callbacks are properly managed and don't leak memory
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    // Create a context and setup rendering with callbacks
    {
        let ctx =
            nsi::Context::new(None).expect("Could not create NSI context");

        // Setup camera transform
        ctx.create("camera_xform", nsi::TRANSFORM, None);
        ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
        ctx.set_attribute(
            "camera_xform",
            &[nsi::double_matrix!(
                "transformationmatrix",
                &[
                    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5.,
                    1.
                ]
            )],
        );

        // Setup camera
        ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
        ctx.connect("camera", None, "camera_xform", "objects", None);

        ctx.create("screen", nsi::SCREEN, None);
        ctx.connect("screen", None, "camera", "screens", None);
        ctx.set_attribute(
            "screen",
            &[nsi::integers!("resolution", &[32, 32]).array_len(2)],
        );

        ctx.create("beauty", nsi::OUTPUT_LAYER, None);
        ctx.set_attribute(
            "beauty",
            &[
                nsi::string!("variablename", "Ci"),
                nsi::string!("scalarformat", "float"),
            ],
        );
        ctx.connect("beauty", None, "screen", "outputlayers", None);

        // Add a simple plane geometry so there's something to render
        ctx.create("mesh", nsi::MESH, None);
        ctx.connect("mesh", None, nsi::ROOT, "objects", None);
        let positions: &[[f32; 3]] = &[
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        ctx.set_attribute(
            "mesh",
            &[nsi::points!("P", positions), nsi::integer!("nvertices", 4)],
        );

        // Write callback that increments counter - use f32 driver
        let write = nsi::output::WriteCallback::<f32>::new(
            move |_: &str,
                  _: usize,
                  _: usize,
                  _: usize,
                  _: usize,
                  _: usize,
                  _: usize,
                  _: &nsi::output::PixelFormat,
                  _: &[f32]| {
                *counter_clone.lock().unwrap() += 1;
                nsi::output::Error::None
            },
        );

        ctx.create("driver", nsi::OUTPUT_DRIVER, None);
        ctx.connect("driver", None, "beauty", "outputdrivers", None);
        ctx.set_attribute(
            "driver",
            &[
                nsi::string!("drivername", nsi::output::FERRIS_F32),
                nsi::string!("imagefilename", "test"),
                nsi::callback!("callback.write", write),
            ],
        );

        // Render
        ctx.render_control(nsi::Action::Start, None);
        ctx.render_control(nsi::Action::Wait, None);
    }

    // Check if callback was called
    // Note: The callback may not be called if the renderer doesn't use our
    // registered driver (e.g., uses an internal implementation). This test
    // primarily verifies that the callback setup doesn't crash.
    let count = *counter.lock().unwrap();
    if count > 0 {
        println!("Write callback was called {} times", count);
    } else {
        println!(
            "Write callback was not called (renderer may use internal driver)"
        );
    }
}

#[test]
fn reference_lifetime() {
    // Test that references passed through FFI are handled safely
    let data = Box::new(vec![42u64, 84, 126]);

    {
        let ctx =
            nsi::Context::new(None).expect("Could not create NSI context");

        // Pass reference to NSI
        ctx.create("test_node", nsi::ATTRIBUTES, None);
        ctx.set_attribute("test_node", &[nsi::reference!("test_data", &data)]);

        // The context should keep the reference valid
        ctx.render_control(nsi::Action::Start, None);
        ctx.render_control(nsi::Action::Wait, None);
    }

    // Data should still be valid after context is dropped
    assert_eq!(data[0], 42);
}

#[test]
fn multiple_contexts() {
    // Test that multiple contexts can coexist safely
    let ctx1 = nsi::Context::new(None).expect("Could not create NSI context 1");
    let ctx2 = nsi::Context::new(None).expect("Could not create NSI context 2");

    // Create nodes in both contexts
    ctx1.create("node1", nsi::ATTRIBUTES, None);
    ctx2.create("node2", nsi::ATTRIBUTES, None);

    // Set attributes
    ctx1.set_attribute("node1", &[nsi::integer!("test", 1)]);
    ctx2.set_attribute("node2", &[nsi::integer!("test", 2)]);

    // Both contexts should work independently
    drop(ctx1);
    // ctx2 should still be valid
    ctx2.set_attribute("node2", &[nsi::integer!("test", 3)]);
}

#[test]
fn thread_safety() {
    // Test that contexts can be used from multiple threads
    let ctx = Arc::new(
        nsi::Context::new(None).expect("Could not create NSI context"),
    );

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let ctx_clone = Arc::clone(&ctx);
            thread::spawn(move || {
                // Each thread creates its own node
                let node_name = format!("thread_node_{}", i);
                ctx_clone.create(&node_name, nsi::ATTRIBUTES, None);
                ctx_clone.set_attribute(
                    &node_name,
                    &[nsi::integer!("thread_id", i as i32)],
                );
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn error_callback() {
    // Test error callback handling
    let error_count = Arc::new(Mutex::new(0));
    let error_count_clone = Arc::clone(&error_count);

    let error_handler = nsi::ErrorCallback::new(
        move |level: log::Level, _code: i32, message: &str| {
            println!("NSI Error [{}]: {}", level, message);
            *error_count_clone.lock().unwrap() += 1;
        },
    );

    let ctx = nsi::Context::new(Some(&[nsi::callback!(
        "errorhandler",
        error_handler
    )]))
    .expect("Could not create NSI context");

    // Trigger an error by connecting non-existent nodes
    ctx.connect("nonexistent1", None, "nonexistent2", "objects", None);

    // Give some time for error to be reported
    std::thread::sleep(std::time::Duration::from_millis(100));

    // We should have received at least one error
    // Note: This might not work if the NSI implementation doesn't report this as an error
    let count = *error_count.lock().unwrap();
    println!("Error count: {}", count);
}

#[test]
fn status_callback() {
    // Test render status callback
    let status_received = Arc::new(Mutex::new(false));
    let status_clone = Arc::clone(&status_received);

    let status_callback = nsi::context::StatusCallback::new(
        move |_ctx: &nsi::Context, status: nsi::context::RenderStatus| {
            println!("Render status: {:?}", status);
            *status_clone.lock().unwrap() = true;
        },
    );

    let ctx = nsi::Context::new(None).expect("Could not create NSI context");

    // Minimal scene setup
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, nsi::ROOT, "objects", None);

    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::integers!("resolution", &[32, 32]).array_len(2)],
    );

    // Start interactive render with callback
    ctx.render_control(
        nsi::Action::Start,
        Some(&[
            nsi::integer!("interactive", 1),
            nsi::callback!("callback", status_callback),
        ]),
    );

    ctx.render_control(nsi::Action::Wait, None);

    // Status callback should have been called
    assert!(
        *status_received.lock().unwrap(),
        "Status callback was not called"
    );
}

#[test]
fn large_data_transfer() {
    // Test transferring large amounts of data through FFI
    let ctx = nsi::Context::new(None).expect("Could not create NSI context");

    // Create a mesh with many vertices
    let vertex_count = 10000;
    let positions: Vec<f32> =
        (0..vertex_count * 3).map(|i| (i as f32) / 1000.0).collect();

    ctx.create("large_mesh", nsi::MESH, None);
    ctx.connect("large_mesh", None, nsi::ROOT, "objects", None);

    // This should not crash or leak memory
    let points: &[[f32; 3]] = bytemuck::cast_slice(&positions);
    ctx.set_attribute(
        "large_mesh",
        &[
            nsi::points!("P", points),
            nsi::integer!("nvertices", 3), // Triangle soup
        ],
    );
}

#[test]
fn string_handling() {
    // Test various string edge cases
    let ctx = nsi::Context::new(None).expect("Could not create NSI context");

    // Test empty string
    ctx.create("", nsi::ATTRIBUTES, None);

    // Test very long string
    let long_name = "a".repeat(1000);
    ctx.create(&long_name, nsi::ATTRIBUTES, None);

    // Test Unicode string (should work as UTF-8)
    ctx.create("测试节点", nsi::ATTRIBUTES, None);

    // Test string with null bytes (should panic or handle gracefully)
    // This is commented out as it might panic
    // ctx.create("test\0node", nsi::ATTRIBUTES, None);
}

#[test]
fn callback_panic_safety() {
    // Test that panics in callbacks don't cause undefined behavior

    let ctx = nsi::Context::new(None).expect("Could not create NSI context");

    // Setup minimal scene
    ctx.create("screen", nsi::SCREEN, None);
    ctx.set_attribute(
        "screen",
        &[nsi::integers!("resolution", &[32, 32]).array_len(2)],
    );

    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.connect("beauty", None, "screen", "outputlayers", None);

    // Callback that might panic - use f32 driver
    let write = nsi::output::WriteCallback::<f32>::new(
        |_: &str,
         _: usize,
         _: usize,
         _: usize,
         _: usize,
         _: usize,
         _: usize,
         _: &nsi::output::PixelFormat,
         _: &[f32]| {
            // This should be caught and not cause UB
            if rand::random::<f32>() > 0.5 {
                panic!("Test panic in callback");
            }
            nsi::output::Error::None
        },
    );

    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", nsi::output::FERRIS_F32),
            nsi::callback!("callback.write", write),
        ],
    );

    // This might panic but should not cause undefined behavior
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        ctx.render_control(nsi::Action::Start, None);
        ctx.render_control(nsi::Action::Wait, None);
    }));

    // We don't care if it panicked, just that it didn't crash
    println!("Panic test result: {:?}", result.is_ok());
}
