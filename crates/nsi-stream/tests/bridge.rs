#![cfg(feature = "delight-bridge")]
//! Contract: `contracts/publication-lifecycle.md`.
//!
//! Row covered here:
//!
//! - "3Delight bridge: buckets upload into publication images, publication
//!   anchored to `Synchronized`/`Restarted` statuses" -- `bridge_publication`.
//!
//! The test needs a real renderer. With `DELIGHT` unset it prints a skip
//! notice and passes, so `cargo test -p nsi-stream --all-features` stays
//! green on a machine without 3Delight:
//!
//! ```text
//! DELIGHT=/path/to/3delight \
//!     cargo test -p nsi-stream --features delight-bridge bridge_publication
//! ```

use nsi_ffi_wrap as nsi;
use nsi_stream::{
    Attr, CallbackTransport, Extent, Layer, LayerFormat, Publication,
    PublishMode, StreamConfig, StreamState, bridge::DelightBridge,
};
use std::sync::{Arc, Mutex};

/// Small enough to render in seconds, large enough for several buckets.
const EXTENT: Extent = Extent::new(64, 64);

/// The one connected `outputlayer`: RGBA `f32`, which is the only format the
/// bridge supports.
fn layers() -> Vec<Layer> {
    vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)]
}

/// The `stream.*` attribute set a client would put on the `outputdriver`
/// node, decoded.
fn config(publish: PublishMode) -> StreamConfig {
    let (config, warnings) = StreamConfig::parse(&[
        Attr::string("drivername", nsi_stream::DRIVER_NAME),
        Attr::int("stream.version", 1),
        Attr::string("stream.transport", "callback"),
        Attr::string("stream.publish", publish.as_str()),
        Attr::int("stream.ring", 3),
    ])
    .expect("a version-1 vocabulary");

    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    assert_eq!(config.publish, publish);

    config
}

/// Collects every publication the driver announces, in order.
fn recorder(bridge: &DelightBridge) -> Arc<Mutex<Vec<Publication>>> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);

    bridge
        .driver()
        .set_callbacks(CallbackTransport::new().on_publish(
            move |publication| {
                sink.lock().expect("publication log").push(*publication);
            },
        ));

    log
}

/// The samples of a publication plane.
fn samples(plane: &[u8]) -> Vec<f32> {
    plane
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("4 bytes")))
        .collect()
}

/// Whether any sample in a publication plane is non-zero -- i.e. whether the
/// render actually landed in the ring.
fn has_signal(plane: &[u8]) -> bool {
    samples(plane).into_iter().any(|sample| 0.0 != sample)
}

/// Report what a mode actually produced, so the gate run carries its own
/// evidence.
fn report(mode: &str, bridge: &DelightBridge, publications: &[Publication]) {
    println!(
        "{mode}: open extent {:?}, {} bucket(s), {} publication(s), \
         serials {:?}, generations {:?}",
        bridge.open_extent(),
        bridge.buckets(),
        publications.len(),
        publications
            .iter()
            .map(|publication| publication.frame_serial)
            .collect::<Vec<_>>(),
        publications
            .iter()
            .map(|publication| publication.scene_generation)
            .collect::<Vec<_>>(),
    );
}

/// Build a minimal scene around `bridge` and render it to completion.
///
/// Returns `false` when no ɴsɪ context could be created, which is how a
/// broken renderer installation is told apart from a bridge failure.
fn render(bridge: &DelightBridge) -> bool {
    let Some(ctx) = nsi::Context::new(None) else {
        return false;
    };

    ctx.set_attribute(
        nsi::GLOBAL,
        &[
            nsi::i32!("renderatlowpriority", 1),
            nsi::string!("bucketorder", "horizontal"),
            nsi::i32!("quality.shadingsamples", 1),
            nsi::i32!("maximumraydepth.reflection", 1),
        ],
    );

    // Camera, five units back, looking down -Z.
    ctx.create("camera_xform", nsi::TRANSFORM, None);
    ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "camera_xform",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.
            ]
        )],
    );

    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, "camera_xform", "objects", None);
    ctx.set_attribute("camera", &[nsi::f32!("fov", 45.)]);

    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::i32_slice!(
            "resolution",
            &[EXTENT.width as i32, EXTENT.height as i32]
        )
        .array_len(2)],
    );

    // One quad, big enough to cover the whole frame.
    ctx.create("plane", nsi::MESH, None);
    ctx.connect("plane", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "plane",
        &[
            nsi::point_slice!(
                "P",
                &[
                    [-4.0f32, -4., 0.],
                    [4., -4., 0.],
                    [4., 4., 0.],
                    [-4., 4., 0.]
                ]
            ),
            nsi::i32_slice!("nvertices", &[4]),
        ],
    );

    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::i32!("withalpha", 1),
            nsi::string!("scalarformat", "float"),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);

    // The whole contract rides a standard `outputdriver` node: the ndspy
    // driver of `nsi-ffi-wrap` plus the bridge's three closures.
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", nsi::output::FERRIS_F32),
            nsi::string!("imagefilename", "nsi-stream-bridge"),
            nsi::callback!("callback.open", bridge.open_callback()),
            nsi::callback!("callback.write", bridge.write_callback()),
            nsi::callback!("callback.finish", bridge.finish_callback()),
        ],
    );

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);

    true
}

