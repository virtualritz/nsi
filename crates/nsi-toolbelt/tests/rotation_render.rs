//! Settles the rotation *direction* against the renderer.
//!
//! `transform`'s unit tests catch the angle being wrong, but not the
//! sense of the turn: a half turn is its own inverse, and a quarter
//! turn's magnitude is the same either way. The old `rotation()`
//! transposed its matrix, and a rotation's transpose is its inverse, so
//! only a render can say whether removing that was right.
//!
//! Needs a licensed 3Delight. Renders the depth layer rather than the
//! beauty, so no shader is involved and a bare quad is still visible.
use nsi_ffi_wrap as nsi;
use nsi_toolbelt::scene;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

const RESOLUTION: usize = 64;
/// Anything nearer than this is the quad; the background reads as a
/// very large depth.
const NEAR_ENOUGH: f32 = 1.0e6;

/// Where the quad landed, in pixels, or `None` if nothing was visible.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Centroid {
    x: f32,
    y: f32,
}

/// Renders one quad, placed by `place`, and reports where it appears.
///
/// `place` returns `(attach, top)`: the node the quad hangs from, and
/// the node connected to `.root`. They differ whenever there is more
/// than one transform in the chain -- conflating them silently bypassed
/// the inner transform, which is how the first version of this test
/// "found" a half turn that did not move anything.
fn render(place: impl FnOnce(&nsi::Context) -> (String, String)) -> Centroid {
    let depth = Arc::new(Mutex::new(vec![f32::MAX; RESOLUTION * RESOLUTION]));

    {
        let ctx = nsi::Context::new(None).expect("context");

        // Camera at +Z looking back at the origin: +X is to the right
        // of frame, +Y is up in the world.
        let camera_transform =
            scene::translation(&ctx, Some("camera_xform"), &[0.0, 0.0, 10.0]);
        let camera = scene::perspective_camera(&ctx, Some("camera"));
        camera.set(&ctx, &[nsi::f32!("fov", 35.0)]);
        scene::root().append(&ctx, &camera_transform);
        camera_transform.append(&ctx, &camera);

        let screen = scene::screen(&ctx, Some("screen"));
        screen.set(
            &ctx,
            &[
                nsi::i32_slice!(
                    "resolution",
                    &[RESOLUTION as i32, RESOLUTION as i32]
                )
                .array_len(const { NonZeroUsize::new(2).unwrap() }),
                nsi::i32!("oversampling", 4),
            ],
        );
        camera.screens(&ctx, &screen);

        let layer = scene::output_layer(&ctx, Some("depth"));
        layer.set(
            &ctx,
            &[
                nsi::string!("variablename", "z"),
                nsi::string!("layertype", "scalar"),
                nsi::string!("scalarformat", "float"),
            ],
        );
        screen.output_layers(&ctx, &layer);

        let buffer = Arc::clone(&depth);
        let write = nsi::output::WriteCallback::new(
            move |_: &str,
                  _width: usize,
                  _height: usize,
                  x_min: usize,
                  x_max_plus_one: usize,
                  y_min: usize,
                  y_max_plus_one: usize,
                  format: &nsi::output::PixelFormat,
                  bucket: &[f32]| {
                let channels = format.channels();
                let mut buffer = buffer.lock().unwrap();
                for y in y_min..y_max_plus_one {
                    for x in x_min..x_max_plus_one {
                        let source = ((y - y_min) * (x_max_plus_one - x_min)
                            + (x - x_min))
                            * channels;
                        buffer[y * RESOLUTION + x] = bucket[source];
                    }
                }
                nsi::output::Error::None
            },
        );

        let driver = scene::output_driver(&ctx, Some("driver"));
        driver.set(
            &ctx,
            &[
                nsi::string!("drivername", nsi::output::FERRIS_F32),
                nsi::string!("imagefilename", "rotation_probe"),
                nsi::callback!("callback.write", write),
            ],
        );
        layer.output_drivers(&ctx, &driver);

        // A unit quad in the XY plane, placed by the caller.
        let quad = scene::mesh(&ctx, Some("quad"));
        quad.set(
            &ctx,
            &[
                nsi::i32!("nvertices", 4),
                nsi::point_slice!(
                    "P",
                    &[
                        [-0.5f32, -0.5, 0.0],
                        [0.5, -0.5, 0.0],
                        [0.5, 0.5, 0.0],
                        [-0.5, 0.5, 0.0],
                    ]
                ),
            ],
        );

        let (attach, top) = place(&ctx);
        ctx.connect("quad", None, &attach, "objects", None);
        ctx.connect(&top, None, nsi::ROOT, "objects", None);

        ctx.render_control(nsi::Action::Start, None);
        ctx.render_control(nsi::Action::Wait, None);
    }

    let depth = depth.lock().unwrap();
    let (mut sum_x, mut sum_y, mut count) = (0.0f32, 0.0f32, 0usize);
    for y in 0..RESOLUTION {
        for x in 0..RESOLUTION {
            if depth[y * RESOLUTION + x] < NEAR_ENOUGH {
                sum_x += x as f32;
                sum_y += y as f32;
                count += 1;
            }
        }
    }
    assert!(count > 0, "the quad was not visible at all");
    Centroid {
        x: sum_x / count as f32,
        y: sum_y / count as f32,
    }
}

