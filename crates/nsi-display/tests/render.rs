//! Renders through the example driver, proving the symbols are what
//! 3Delight actually resolves.
//!
//! Needs a licensed 3Delight; see the crate's `png_driver` example doc
//! comment for how 3Delight resolves `drivername` to a `.dpy` on disk,
//! and for why this test runs the renderer with its working directory
//! set to a staging directory rather than an environment variable --
//! there isn't one for display drivers.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Restores the process's working directory on drop, so a panic
/// between `set_current_dir` calls can't leave it changed for
/// whatever runs next in the same process.
struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// Builds and renders the same 16x16, `drivername = "rust_png"` scene
/// used by both phases of the test below, writing (if a driver
/// answering to that name is found) to `out`.
fn render_scene(out: &Path) {
    let ctx = nsi::Context::new(None).expect("context");
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, nsi::ROOT, "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::i32_slice!("resolution", &[16, 16])
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    );
    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "uint8"),
            // The port assumes RGBA throughout, same as the original
            // it ports -- this is what asks 3Delight for the alpha
            // channel.
            nsi::i32!("withalpha", 1),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "rust_png"),
            nsi::string!("imagefilename", out.to_str().unwrap()),
        ],
    );
    ctx.connect("driver", None, "beauty", "outputdrivers", None);

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
}

#[test]
fn the_example_driver_receives_pixels_from_3delight() {
    // Build the driver and put it where the renderer will find it.
    let status = Command::new(env!("CARGO"))
        .args(["build", "--example", "png_driver"])
        .status()
        .expect("cargo build");
    assert!(status.success());

    let built = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/libpng_driver.so");

    // 3Delight resolves `drivername` against a search path, not an
    // environment variable -- there is no `DL_DISPLAYS_PATH` (unlike
    // every other resource kind, e.g. `DL_SHADERS_PATH`). Its default
    // display search path is ".:$DELIGHT/displays:$DELIGHT/lib" (from
    // `strings` on lib3delight.so), with "." -- the renderer's cwd --
    // searched first, and the artefact it looks for is plain
    // "<drivername>.dpy" (confirmed by staging a driver under that
    // exact name and observing it get called; see the `png_driver`
    // doc comment for the full writeup).
    //
    // The name matters too: "png" is one of 3Delight's own built-in
    // drivers, so it never reaches ours -- hence `rust_png` here. That
    // guard is name-specific, though: it only proves *this* collision
    // is avoided today. The negative control below proves provenance
    // structurally instead -- the PNG's existence is caused by our
    // staged artefact being present, whatever name is in use.
    let stage_dir = std::env::temp_dir().join("nsi_display_test_stage");
    std::fs::create_dir_all(&stage_dir).expect("create stage dir");
    let dpy = stage_dir.join("rust_png.dpy");
    // Make sure the driver is NOT staged yet, for the negative
    // control phase below.
    let _ = std::fs::remove_file(&dpy);

    let out = stage_dir.join("nsi_display_test");
    let written = out.with_extension("png");
    let _ = std::fs::remove_file(&written);

    let original_dir = std::env::current_dir().expect("cwd");
    let _cwd_guard = CwdGuard(original_dir);
    std::env::set_current_dir(&stage_dir).expect("chdir into stage dir");

    // Phase 1: negative control. Render the identical scene, same
    // `drivername`, with no driver answering to that name anywhere on
    // the search path (the stage dir -- first in the path -- is
    // otherwise empty, and nothing else on the machine ships a
    // "rust_png" driver). If nothing gets written here, a later green
    // Phase 2 can only be explained by OUR staged artefact, not by a
    // same-named driver 3Delight happens to already have.
    render_scene(&out);
    assert!(
        !written.exists(),
        "no driver staged: {} must not have been written, but was",
        written.display()
    );

    // Phase 2: stage the driver and render the same scene again.
    std::fs::copy(&built, &dpy).expect("stage the .dpy");
    render_scene(&out);

    assert!(
        written.exists(),
        "the driver's close() must have written {}",
        written.display()
    );

    // Assert on real content: valid PNG, right dimensions, plausible
    // size for a 16x16 RGBA frame.
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&written).expect("open png"),
    ));
    let mut reader = decoder.read_info().expect("read png header");
    let info = reader.info();
    assert_eq!(16, info.width, "png width");
    assert_eq!(16, info.height, "png height");
    assert_eq!(png::ColorType::Rgba, info.color_type, "png color type");

    let mut buf =
        vec![0u8; reader.output_buffer_size().expect("png output buffer size")];
    reader.next_frame(&mut buf).expect("decode png frame");
    assert_eq!(16 * 16 * 4, buf.len(), "one RGBA quadruple per pixel");

    let _ = std::fs::remove_file(&written);
    let _ = std::fs::remove_file(&dpy);
}
