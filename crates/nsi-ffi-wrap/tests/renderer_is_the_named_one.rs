//! That naming a renderer reaches *that* renderer.
//!
//! `backend_selection.rs` checks a context comes back. That is not the
//! same as checking which library made it: a cache that returned the
//! default for every name would pass every one of those tests and be
//! completely wrong.
//!
//! So this asks the MoonRay backend to do something only it does. It
//! is a file of its own because it sets an environment variable, and
//! the tests in one file share a process.

use nsi_ffi_wrap as nsi;
use nsi_ffi_wrap::backend;

#[test]
fn a_moonray_context_is_moonray() {
    let installed = backend::candidates("moonray")
        .into_iter()
        .any(|path| path.is_file());

    if !installed {
        eprintln!(
            "no MoonRay backend on this machine; set $NSI_MOONRAY to an \
             install prefix to run this"
        );
        return;
    }

    let dump = std::env::temp_dir().join("nsi-is-really-moonray.rdla");
    let _ = std::fs::remove_file(&dump);

    // `$NSI_MOONRAY_SCENE` makes the MoonRay backend write the rdl2
    // scene it built. No other ɴsɪ renderer has heard of it, so the
    // file appearing is the proof.
    //
    // SAFETY: this test file holds one test, so nothing else in this
    // process reads the environment concurrently.
    unsafe { std::env::set_var("NSI_MOONRAY_SCENE", &dump) };

    {
        let context =
            nsi::Context::new(Some(&[nsi::string!("renderer", "moonray")]))
                .expect("the MoonRay backend is installed");

        context.create("floor", nsi::MESH, None);
        context.connect("floor", None, nsi::ROOT, "objects", None);
        context.render_control(nsi::Action::Start, None);
        context.render_control(nsi::Action::Wait, None);
    }

    unsafe { std::env::remove_var("NSI_MOONRAY_SCENE") };

    let written = std::fs::read_to_string(&dump).unwrap_or_default();
    let _ = std::fs::remove_file(&dump);

    assert!(
        !written.is_empty(),
        "the MoonRay backend should have written its rdl2 scene; either \
         the \"renderer\" argument did not reach it, or something else \
         answered to the name"
    );
    // rdl2's own vocabulary, which an ɴsɪ stream does not share.
    assert!(
        written.contains("SceneVariables"),
        "this is not an rdl2 scene:\n{written}"
    );
}
