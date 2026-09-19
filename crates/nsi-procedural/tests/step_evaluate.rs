//! Evaluates the `step_procedural` example in 3Delight, as a dynamic
//! library, and renders the part it emits.
//!
//! Needs 3Delight 2.9.210 or later at `$DELIGHT` and a license: this one
//! renders. The part, `io1-ec-214.stp`, is `monster-step-viewer`'s
//! fixture, read where it is; the test skips when it is absent.
use nsi_ffi_wrap as nsi;
use std::{
    env,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

const PART: &str =
    "/home/ritz/code/crates/monster-step-viewer/step-files/io1-ec-214.stp";

/// Builds the example and returns the library's path.
fn build_example() -> PathBuf {
    let status = Command::new(env!("CARGO"))
        .args(["build", "--example", "step_procedural"])
        .status()
        .expect("cargo build");
    assert!(status.success());

    // The build honours `CARGO_TARGET_DIR`, so the lookup must too.
    env::var("CARGO_TARGET_DIR")
        .ok()
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
        })
        .join("debug/examples/libstep_procedural.so")
}

/// A context whose messages are collected, and the collection.
fn new_context() -> (nsi::Context<'static>, Arc<Mutex<Vec<String>>>) {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let sink = messages.clone();
    let error_handler =
        nsi::ErrorCallback::new(move |_: log::Level, _: i32, message: &str| {
            sink.lock().unwrap().push(message.to_string())
        });
    let context = nsi::Context::new(Some(&[nsi::callback!(
        "errorhandler",
        error_handler
    )]))
    .expect("context");
    (context, messages)
}

/// A camera above the part looking down, a 128×128 PNG, and a red
/// constant shader bound to everything below `.root`.
fn set_up_render(context: &nsi::Context<'_>, image: &Path) {
    context.create("camera_transform", nsi::TRANSFORM, None);
    context.connect("camera_transform", None, nsi::ROOT, "objects", None);
    // The part spans x -34..44, y ±29.5, z 0..33; ɴsɪ cameras look down
    // -z.
    #[rustfmt::skip]
    context.set_attribute(
        "camera_transform",
        &[nsi::matrix4_f64!("transformationmatrix", &[
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            5.0, 0.0, 250.0, 1.0,
        ])],
    );
    context.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    context.set_attribute("camera", &[nsi::real_f32!("fov", 30.0)]);
    context.connect("camera", None, "camera_transform", "objects", None);
    context.create("screen", nsi::SCREEN, None);
    context.connect("screen", None, "camera", "screens", None);
    context.set_attribute(
        "screen",
        &[nsi::integer_i32_slice!("resolution", &[128, 128])
            .array_len(const { NonZeroUsize::new(2).unwrap() })],
    );
    context.create("layer", nsi::OUTPUT_LAYER, None);
    context.set_attribute(
        "layer",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "uint8"),
        ],
    );
    context.connect("layer", None, "screen", "outputlayers", None);
    context.create("driver", nsi::OUTPUT_DRIVER, None);
    context.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "png"),
            nsi::string!("imagefilename", image.to_str().unwrap()),
        ],
    );
    context.connect("driver", None, "layer", "outputdrivers", None);

    let delight = env::var("DELIGHT").expect("$DELIGHT");
    context.create("red", nsi::SHADER, None);
    context.set_attribute(
        "red",
        &[
            nsi::string!(
                "shaderfilename",
                format!("{delight}/osl/dlConstant.oso").as_str()
            ),
            nsi::color3_f32!("i_color", &[1.0, 0.0, 0.0]),
            // One-sided, so the part shows red only where its faces' fronts
            // face out.
            nsi::integer_i32!("doublesided", 0),
        ],
    );
    context.create("look", nsi::ATTRIBUTES, None);
    context.connect("red", None, "look", "surfaceshader", None);
    context.connect("look", None, nsi::ROOT, "geometryattributes", None);
}

/// How many pixels of the PNG at `image` are red.
fn red_pixels(image: &Path) -> usize {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(image).expect("the render wrote an image"),
    ));
    let mut reader = decoder.read_info().expect("png");
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).expect("frame");
    let channels = info.color_type.samples();
    pixels[..info.buffer_size()]
        .chunks(channels)
        .filter(|pixel| pixel[0] > 128 && pixel[1] < 64)
        .count()
}

/// Renders the part with or without welds and returns the red pixels
/// and 3Delight's messages.
fn render(weld: i32) -> Option<(usize, Vec<String>)> {
    if !Path::new(PART).exists() {
        eprintln!("skipped: {PART} is absent");
        return None;
    }
    let library = build_example();
    let directory = env::temp_dir().join("nsi_step_procedural_test");
    std::fs::create_dir_all(&directory).unwrap();
    let image = directory.join(format!("weld{weld}.png"));
    let _ = std::fs::remove_file(&image);

    let (context, messages) = new_context();
    set_up_render(&context, &image);
    context.evaluate(&[
        nsi::string!("type", "dynamiclibrary"),
        nsi::string!("filename", library.to_str().unwrap()),
        nsi::string!("stepfilename", PART),
        nsi::integer_i32!("weld", weld),
    ]);
    context.render_control(nsi::Action::Start, None);
    context.render_control(nsi::Action::Wait, None);
    drop(context);

    let messages = messages.lock().unwrap().clone();
    Some((red_pixels(&image), messages))
}

/// 3Delight loads the library, the procedural reads its parameters, and
/// the part renders -- the same with weld declarations as without.
///
/// Seen from above, the part is a disc of radius 44 with a hole of 18
/// through its middle and six of 7 through its flange: about 4100 square
/// units, some 3500 of the 16384 pixels.
#[test]
fn three_delight_renders_the_part() {
    let Some((welded, welded_messages)) = render(1) else {
        return;
    };
    let Some((plain, plain_messages)) = render(0) else {
        return;
    };

    for messages in [&welded_messages, &plain_messages] {
        assert!(
            messages
                .iter()
                .any(|message| message.contains("1 shell(s), 17 face(s)")),
            "the procedural ran in 3Delight: {messages:#?}"
        );
    }
    assert!(
        (3000..5000).contains(&plain),
        "the part is visible: {plain} red pixels, {plain_messages:#?}"
    );
    // Keeping the loops that trace a face's whole domain changes nothing
    // on screen.
    assert!(
        welded.abs_diff(plain) <= plain / 100,
        "{welded} red pixels with welds, {plain} without"
    );

    // Without welds, 3Delight has nothing to say. With them, it only
    // refuses the draft's `weld` node, which it does not know yet, and
    // the connections to it.
    let complaints = |messages: &[String]| {
        messages
            .iter()
            .filter(|message| !message.starts_with("step_procedural:"))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(complaints(&plain_messages), Vec::<String>::new());
    let unexpected = complaints(&welded_messages)
        .into_iter()
        .filter(|message| {
            !message.contains("of unknown type 'weld'")
                && !(message.contains("_welds' passed to NSIConnect")
                    && message.ends_with(",weld>)"))
        })
        .collect::<Vec<_>>();
    assert!(unexpected.is_empty(), "{unexpected:#?}");
}
