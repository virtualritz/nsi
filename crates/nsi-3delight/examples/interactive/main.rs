//! Demonstrates using the render_control() "callback" parameter to have the
//! renderer call a closure when it is done.
use nsi_core as nsi;

fn main() {
    let ctx = nsi::Context::new(&[]).unwrap();

    let status_callback = nsi::context::StatusCallback::new(
        |_ctx: &nsi::Context, status: nsi::context::RenderStatus| {
            println!("Status: {:?}", status);
        },
    );

    // The renderer will abort because we didn't define an output driver.
    // So our status_callback() above will receive RenderStatus::Aborted.
    ctx.render_control(&[
        nsi::string!("action", "start"),
        nsi::integer!("interactive", true as _),
        nsi::callback!("callback", status_callback),
    ]);

    // Block untile the renderer is really done.
    ctx.render_control(&[nsi::string!("action", "wait")]);
}
