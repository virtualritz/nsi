//! End to end on a real part: STEP file → `step_procedural` → `nurbs`
//! nodes with weld declarations → `nsi-tessellate` → displaced in
//! 3Delight. Welded, the displaced part stays closed; unwelded, it cracks
//! along the edges its faces share.
//!
//! Every surface uses `nsi-tessellate`'s `inside_bright`: black seen from
//! the front, red seen from the back. The backdrop is blue. Red pixels are
//! insides in view -- through a crack, or a surface facing the wrong way.
//!
//! Needs 3Delight 2.9.210 with its `oslc`, and `monster-step-viewer`'s
//! fixture, read where it is; the test skips when that is absent.
use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Recorder, Scene, write_stream};
use nsi_procedural::Report;
use nsi_tessellate::{NurbsOptions, NurbsTessellation, nurbs_meshes};
use std::{
    env,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
};

#[allow(dead_code)]
#[path = "../examples/step/lib.rs"]
mod step;

const PART: &str =
    "/home/ritz/code/crates/monster-step-viewer/step-files/io1-ec-214.stp";
const SIZE: usize = 256;

/// The part spans x -34..44, y ±29.5, z 0..33.
const TARGET: [f64; 3] = [5.0, 0.0, 16.0];
const EYE: [f64; 3] = [135.0, 110.0, 190.0];

/// Runs the procedural on the part and returns the scene it made.
fn record(weld: i32) -> Scene {
    let recorder = Recorder::new();
    let sink = |_, message: &str| eprintln!("{message}");
    nsi_procedural::execute(
        &step::StepProcedural,
        &recorder,
        &Report::new(&sink),
        &[
            nsi::string!("filename", PART),
            nsi::integer_i32!("weld", weld),
        ],
    )
    .expect("the procedural runs");
    recorder.into_scene()
}

