//! Demonstrates using the render_control() "callback" parameter to have the
//! renderer call a closure when it is done and using a channel block the main
//! thread from exiting while the render is still running.
use nsi_ffi_wrap as nsi;

fn main() {
    let ctx = nsi::Context::new(None).unwrap();

    // Create a channel to communicate between the main thread and the render
    // thread.
    let (sender, receiver) = std::sync::mpsc::channel();

    let status_callback = nsi::StatusCallback::new(
        |_ctx: &nsi::Context, status: nsi::RenderStatus| {
            println!("Status: {:?}", status);

            // Send the status through our channel.
            sender.send(status).unwrap();
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
    // This is an alternative to using
    // ctx.render_control(nsi::Action::Wait, None);
    loop {
        // Check for messages from the status_callback above.
        match receiver.recv().unwrap() {
            nsi::RenderStatus::Aborted | nsi::RenderStatus::Completed => break,
            _ => (),
        }
    }
}
