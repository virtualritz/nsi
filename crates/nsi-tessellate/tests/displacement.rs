//! End to end in 3Delight: welded NURBS, tessellated here and displaced by
//! the renderer, stay closed; the same patches unwelded crack.
//!
//! Every surface uses `inside_bright`: black seen from the front, red seen
//! from the back. The backdrop is blue. So red pixels mean the viewer sees
//! an inside -- through a crack, or because a surface faces the wrong way --
//! and nothing else in the frame is red.
//!
//! Needs 3Delight 2.9.210 with its `oslc`.
#![cfg(feature = "nurbs")]

mod common;

use common::{Facing, cube, cube_facing, cube_with};
use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Scene, write_stream};
use nsi_tessellate::{NurbsOptions, NurbsTessellation, nurbs_meshes};
use std::{num::NonZeroUsize, path::Path, process::Command};

const SIZE: usize = 192;

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

/// Compiles `name`.osl from `tests/shaders` into `directory`.
fn compile(directory: &Path, name: &str) -> String {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/shaders/{name}.osl"));
    let compiled = directory.join(format!("{name}.oso"));
    let oslc = Path::new(&std::env::var("DELIGHT").expect("$DELIGHT"))
        .join("bin/oslc");
    let status = Command::new(oslc)
        .arg("-o")
        .arg(&compiled)
        .arg(&source)
        .status()
        .expect("oslc");
    assert!(status.success());
    compiled.to_str().unwrap().to_string()
}

struct Shaders {
    surface: String,
    push: String,
    directory: std::path::PathBuf,
}

fn shaders() -> Shaders {
    let directory = std::env::temp_dir().join("nsi_tessellate_displacement");
    std::fs::create_dir_all(&directory).unwrap();
    Shaders {
        surface: compile(&directory, "inside_bright"),
        push: compile(&directory, "push_along_normal"),
        directory,
    }
}

