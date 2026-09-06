//! Test to verify the callback memory management fix

// The renderer's pixel-streaming API lives behind `output`, so this
// file is empty without it. Without the gate the whole test target
// failed to compile for any configuration that did not happen to
// enable the feature -- which is every configuration `--all-features`
// is not.
#![cfg(feature = "output")]

use nsi_ffi_wrap as nsi;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

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
            &[nsi::i32_slice!("resolution", &[32, 32])
                .array_len(const { NonZeroUsize::new(2).unwrap() })],
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