/// A rotation must turn the way the matrix says, and the only witness
/// is the renderer.
///
/// Four placements of the same quad:
/// - at +X, giving the baseline and which way +X is on screen,
/// - at +Y, establishing which way +Y is on screen without assuming
///   whether image rows run up or down,
/// - at +X under a half turn about Z, which must land where -X is,
/// - at +X under a quarter turn about Z, which must land where +Y is.
#[test]
fn a_rotation_turns_the_way_the_renderer_agrees_with() {
    let centre = RESOLUTION as f32 / 2.0;

    let at_x = render(|ctx| {
        let t = scene::translation(ctx, Some("t"), &[2.0, 0.0, 0.0]);
        (t.as_str().to_owned(), t.as_str().to_owned())
    });
    let at_y = render(|ctx| {
        let t = scene::translation(ctx, Some("t"), &[0.0, 2.0, 0.0]);
        (t.as_str().to_owned(), t.as_str().to_owned())
    });

    // Sanity: the two probes are on different axes of the image.
    assert!(
        at_x.x > centre + 5.0,
        "+X should be right of centre: {at_x:?}"
    );
    assert!(
        (at_x.y - centre).abs() < 5.0,
        "+X should be vertically centred: {at_x:?}"
    );
    assert!(
        (at_y.x - centre).abs() < 5.0,
        "+Y should be horizontally centred: {at_y:?}"
    );
    assert!(
        (at_y.y - centre).abs() > 5.0,
        "+Y should be off centre vertically: {at_y:?}"
    );

    // A half turn about Z sends +X to -X. This is the end-to-end check
    // on the angle: with the old `angle * TAU / 90.0`, 180 degrees was
    // a 720 degree turn and the quad would not have moved at all.
    let half = render(|ctx| {
        let spin = scene::rotation(ctx, Some("r"), 180.0, &[0.0, 0.0, 1.0]);
        let place = scene::translation(ctx, Some("t"), &[2.0, 0.0, 0.0]);
        spin.append(ctx, &place);
        (place.as_str().to_owned(), spin.as_str().to_owned())
    });
    assert!(
        half.x < centre - 5.0,
        "a half turn must send +X to the other side: {half:?} (baseline {at_x:?})"
    );

    // A quarter turn about Z sends +X to +Y. Compared against the +Y
    // probe rather than against an assumed image orientation, so this
    // pins the *direction* of the turn without hard-coding whether
    // image rows run up or down.
    let quarter = render(|ctx| {
        let spin = scene::rotation(ctx, Some("r"), 90.0, &[0.0, 0.0, 1.0]);
        let place = scene::translation(ctx, Some("t"), &[2.0, 0.0, 0.0]);
        spin.append(ctx, &place);
        (place.as_str().to_owned(), spin.as_str().to_owned())
    });
    assert!(
        (quarter.x - centre).abs() < 5.0,
        "a quarter turn must clear the X axis: {quarter:?}"
    );
    assert!(
        (quarter.y - at_y.y).abs() < 5.0,
        "a quarter turn about Z must send +X where +Y is, not the \
         opposite way: quarter {quarter:?}, +Y probe {at_y:?}, centre \
         {centre}"
    );
}