/// What to render.
enum Subject<'a> {
    /// Tessellated meshes, with their normals.
    Meshes(&'a NurbsTessellation),
    /// The `nurbs` nodes themselves, for 3Delight to evaluate.
    Nurbs(&'a Scene),
}

/// Renders `subject`, displaced along its normals by `push`, and returns
/// how many pixels are red: insides in view.
fn red_pixels(
    subject: Subject<'_>,
    push: f32,
    name: &str,
    shaders: &Shaders,
) -> usize {
    let image = shaders.directory.join(format!("{name}.png"));
    let ctx = nsi::Context::new(None).expect("context");
    let delight = std::env::var("DELIGHT").expect("$DELIGHT");

    let target = [0.5, 0.5, 0.5];
    let eye = [3.0, 2.6, 3.4];
    ctx.create("camera_xform", nsi::TRANSFORM, None);
    ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "camera_xform",
        &[nsi::matrix4_f64!(
            "transformationmatrix",
            &looking(eye, target, eye)
        )],
    );
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.set_attribute("camera", &[nsi::real_f32!("fov", 30.0)]);
    ctx.connect("camera", None, "camera_xform", "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    let size = SIZE as i32;
    ctx.set_attribute(
        "screen",
        &[
            nsi::integer_i32_slice!("resolution", &[size, size])
                .array_len(NonZeroUsize::new(2).unwrap()),
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

    // A blue wall behind the subject, facing the camera.
    let behind = {
        let d = [eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        [
            target[0] - 3.0 * d[0] / l,
            target[1] - 3.0 * d[1] / l,
            target[2] - 3.0 * d[2] / l,
        ]
    };
    ctx.create("wall_xform", nsi::TRANSFORM, None);
    ctx.connect("wall_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "wall_xform",
        &[nsi::matrix4_f64!(
            "transformationmatrix",
            &looking(eye, target, behind)
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
        &[nsi::string!("shaderfilename", shaders.surface.as_str())],
    );
    ctx.create("push", nsi::SHADER, None);
    ctx.set_attribute(
        "push",
        &[
            nsi::string!("shaderfilename", shaders.push.as_str()),
            nsi::real_f32!("amount", push),
        ],
    );
    ctx.create("look", nsi::ATTRIBUTES, None);
    ctx.connect("inside", None, "look", "surfaceshader", None);
    ctx.connect("push", None, "look", "displacementshader", None);
    // 3Delight reads this undocumented attribute as the furthest a
    // displacement moves the surface, and displaces nothing without it.
    ctx.set_attribute("look", &[nsi::real_f32!("displacementbound", 1.0)]);

    match subject {
        Subject::Meshes(tessellation) => {
            for mesh in &tessellation.meshes {
                let handle = format!("mesh_{}", mesh.geometry);
                let positions: Vec<[f32; 3]> = mesh
                    .positions
                    .iter()
                    .map(|p| p.map(|c| c as f32))
                    .collect();
                let normals: Vec<[f32; 3]> =
                    mesh.normals.iter().map(|n| n.map(|c| c as f32)).collect();
                let indices: Vec<i32> = mesh
                    .triangles
                    .iter()
                    .flatten()
                    .map(|&i| i as i32)
                    .collect();
                let counts = vec![3; mesh.triangles.len()];
                ctx.create(&handle, nsi::MESH, None);
                ctx.connect(&handle, None, nsi::ROOT, "objects", None);
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
        }
        Subject::Nurbs(scene) => {
            let stream = shaders.directory.join(format!("{name}.nsi"));
            let mut out = Vec::new();
            write_stream(scene, &mut out).expect("write the scene");
            std::fs::write(&stream, out).unwrap();
            ctx.evaluate(&[
                nsi::string!("type", "apistream"),
                nsi::string!("filename", stream.to_str().unwrap()),
            ]);
            for (handle, node) in scene.nodes() {
                if node.node_type() == "nurbs" {
                    ctx.connect(
                        "look",
                        None,
                        handle,
                        "geometryattributes",
                        None,
                    );
                }
            }
        }
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

/// 3Delight's own convention for `nurbs`: a face's front is the side
/// `∂P/∂u × ∂P/∂v` points to. The outward cube shows only fronts; the
/// inward one, only backs. An earlier version of the tessellator assumed
/// the opposite, from reading an exporter's conventions, and this render
/// is what caught it.
#[test]
fn nurbs_fronts_face_the_standard_way() {
    let shaders = shaders();
    let outward = red_pixels(
        Subject::Nurbs(&cube_facing(false, Facing::Outward)),
        0.0,
        "nurbs_outward",
        &shaders,
    );
    let inward = red_pixels(
        Subject::Nurbs(&cube_facing(false, Facing::Inward)),
        0.0,
        "nurbs_inward",
        &shaders,
    );
    println!("nurbs: outward {outward} red px, inward {inward}");
    assert!(outward < 20, "the outward cube shows its fronts: {outward}");
    assert!(inward > 2000, "the inward cube shows its backs: {inward}");
}

/// The tessellated cube faces the way 3Delight's own `nurbs` do: the same
/// ɴsɪ patches, meshed, show their fronts from outside.
#[test]
fn tessellated_fronts_face_out() {
    let shaders = shaders();
    let meshes = nurbs_meshes(&cube(true), &NurbsOptions { tolerance: 0.002 });
    let red =
        red_pixels(Subject::Meshes(&meshes), 0.0, "mesh_outward", &shaders);
    assert!(red < 20, "the tessellated cube shows its fronts: {red}");
}

#[test]
fn displaced_welded_patches_stay_closed_and_unwelded_ones_crack() {
    let shaders = shaders();
    let options = NurbsOptions { tolerance: 0.002 };
    let welded = nurbs_meshes(&cube(true), &options);
    let unwelded = nurbs_meshes(&cube(false), &options);
    assert_eq!(welded.open_edges, 0);
    assert!(unwelded.open_edges > 0);

    let push = 0.15;
    let welded_red =
        red_pixels(Subject::Meshes(&welded), push, "welded", &shaders);
    let unwelded_red =
        red_pixels(Subject::Meshes(&unwelded), push, "unwelded", &shaders);
    println!(
        "displaced by {push}: welded {welded_red} red px, unwelded {unwelded_red}"
    );

    assert!(
        welded_red < 20,
        "the welded cube stays closed: {welded_red}"
    );
    assert!(
        unwelded_red > 200,
        "the unwelded cube cracks: {unwelded_red}"
    );
}

/// The same end to end with natural sides: faces z = 0, y = 0 and x = 0
/// untrimmed and welded by `nurbs-side`, the rest by trim curves, and every
/// cube edge split in two by `weld.range`.
#[test]
fn displaced_side_welded_patches_stay_closed_and_unwelded_ones_crack() {
    let shaders = shaders();
    let options = NurbsOptions { tolerance: 0.002 };
    let sides = [true, false, true, false, true, false];
    let welded =
        nurbs_meshes(&cube_with(true, Facing::Outward, sides, true), &options);
    let unwelded =
        nurbs_meshes(&cube_with(false, Facing::Outward, sides, true), &options);
    assert_eq!(welded.open_edges, 0);
    assert!(unwelded.open_edges > 0);

    let push = 0.15;
    let welded_red =
        red_pixels(Subject::Meshes(&welded), push, "sides_welded", &shaders);
    let unwelded_red = red_pixels(
        Subject::Meshes(&unwelded),
        push,
        "sides_unwelded",
        &shaders,
    );
    println!(
        "sides, displaced by {push}: welded {welded_red} red px, unwelded {unwelded_red}"
    );
    assert!(welded_red < 20, "the welded cube stays closed: {welded_red}");
    assert!(unwelded_red > 200, "the unwelded cube cracks: {unwelded_red}");
}
