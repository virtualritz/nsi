//! Test to verify the callback memory management fix

use nsi_ffi_wrap as nsi;
use std::sync::{Arc, Mutex};

#[test]
fn simple_callback() {
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    {
        let ctx =
            nsi::Context::new(None).expect("Could not create NSI context");

        // Minimal scene - need camera for valid output
        ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
        ctx.connect("camera", None, nsi::ROOT, "objects", None);

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

        // Test finish callback - note: FnFinish no longer receives pixel data
        let finish = nsi::output::FinishCallback::new(
            move |_name: String,
                  _width: usize,
                  _height: usize,
                  _fmt: nsi::output::PixelFormat| {
                *counter_clone.lock().unwrap() += 1;
                println!("Finish callback called!");
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
                nsi::callback!("callback.finish", finish),
            ],
        );

        println!("Starting render...");
        ctx.render_control(nsi::Action::Start, None);
        ctx.render_control(nsi::Action::Wait, None);
        println!("Render complete");
    }

    // Check callback was called
    let count = *counter.lock().unwrap();
    println!("Callback was called {} times", count);
    assert!(count > 0, "Callback was not called");
}
