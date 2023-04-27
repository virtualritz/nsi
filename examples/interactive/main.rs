//! Demonstrates using the render_control() "callback" parameter to have the
//! renderer call a closure when it is done.
use nsi_core as nsi;

fn main() {
    let ctx = nsi::Context::new(None).unwrap();

    let status_callback = nsi::StatusCallback::new(
        |_ctx: &nsi::Context, status: nsi::RenderStatus| {
            println!("Status: {:?}", status);
        },
    );

    // The renderer will abort because we didn't define an output driver.
    // So our status_callback() above will receive RenderStatus::Aborted.
    ctx.render_control(
        nsi::Action::Start,
        Some(&[
            nsi::integer!("interactive", true as _),
            nsi::callback!("callback", status_callback),
        ]),
    );

    // Block until the renderer is really done.
    ctx.render_control(nsi::Action::Wait, None);
}
