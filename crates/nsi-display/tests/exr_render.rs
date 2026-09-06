//! Renders a multi-AOV scene through the example EXR driver.
//!
//! Needs a licensed 3Delight, and OIDN found at link time and again at
//! run time:
//!
//! ```text
//! RUSTFLAGS="-C link-arg=-Wl,-rpath,$DELIGHT/lib/oidn/lib" \
//!   OIDN_DIR=$DELIGHT/lib/oidn \
//!   cargo test -p nsi-display --features exr-driver --test exr_render
//! ```
//!
//! An rpath rather than `LD_LIBRARY_PATH`: 3Delight's OIDN directory
//! also holds its own TBB, and `mold` loads TBB itself, so putting that
//! directory on `LD_LIBRARY_PATH` makes the *linker* pick up the wrong
//! one and die on an undefined symbol. The rpath also makes the built
//! `.dpy` self-contained, which is what you want when the renderer,
//! not cargo, is loading it.
#![cfg(feature = "exr-driver")]

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// A 16x16 render with a beauty layer and a depth layer, both connected
/// to one `rust_exr` driver -- so the driver sees a genuinely multi-AOV
/// `PixelFormat`, which is the case the layer-boundary parser used to
/// get wrong.
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

    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "rust_exr"),
            nsi::string!("imagefilename", out.to_str().unwrap()),
            nsi::string!("compression", "zips"),
            nsi::string!("header.comment", "written by nsi-display"),
        ],
    );

    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "float"),
            nsi::i32!("withalpha", 1),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);

    ctx.create("depth", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "depth",
        &[
            nsi::string!("variablename", "z"),
            nsi::string!("layername", "depth"),
            nsi::string!("layertype", "scalar"),
            nsi::string!("scalarformat", "float"),
        ],
    );
    ctx.connect("depth", None, "screen", "outputlayers", None);
    ctx.connect("driver", None, "depth", "outputdrivers", None);

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
}

#[test]
fn the_exr_driver_writes_every_connected_layer() {
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--features",
            "exr-driver",
            "--example",
            "exr_driver",
        ])
        .status()
        .expect("cargo build");
    assert!(status.success());

    let target_dir = env::var("CARGO_TARGET_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
        });
    let built = target_dir.join("debug/examples/libexr_driver.so");

    // See `png_driver`'s docs: `drivername` resolves against a search
    // path whose first entry is the renderer's cwd, and the artefact is
    // `<drivername>.dpy`.
    let stage_dir = std::env::temp_dir().join("nsi_display_exr_stage");
    std::fs::create_dir_all(&stage_dir).expect("create stage dir");
    let dpy = stage_dir.join("rust_exr.dpy");
    let _ = std::fs::remove_file(&dpy);

    let out = stage_dir.join("nsi_exr_test");
    let written = out.with_extension("exr");
    let _ = std::fs::remove_file(&written);

    let original_dir = std::env::current_dir().expect("cwd");
    let _cwd_guard = CwdGuard(original_dir);
    std::env::set_current_dir(&stage_dir).expect("chdir into stage dir");

    // Negative control: nothing answers to `rust_exr` yet, so a green
    // second phase can only be our staged artefact.
    render_scene(&out);
    assert!(
        !written.exists(),
        "no driver staged: {} must not have been written",
        written.display()
    );

    std::fs::copy(&built, &dpy).expect("stage the .dpy");
    render_scene(&out);
    assert!(
        written.exists(),
        "the driver's close() must have written {}",
        written.display()
    );

    // Assert on real content, read back through a different code path
    // than the one that wrote it.
    let image = exr::prelude::read_all_data_from_file(&written)
        .expect("read the exr back");
    let layer = &image.layer_data[0];
    assert_eq!(16, layer.size.0, "exr width");
    assert_eq!(16, layer.size.1, "exr height");

    let names: Vec<String> = layer
        .channel_data
        .list
        .iter()
        .map(|c| c.name.to_string())
        .collect();

    // 3Delight sends both layers' channels unprefixed --
    // `["r","g","b","a","z"]`, measured -- so the beauty is R,G,B,A and
    // the depth is OpenEXR's conventional `Z`. Both must survive: the
    // parser defect this exercises used to merge the second into the
    // first and drop a channel, leaving a five-channel "Ci".
    for expected in ["A", "B", "G", "R", "Z"] {
        assert!(
            names.iter().any(|n| n == expected),
            "channel {expected} missing from {names:?}"
        );
    }

    // `header.comment` must have reached the file's own header.
    let comment = layer
        .attributes
        .other
        .iter()
        .find(|(key, _)| **key == *"comment");
    assert!(
        comment.is_some(),
        "header.comment must be written into the exr header, got {:?}",
        layer.attributes.other.keys().collect::<Vec<_>>()
    );

    let _ = std::fs::remove_file(&written);
    let _ = std::fs::remove_file(&dpy);
}
