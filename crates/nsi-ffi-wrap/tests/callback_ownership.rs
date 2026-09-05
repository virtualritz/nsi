//! A callback handed to the renderer must not be leaked forever.
//!
//! Each callback is a closure, and closures capture: an output-driver write
//! callback typically holds an `Arc` to the caller's pixel buffer. Leaking
//! the closure therefore pins that buffer for the life of the process, and
//! a consumer that re-sets callbacks on a long-lived context (switching
//! output modes, say) leaks a set every time.
//!
//! These tests capture an `Arc` in the callback and watch it through a
//! `Weak`: if the callback is reclaimed, the `Weak` dies.

use nsi_ffi_wrap as nsi;
use std::sync::{Arc, Weak};

/// Builds the minimum scene an output driver needs to be attached to.
fn scene(ctx: &nsi::Context) {
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, nsi::ROOT, "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.connect("beauty", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);
}

/// Sets a write callback that owns a clone of `payload`.
fn set_write_callback(ctx: &nsi::Context, payload: Arc<()>) {
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
            // Only here to own `payload`.
            let _keep = &payload;
            nsi::output::Error::None
        },
    );
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", nsi::output::FERRIS_F32),
            nsi::callback!("callback.write", write),
        ],
    );
}

#[test]
fn dropping_the_context_reclaims_its_callbacks() {
    let observed: Weak<()>;
    {
        let ctx = nsi::Context::new(None).expect("could not create context");
        scene(&ctx);

        let payload = Arc::new(());
        observed = Arc::downgrade(&payload);
        set_write_callback(&ctx, payload);

        assert!(
            observed.upgrade().is_some(),
            "the callback must still own the payload while the context lives"
        );
    }

    assert!(
        observed.upgrade().is_none(),
        "dropping the context must reclaim its callbacks; the closure -- and \
         everything it captured, which for a real driver is the pixel buffer \
         -- is otherwise leaked for the life of the process"
    );
}

#[test]
fn replacing_a_callback_reclaims_the_previous_one() {
    let ctx = nsi::Context::new(None).expect("could not create context");
    scene(&ctx);

    let first = Arc::new(());
    let observed = Arc::downgrade(&first);
    set_write_callback(&ctx, first);

    // Replace it, exactly as a consumer switching output modes does.
    set_write_callback(&ctx, Arc::new(()));

    // No render is in flight, so the displaced callback is unreachable.
    ctx.render_control(nsi::Action::Stop, None);
    ctx.render_control(nsi::Action::Wait, None);

    assert!(
        observed.upgrade().is_none(),
        "re-setting an attribute must reclaim the callback it displaced; \
         otherwise a long-lived context leaks a closure per switch"
    );
}