/// Serials must be strictly monotonic across every publication of a stream.
fn assert_monotonic(publications: &[Publication]) {
    publications.windows(2).for_each(|pair| {
        assert!(
            pair[1].frame_serial > pair[0].frame_serial,
            "frame serials must be strictly monotonic, got {} after {}",
            pair[1].frame_serial,
            pair[0].frame_serial
        );
    });
}

/// The bridge uploads 3Delight's buckets into the publication ring and
/// preserves the publication semantics of both publish modes.
#[test]
fn bridge_publication() {
    if std::env::var_os("DELIGHT").is_none() {
        println!("skipped: DELIGHT not set");
        return;
    }

    // A `DELIGHT` that does not yield a usable renderer is a broken install,
    // not a contract failure.
    if nsi::Context::new(None).is_none() {
        println!("skipped: DELIGHT is set but no ɴsɪ context could be created");
        return;
    }

    // ── `commit`: one publication per applied synchronize ───────────────────

    let bridge =
        DelightBridge::new(config(PublishMode::Commit), layers(), EXTENT)
            .expect("a legal bridge");
    let client = bridge.client();
    let log = recorder(&bridge);

    // The integrator's `stoppedcallback` would call this on
    // `RenderStatus::Synchronized`; here the scene is synchronized once,
    // before the render, so the expected generation is exactly 1.
    bridge
        .synchronized()
        .expect("an open stream")
        .expect("a free slot");
    assert_eq!(bridge.generation(), 1);

    assert!(render(&bridge), "the renderer must provide a context");

    assert_eq!(
        bridge.open_extent(),
        Some(EXTENT),
        "the open callback must have fired at the configured extent"
    );
    assert_eq!(bridge.error(), None, "the bridge recorded a failure");
    assert!(bridge.is_finished(), "the finish callback must have run");
    assert!(bridge.buckets() > 0, "no bucket reached the accumulation");

    let publications = log.lock().expect("publication log").clone();
    report("commit", &bridge, &publications);

    assert!(
        publications.len() >= 2,
        "expected the synchronize and the finish publication, got {}",
        publications.len()
    );
    assert_monotonic(&publications);
    publications.iter().for_each(|publication| {
        assert_eq!(
            publication.scene_generation, 1,
            "every publication carries the generation of the one applied \
             synchronize"
        );
        assert_eq!(publication.extent, EXTENT);
    });
    assert_eq!(
        bridge.driver().ring().published(),
        publications.len() as u64,
        "`commit` mode must not publish between commits"
    );

    // The finish callback latched the final image before closing, so the
    // last rendered frame is still acquirable while the stream drains.
    let token = bridge
        .final_image()
        .expect("the finish callback latches the final publication");

    println!(
        "commit: final image serial {}, centre pixel {:?}",
        token.publication().frame_serial,
        &samples(token.plane(0).expect("the beauty plane"))
            [(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4]
    );

    assert_eq!(token.extent(), EXTENT);
    assert_eq!(token.publication().scene_generation, 1);
    assert_eq!(
        token.publication().frame_serial,
        publications.last().expect("a publication").frame_serial
    );
    assert!(
        has_signal(token.plane(0).expect("the beauty plane")),
        "the render must have landed in the ring"
    );

    // The quad covers the frame, so the alpha channel proves the buckets
    // landed at the right place and not merely that something was written.
    let covered = samples(token.plane(0).expect("the beauty plane"))
        .chunks_exact(4)
        .filter(|pixel| 0.0 != pixel[3])
        .count();

    assert!(
        covered > EXTENT.pixels() / 2,
        "the rendered geometry must cover most of the frame, got {covered} \
         of {} pixels",
        EXTENT.pixels()
    );

    assert_eq!(bridge.driver().state(), StreamState::Draining);
    assert!(!client.is_drained(), "a lease is still out");

    client.release(token);

    assert!(client.is_drained(), "the stream drains on the last release");
    assert_eq!(bridge.driver().state(), StreamState::Closed);

    // ── `continuous`: every bucket may publish ──────────────────────────────

    let bridge =
        DelightBridge::new(config(PublishMode::Continuous), layers(), EXTENT)
            .expect("a legal bridge");
    let client = bridge.client();
    let log = recorder(&bridge);

    assert!(render(&bridge), "the renderer must provide a context");

    assert_eq!(bridge.open_extent(), Some(EXTENT));
    assert_eq!(bridge.error(), None, "the bridge recorded a failure");
    assert!(bridge.is_finished());
    assert!(bridge.buckets() > 0);

    let publications = log.lock().expect("publication log").clone();
    report("continuous", &bridge, &publications);

    assert!(
        !publications.is_empty(),
        "`continuous` mode must publish while buckets arrive"
    );
    assert_monotonic(&publications);

    let token = bridge
        .final_image()
        .expect("the finish callback latches the final publication");

    assert!(
        has_signal(token.plane(0).expect("the beauty plane")),
        "the render must have landed in the ring"
    );

    client.release(token);

    assert!(client.is_drained());
    assert_eq!(bridge.driver().state(), StreamState::Closed);
}