/// A row-major ɴsɪ transform placing local -z along `target - eye`, the
/// way a camera looks, at `origin`.
fn looking(eye: [f64; 3], target: [f64; 3], origin: [f64; 3]) -> [f64; 16] {
    let sub =
        |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |a: [f64; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let back = unit(sub(eye, target));
    let right = unit(cross([0.0, 1.0, 0.0], back));
    let up = cross(back, right);
    [
        right[0], right[1], right[2], 0.0, //
        up[0], up[1], up[2], 0.0, //
        back[0], back[1], back[2], 0.0, //
        origin[0], origin[1], origin[2], 1.0,
    ]
}

/// Compiles `nsi-tessellate`'s `name`.osl into `directory`.
fn compile(directory: &Path, name: &str) -> String {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../nsi-tessellate/tests/shaders/{name}.osl"));
    let compiled = directory.join(format!("{name}.oso"));
    let oslc =
        Path::new(&env::var("DELIGHT").expect("$DELIGHT")).join("bin/oslc");
    let status = Command::new(oslc)
        .arg("-o")
        .arg(&compiled)
        .arg(&source)
        .status()
        .expect("oslc");
    assert!(status.success());
    compiled.to_str().unwrap().to_string()
}

/// Renders the part as `meshes`, displaced along their normals by
/// `push`, and returns how many pixels are red.
fn red_pixels(
    scene: &Scene,
    meshes: &NurbsTessellation,
    push: f32,
    name: &str,
) -> usize {
    let directory = env::temp_dir().join("nsi_step_displacement_test");
    std::fs::create_dir_all(&directory).unwrap();
    let inside = compile(&directory, "inside_bright");
    let displace = compile(&directory, "push_along_normal");
    let image: PathBuf = directory.join(format!("{name}.png"));
    let _ = std::fs::remove_file(&image);

    let ctx = nsi::Context::new(None).expect("context");
    let delight = env::var("DELIGHT").expect("$DELIGHT");

    ctx.create("camera_xform", nsi::TRANSFORM, None);
    ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "camera_xform",
        &[nsi::matrix4_f64!(
            "transformationmatrix",
            &looking(EYE, TARGET, EYE)
        )],
    );
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.set_attribute("camera", &[nsi::real_f32!("fov", 35.0)]);
    ctx.connect("camera", None, "camera_xform", "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    let size = SIZE as i32;
    ctx.set_attribute(
        "screen",
        &[
            nsi::integer_i32_slice!("resolution", &[size, size])
                .array_len(const { NonZeroUsize::new(2).unwrap() }),
            nsi::integer_i32!("oversampling", 16),
        ],
    );
    ctx.create("layer", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "layer",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "uint8"),
        ],
    );
    ctx.connect("layer", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "png"),
            nsi::string!("imagefilename", image.to_str().unwrap()),
        ],
    );
    ctx.connect("driver", None, "layer", "outputdrivers", None);

    // A blue wall behind the part, facing the camera.
    let behind = {
        let d = [EYE[0] - TARGET[0], EYE[1] - TARGET[1], EYE[2] - TARGET[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        [
            TARGET[0] - 100.0 * d[0] / l,
            TARGET[1] - 100.0 * d[1] / l,
            TARGET[2] - 100.0 * d[2] / l,
        ]
    };
    ctx.create("wall_xform", nsi::TRANSFORM, None);
    ctx.connect("wall_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "wall_xform",
        &[nsi::matrix4_f64!(
            "transformationmatrix",
            &looking(EYE, TARGET, behind)
        )],
    );
    ctx.create("wall", nsi::PLANE, None);
    ctx.connect("wall", None, "wall_xform", "objects", None);
    ctx.create("blue", nsi::SHADER, None);
    ctx.set_attribute(
        "blue",
        &[
            nsi::string!(
                "shaderfilename",
                format!("{delight}/osl/dlConstant.oso").as_str()
            ),
            nsi::color3_f32!("i_color", &[0.0, 0.0, 1.0]),
        ],
    );
    ctx.create("wall_look", nsi::ATTRIBUTES, None);
    ctx.connect("blue", None, "wall_look", "surfaceshader", None);
    ctx.connect("wall_look", None, "wall", "geometryattributes", None);

    ctx.create("inside", nsi::SHADER, None);
    ctx.set_attribute(
        "inside",
        &[nsi::string!("shaderfilename", inside.as_str())],
    );
    ctx.create("push", nsi::SHADER, None);
    ctx.set_attribute(
        "push",
        &[
            nsi::string!("shaderfilename", displace.as_str()),
            nsi::real_f32!("amount", push),
        ],
    );
    ctx.create("look", nsi::ATTRIBUTES, None);
    ctx.connect("inside", None, "look", "surfaceshader", None);
    ctx.connect("push", None, "look", "displacementshader", None);
    // 3Delight displaces nothing without it.
    ctx.set_attribute(
        "look",
        &[nsi::real_f32!("displacementbound", push.abs() + 1.0)],
    );

    // The procedural's scene, for its transforms; each `nurbs` node is
    // then replaced by its mesh, below the same transform.
    let stream = directory.join(format!("{name}.nsi"));
    let mut out = Vec::new();
    write_stream(scene, &mut out).expect("write the scene");
    std::fs::write(&stream, out).unwrap();
    ctx.evaluate(&[
        nsi::string!("type", "apistream"),
        nsi::string!("filename", stream.to_str().unwrap()),
    ]);
    for mesh in &meshes.meshes {
        let face = mesh.geometry.as_str();
        // The example names faces `<shell>_face<j>`, below `<shell>`.
        let (shell, _) = face.rsplit_once("_face").expect("a face handle");
        ctx.delete(face, None);
        let handle = format!("{face}_mesh");
        let positions: Vec<[f32; 3]> =
            mesh.positions.iter().map(|p| p.map(|c| c as f32)).collect();
        let normals: Vec<[f32; 3]> =
            mesh.normals.iter().map(|n| n.map(|c| c as f32)).collect();
        let indices: Vec<i32> =
            mesh.triangles.iter().flatten().map(|&i| i as i32).collect();
        let counts = vec![3; mesh.triangles.len()];
        ctx.create(&handle, nsi::MESH, None);
        ctx.connect(&handle, None, shell, "objects", None);
        ctx.connect("look", None, &handle, "geometryattributes", None);
        ctx.set_attribute(
            &handle,
            &[
                nsi::integer_i32_slice!("nvertices", &counts),
                nsi::point3_f32_slice!("P", &positions),
                nsi::integer_i32_slice!("P.indices", &indices),
                nsi::normal3_f32_slice!("N", &normals),
                nsi::integer_i32_slice!("N.indices", &indices),
            ],
        );
    }

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
    drop(ctx);

    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&image).expect("the render wrote an image"),
    ));
    let mut reader = decoder.read_info().expect("png");
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).expect("frame");
    let channels = info.color_type.samples();
    pixels[..info.buffer_size()]
        .chunks(channels)
        .filter(|pixel| pixel[0] > 128 && pixel[2] < 64)
        .count()
}

#[test]
fn the_displaced_part_stays_closed_only_where_welded() {
    if !Path::new(PART).exists() {
        eprintln!("skipped: {PART} is absent");
        return;
    }
    let options = NurbsOptions { tolerance: 0.02 };
    let welded_scene = record(1);
    let unwelded_scene = record(0);
    let welded = nurbs_meshes(&welded_scene, &options);
    let unwelded = nurbs_meshes(&unwelded_scene, &options);
    assert!(welded.problems.is_empty(), "{:#?}", welded.problems);
    assert_eq!(welded.meshes.len(), 17);
    assert_eq!(unwelded.meshes.len(), 17);
    println!(
        "open edges: welded {}, unwelded {}",
        welded.open_edges, unwelded.open_edges
    );
    assert_eq!(welded.open_edges, 0, "the welded part is watertight");
    assert!(unwelded.open_edges > 0, "the unwelded part is not");

    let still = red_pixels(&welded_scene, &welded, 0.0, "welded_still");
    let push = 1.0;
    let welded_red = red_pixels(&welded_scene, &welded, push, "welded");
    let unwelded_red = red_pixels(&unwelded_scene, &unwelded, push, "unwelded");
    println!(
        "red px: still {still}; displaced by {push}: welded {welded_red}, \
         unwelded {unwelded_red}"
    );
    assert!(still < 20, "the part shows its fronts: {still}");
    assert!(
        welded_red < 20,
        "the welded part stays closed: {welded_red}"
    );
    assert!(
        unwelded_red > 200,
        "the unwelded part cracks: {unwelded_red}"
    );
}
