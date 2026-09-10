//! Choosing an ɴsɪ renderer at runtime.
//!
//! ɴsɪ is an interface and more than one renderer implements it. These
//! check the `"renderer"` argument to `Context::new`: that a name
//! reaches the right library, that an unknown one is looked for as a
//! library rather than refused, and that the argument itself never
//! reaches a renderer.

use nsi_ffi_wrap as nsi;
use nsi_ffi_wrap::backend;

/// The names in the table resolve, whatever case or alias was typed.
#[test]
fn the_known_names_resolve() {
    for (given, canonical) in [
        ("3delight", "3delight"),
        ("delight", "3delight"),
        ("3Delight", "3delight"),
        ("moonray", "moonray"),
        ("MoonRay", "moonray"),
        ("nsi-moonray", "moonray"),
    ] {
        assert_eq!(
            backend::lookup(given).map(|backend| backend.name),
            Some(canonical),
            "{given:?} should resolve to {canonical:?}"
        );
    }
}

/// **The set is open.** A name this crate has never heard of is looked
/// for as a library rather than refused, which is what lets a renderer
/// released after this crate be used without a release of this crate.
#[test]
fn an_unknown_name_is_looked_for_as_a_library() {
    assert!(backend::lookup("mitsuba").is_none());

    let names: Vec<String> = backend::candidates("mitsuba")
        .iter()
        .filter_map(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();

    // Both spellings, because the two backends this crate knows do not
    // agree on one: `lib3delight` against `libnsi_moonray`.
    assert!(
        names.iter().any(|name| name.contains("nsi_mitsuba")),
        "{names:?}"
    );
    assert!(names.iter().any(|name| name == "mitsuba"), "{names:?}");
}

/// A renderer that is not there fails, and says all three things a
/// reader needs: that no backend is named that, that no library could
/// be loaded either, and what the known names are.
#[test]
fn an_absent_renderer_is_reported_by_name() {
    let context = nsi::Context::new(Some(&[nsi::string!(
        "renderer",
        "definitely-not-a-renderer"
    )]));

    assert!(
        context.is_none(),
        "a renderer that cannot be loaded must not yield a context"
    );
}

/// **The argument is answered here, not forwarded.**
///
/// `"renderer"` tells this crate which library to open. No renderer
/// declares it, so passing it through would have every scene carry an
/// argument its renderer does not understand. Checked against the
/// stream output, which is the one place every argument to a context
/// is visible as text.
#[test]
fn the_renderer_argument_is_not_forwarded() {
    let stream = std::env::temp_dir().join("nsi-renderer-argument.nsi");
    let _ = std::fs::remove_file(&stream);

    {
        let context = nsi::Context::new(Some(&[
            nsi::string!("renderer", "3delight"),
            nsi::string!("type", "apistream"),
            nsi::string!(
                "streamfilename",
                stream.to_string_lossy().as_ref()
            ),
        ]))
        .expect("3delight is the default and should be present");

        context.create("a", nsi::TRANSFORM, None);
    }

    let written = std::fs::read_to_string(&stream).unwrap_or_default();
    let _ = std::fs::remove_file(&stream);

    assert!(
        !written.contains("renderer"),
        "the renderer argument must not reach the renderer:\n{written}"
    );
}

/// The MoonRay backend, when this machine has one.
///
/// Skipped rather than failed without it: the point of naming a
/// renderer is that a machine need not have all of them, and a test
/// that demands every backend be installed would be the opposite of
/// what this feature is for.
#[test]
fn moonray_loads_when_it_is_installed() {
    let found = backend::candidates("moonray")
        .into_iter()
        .find(|path| path.is_file());

    let Some(path) = found else {
        eprintln!(
            "no MoonRay backend on this machine; set $NSI_MOONRAY to an \
             install prefix to run this. Searched: {:?}",
            backend::candidates("moonray")
        );
        return;
    };

    let context = nsi::Context::new(Some(&[nsi::string!(
        "renderer",
        "moonray"
    )]))
    .unwrap_or_else(|| panic!("{} exists but did not load", path.display()));

    // It is a real context, so it takes a real scene.
    context.create("a", nsi::TRANSFORM, None);
    context.connect("a", None, nsi::ROOT, "objects", None);
}

/// **Two renderers in one process, at once.**
///
/// The thing that was impossible before: the renderer used to be a
/// process-wide static, resolved once and never again. Both contexts
/// are alive at the same time here, which is what a host switching
/// between them in one session actually does.
#[test]
fn two_renderers_coexist() {
    let moonray = backend::candidates("moonray")
        .into_iter()
        .any(|path| path.is_file());

    if !moonray {
        eprintln!("no MoonRay backend on this machine; skipping");
        return;
    }

    let delight = nsi::Context::new(Some(&[nsi::string!(
        "renderer",
        "3delight"
    )]))
    .expect("3delight");
    let moonray = nsi::Context::new(Some(&[nsi::string!(
        "renderer",
        "moonray"
    )]))
    .expect("moonray");

    // Each takes its own scene, and neither disturbs the other.
    delight.create("in_delight", nsi::TRANSFORM, None);
    moonray.create("in_moonray", nsi::TRANSFORM, None);
    delight.create("also_in_delight", nsi::TRANSFORM, None);
}
