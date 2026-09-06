//! Tests for [`super`].
//!
//! Separate file per the workspace rule: source files do not grow
//! inline `#[cfg(test)]` modules.

use crate::{OwnedArg, OwnedData, ResolveError, Scene};
use nsi_trait::Type;

/// A 4x4 row-major translation, the shape ɴsɪ stores in
/// `transformationmatrix`.
fn translate(x: f64, y: f64, z: f64) -> OwnedArg {
    #[rustfmt::skip]
    let m = vec![
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
          x,   y,   z, 1.0,
    ];
    OwnedArg {
        name: "transformationmatrix".to_string(),
        type_tag: Type::MatrixF64,
        array_length: 1,
        flags: 0,
        data: OwnedData::F64(m),
    }
}

fn scale(s: f64) -> OwnedArg {
    #[rustfmt::skip]
    let m = vec![
          s, 0.0, 0.0, 0.0,
        0.0,   s, 0.0, 0.0,
        0.0, 0.0,   s, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    OwnedArg {
        name: "transformationmatrix".to_string(),
        type_tag: Type::MatrixF64,
        array_length: 1,
        flags: 0,
        data: OwnedData::F64(m),
    }
}

#[test]
fn a_node_with_no_transforms_is_identity() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    assert_eq!(scene.world_transform("mesh").unwrap(), super::IDENTITY);
}

/// ɴsɪ: "A node can exist in an nsi context without being connected
/// to the root node but in that case it won't affect the render in
/// any way." Answering identity would put unrendered geometry at the
/// origin of a backend that iterates `scene.nodes`.
#[test]
fn a_detached_node_is_an_error_not_identity() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    assert_eq!(
        scene.world_transform("mesh"),
        Err(ResolveError::Detached {
            handle: "mesh".to_string()
        })
    );
}

/// A node under a transform that is itself detached is detached too;
/// the walk reports the node that failed to reach the root.
#[test]
fn detachment_is_reported_at_the_node_that_fails_to_reach_root() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    assert_eq!(
        scene.world_transform("mesh"),
        Err(ResolveError::Detached {
            handle: "grp".to_string()
        })
    );
}

#[test]
fn a_single_transform_applies_to_its_child() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![translate(1.0, 2.0, 3.0)])
        .unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    let m = scene.world_transform("mesh").unwrap();
    assert_eq!(&m[12..15], &[1.0, 2.0, 3.0]);
}

/// Nested translations accumulate.
#[test]
fn nested_transforms_compose() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene
        .set_attribute("outer", vec![translate(10.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inner", "transform").unwrap();
    scene
        .set_attribute("inner", vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "inner", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();

    let m = scene.world_transform("mesh").unwrap();
    assert_eq!(m[12], 11.0);
}

/// Order matters, and this is the test that catches composing the
/// chain backwards. ɴsɪ is row-vector (RenderMan) convention, so a
/// child's matrix applies before its parent's: scaling by 2 under a
/// translation of 10 puts the origin at 10, whereas translating
/// under a scale would put it at 20.
#[test]
fn child_transform_applies_before_parent() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene
        .set_attribute("outer", vec![translate(10.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inner", "transform").unwrap();
    scene.set_attribute("inner", vec![scale(2.0)]).unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "inner", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();

    let m = scene.world_transform("mesh").unwrap();
    assert_eq!(m[0], 2.0, "scale survives");
    assert_eq!(m[12], 10.0, "translation is not scaled");
}

/// A transform node's own matrix counts, not just its ancestors'.
#[test]
fn a_transforms_own_matrix_is_included() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    assert_eq!(scene.world_transform("xf").unwrap()[12], 5.0);
}

/// A cycle must not hang the resolver, and must not answer either.
/// ɴsɪ does not forbid one; no correct transform exists for it.
#[test]
fn a_cycle_is_an_error() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, "b", "objects").unwrap();
    scene.connect("b", None, "a", "objects").unwrap();
    assert_eq!(
        scene.world_transform("a"),
        Err(ResolveError::Cycle {
            handle: "a".to_string()
        })
    );
}

/// Two `objects` parents is ɴsɪ's lightweight instancing: the node
/// exists once per path, each with its own world transform. One
/// matrix cannot say that, so refusing beats answering for whichever
/// parent was connected first.
#[test]
fn more_than_one_parent_is_an_error() {
    let mut scene = Scene::default();
    scene.create("left", "transform").unwrap();
    scene
        .set_attribute("left", vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene.create("right", "transform").unwrap();
    scene
        .set_attribute("right", vec![translate(9.0, 0.0, 0.0)])
        .unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "left", "objects").unwrap();
    scene.connect("mesh", None, "right", "objects").unwrap();

    assert_eq!(
        scene.world_transform("mesh"),
        Err(ResolveError::MultipleParents {
            handle: "mesh".to_string(),
            parents: vec!["left".to_string(), "right".to_string()],
        })
    );
}

/// `world_transform` reads static attributes only, so a
/// motion-sampled chain has no static matrix to read. Answering
/// identity would hand a motion-blurred scene back an unblurred
/// pose, so it is an error until per-sample composition exists.
#[test]
fn a_motion_sampled_transform_is_an_error() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    assert_eq!(
        scene.world_transform("mesh"),
        Err(ResolveError::MotionSampledTransform {
            handle: "xf".to_string()
        })
    );
}

/// A static transform on a node that also carries unrelated motion
/// samples still resolves; only a sampled *transform* is refused.
#[test]
fn motion_samples_of_other_attributes_do_not_block_resolution() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time(
            "xf",
            0.5,
            vec![OwnedArg {
                name: "unrelated".to_string(),
                type_tag: Type::F64,
                array_length: 1,
                flags: 0,
                data: OwnedData::F64(vec![1.0]),
            }],
        )
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    assert_eq!(scene.world_transform("xf").unwrap()[12], 5.0);
}

/// The motion API's reason to exist: two samples give two different
/// world transforms, where `world_transform` refuses outright.
#[test]
fn a_sampled_chain_resolves_per_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    assert_eq!(scene.world_transform_at("mesh", 0.0).unwrap()[12], 0.0);
    assert_eq!(scene.world_transform_at("mesh", 1.0).unwrap()[12], 5.0);
}

/// A static node is constant, so it contributes at every time. This
/// is the common shape: a moving object under a fixed group.
#[test]
fn a_static_parent_composes_with_a_sampled_child() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene
        .set_attribute("grp", vec![translate(100.0, 0.0, 0.0)])
        .unwrap();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("xf", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();

    assert_eq!(scene.world_transform_at("xf", 0.0).unwrap()[12], 100.0);
    assert_eq!(scene.world_transform_at("xf", 1.0).unwrap()[12], 105.0);
}

/// The union of every sample time in the chain, ascending and
/// deduplicated -- what a backend iterates to build motion blur.
#[test]
fn motion_times_are_the_union_of_the_chain() {
    let mut scene = Scene::default();
    // `inner` is walked first and its only time sorts last, so a
    // merge that just concatenated the chain would come out
    // unsorted. `0.0` appears on both, so it must also dedup.
    scene.create("outer", "transform").unwrap();
    scene
        .set_attribute_at_time("outer", 0.5, vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("outer", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inner", "transform").unwrap();
    scene
        .set_attribute_at_time("inner", 2.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();

    assert_eq!(scene.motion_times("inner").unwrap(), vec![0.0, 0.5, 2.0]);
}

/// A static chain has no motion times, which is how a backend tells
/// the two cases apart.
#[test]
fn a_static_chain_has_no_motion_times() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    // A motion sample of something that is not a transform. The
    // chain is still static as far as transforms go, and counting
    // this would invent motion blur out of an animated colour.
    scene
        .set_attribute_at_time(
            "xf",
            0.5,
            vec![OwnedArg {
                name: "unrelated".to_string(),
                type_tag: Type::F64,
                array_length: 1,
                flags: 0,
                data: OwnedData::F64(vec![1.0]),
            }],
        )
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    assert!(scene.motion_times("xf").unwrap().is_empty());
    // And it resolves at any time, agreeing with the static answer.
    assert_eq!(
        scene.world_transform_at("xf", 0.25).unwrap(),
        scene.world_transform("xf").unwrap()
    );
}

/// `world_transform_samples` is the pair of the two: the times, and
/// the composed matrix at each.
#[test]
fn samples_pair_every_time_with_its_matrix() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.5, vec![translate(2.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    let samples = scene.world_transform_samples("xf").unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].0, 0.0);
    assert_eq!(samples[0].1[12], 0.0);
    assert_eq!(samples[1].0, 0.5);
    assert_eq!(samples[1].1[12], 2.0);
}

/// Asking a sampled node at a time it does not have is an error, not
/// an interpolation. Element-wise interpolation of a matrix is wrong
/// for anything with a rotation in it, and the right decomposition
/// is the backend's to choose.
#[test]
fn a_time_between_samples_is_an_error_not_an_interpolation() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();

    assert_eq!(
        scene.world_transform_at("xf", 0.5),
        Err(ResolveError::MissingSampleAtTime {
            handle: "xf".to_string(),
            time: 0.5,
            available: vec![0.0, 1.0],
        })
    );
}

/// A chain whose nodes are sampled at **different** times resolves.
///
/// This asserted an error, on the reasoning that a sweep needs every
/// node to have a sample at each time. 3Delight renders such a scene --
/// `outer` sampled at 0 and 0.5, `inner` at 0.25 and 0.75, shutter
/// [0, 1], draws a smear across six bands with no error -- because each
/// node is interpolated at each time independently. Refusing it was a
/// refusal where the renderer answers, and the crate-level
/// documentation steered a backend straight into it.
///
/// `world_transform_samples` interpolates now, so it agrees with
/// `world_transform_interpolated_at` at every time it reports.
#[test]
fn a_chain_sampled_at_different_times_resolves() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene
        .set_attribute_at_time("outer", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("outer", 0.5, vec![translate(2.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inner", "transform").unwrap();
    scene
        .set_attribute_at_time("inner", 0.25, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 0.75, vec![translate(4.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();

    let times = scene.motion_times("inner").unwrap();
    assert_eq!(times, vec![0.0, 0.25, 0.5, 0.75], "the union of both");

    let samples = scene.world_transform_samples("inner").unwrap();
    assert_eq!(samples.len(), 4);
    for (time, matrix) in &samples {
        assert_eq!(
            *matrix,
            scene
                .world_transform_interpolated_at("inner", *time)
                .unwrap(),
            "the sweep agrees with the interpolating accessor at {time}",
        );
    }

    // At t=0.5 the outer is at its own sample (2.0) and the inner is
    // halfway between its two (2.0): 4.0 together.
    assert_eq!(samples[2].1[12], 4.0);
}

/// ɴsɪ documents `transformationmatrix` as `doublematrix`. An `f32`
/// one is skipped rather than reinterpreted, and this pins that the
/// skip is deliberate: the same numbers as `MatrixF64` resolve, as
/// `MatrixF32` they do not.
#[test]
fn a_non_f64_matrix_is_skipped_not_reinterpreted() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    #[rustfmt::skip]
    let m = vec![
        1.0f32, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        7.0, 0.0, 0.0, 1.0,
    ];
    scene
        .set_attribute(
            "xf",
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::MatrixF32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(m),
            }],
        )
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    assert_eq!(scene.world_transform("xf").unwrap(), super::IDENTITY);
}

/// An ɴsɪ `"index"` connection argument.
fn index_arg(value: i32) -> OwnedArg {
    OwnedArg {
        name: "index".to_string(),
        ..priority(value)
    }
}

/// An ɴsɪ `"priority"` connection argument.
fn priority(value: i32) -> OwnedArg {
    OwnedArg {
        name: "priority".to_string(),
        type_tag: Type::I32,
        array_length: 1,
        flags: 0,
        data: OwnedData::I32(vec![value]),
    }
}

/// The canonical ɴsɪ shape: shader -> attributes -> geometry, with
/// the geometry actually in the scene.
fn scene_with_material() -> Scene {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();
    scene
}

#[test]
fn dissolves_attributes_to_a_shader() {
    let scene = scene_with_material();
    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["attr".to_string()]);
    assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
}

#[test]
fn unbound_geometry_has_no_binding() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    assert!(scene.geometry_binding("mesh").unwrap().is_none());
}

/// An attributes node with no shader still binds -- it may carry
/// only visibility flags.
#[test]
fn attributes_without_a_shader_still_bind() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "mesh", "geometryattributes")
        .unwrap();
    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["attr".to_string()]);
    assert!(binding.surface_shader.is_none());
}

/// One attributes node bound to several shapes must resolve for each
/// of them. This is the fan-out the spec calls out.
#[test]
fn one_attributes_node_fans_out_to_every_shape() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();
    for mesh in ["a", "b", "c"] {
        scene.create(mesh, "mesh").unwrap();
        scene.connect(mesh, None, ".root", "objects").unwrap();
        scene
            .connect("attr", None, mesh, "geometryattributes")
            .unwrap();
    }
    for mesh in ["a", "b", "c"] {
        let binding = scene.geometry_binding(mesh).unwrap().expect("bound");
        assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
    }
}

/// ɴsɪ binds `geometryattributes` to a transform as readily as to a
/// geometry, and a binding on a transform applies to everything
/// beneath it. Resolving only direct edges would leave every shape
/// under a bound transform unmaterialled.
#[test]
fn a_binding_on_an_ancestor_transform_is_inherited() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "grp", "geometryattributes")
        .unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["attr".to_string()]);
    assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
}

/// ɴsɪ describes the root as "much like a transform node", with its
/// own `geometryattributes`. A scene-wide attributes node is bound
/// there, and gathering that stops at `.root` would never see it.
#[test]
fn a_binding_on_the_root_is_gathered() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("global_attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("global_attr", None, ".root", "geometryattributes")
        .unwrap();
    scene
        .connect("shader", None, "global_attr", "surfaceshader")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["global_attr".to_string()]);
    assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
}

/// The one ɴsɪ says out loud: "one attributes node can set object
/// visibility and another can set the surface shader ... and will
/// all be considered". A winner-take-all resolver returns the
/// nearest node and silently loses the shader on the other.
#[test]
fn every_attributes_node_on_the_path_is_gathered() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.create("shaded", "attributes").unwrap();
    scene.create("visibility", "attributes").unwrap();
    scene.create("metal", "shader").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    // The shader lives on the group's attributes node...
    scene
        .connect("shaded", None, "grp", "geometryattributes")
        .unwrap();
    scene
        .connect("metal", None, "shaded", "surfaceshader")
        .unwrap();
    // ...and visibility on the mesh's own, which is nearer.
    scene
        .connect("visibility", None, "mesh", "geometryattributes")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(
        binding.attributes,
        vec!["visibility".to_string(), "shaded".to_string()],
        "both nodes gathered, nearest first"
    );
    assert_eq!(
        binding.surface_shader.as_deref(),
        Some("metal"),
        "the shader survives being on the farther node"
    );
}

/// At equal priority the more specific definition wins: ɴsɪ selects
/// "the definition that is the closest to the geometric primitive".
#[test]
fn the_nearest_binding_wins_at_equal_priority() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.create("outer", "attributes").unwrap();
    scene.create("own", "attributes").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    scene
        .connect("outer", None, "grp", "geometryattributes")
        .unwrap();
    scene
        .connect("own", None, "mesh", "geometryattributes")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes[0], "own");
}

/// The `priority` on a `geometryattributes` connection does **not**
/// reorder the gathered nodes.
///
/// This test previously asserted the opposite, quoting ɴsɪ's
/// "`priority` ... indicates in which order the nodes should be
/// considered when evaluating the value of an attribute". Rendered in
/// 3Delight, the connection priority does nothing: `outer` carries
/// `priority` 10 and `own` still wins, because it is nearer. Moving the
/// same 10 onto `outer` as an `ATTR.priority` *does* flip it, which is
/// `attr_priority_beats_proximity`. The renderer is the oracle, so the
/// expectation was corrected to what it does.
#[test]
fn a_geometryattributes_connection_priority_does_not_reorder() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.create("outer", "attributes").unwrap();
    scene.create("own", "attributes").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    scene
        .connect_with_args(
            "outer",
            None,
            "grp",
            "geometryattributes",
            vec![priority(10)],
        )
        .unwrap();
    scene
        .connect("own", None, "mesh", "geometryattributes")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(
        binding.attributes[0], "own",
        "proximity decides; the connection priority is inert",
    );
}

/// A `surfaceshader` connection carries its own priority, "useful
/// for overriding a shader from higher in the scene graph".
#[test]
fn a_surfaceshader_connection_priority_wins() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.create("outer", "attributes").unwrap();
    scene.create("own", "attributes").unwrap();
    scene.create("far_shader", "shader").unwrap();
    scene.create("near_shader", "shader").unwrap();
    scene.connect("mesh", None, "grp", "objects").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    scene
        .connect("outer", None, "grp", "geometryattributes")
        .unwrap();
    scene
        .connect("own", None, "mesh", "geometryattributes")
        .unwrap();
    // The nearer node's shader would win on proximity alone.
    scene
        .connect("near_shader", None, "own", "surfaceshader")
        .unwrap();
    scene
        .connect_with_args(
            "far_shader",
            None,
            "outer",
            "surfaceshader",
            vec![priority(5)],
        )
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.surface_shader.as_deref(), Some("far_shader"));
}

/// An instancing prototype reaches the scene through its
/// `instances` node, never through `.root` directly. Calling it
/// detached would leave every prototype in a `GeometrySet` with no
/// material.
#[test]
fn an_instancing_prototype_is_not_detached() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.create("metal", "shader").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene
        .connect("attr", None, "proto", "geometryattributes")
        .unwrap();
    scene
        .connect("metal", None, "attr", "surfaceshader")
        .unwrap();

    let binding = scene.geometry_binding("proto").unwrap().expect("bound");
    assert_eq!(binding.surface_shader.as_deref(), Some("metal"));
}

/// ...but it has no single world transform. ɴsɪ gives an
/// `instances` node "a transformation matrix for each instance", so
/// answering with the instancer's own would put every instance in
/// the same wrong place. Attributes gather through it; transforms
/// stop at it.
#[test]
fn a_prototype_has_no_single_world_transform() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("inst", None, "xf", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    assert_eq!(
        scene.world_transform("proto"),
        Err(crate::ResolveError::Instanced {
            instancer: "inst".to_string()
        })
    );
}

/// A prototype may also be placed directly. ɴsɪ gathers "through
/// all the transform nodes it is connected to", so the direct
/// placement is the path and the `instances` connection is not a
/// second parent -- reporting one made a legal scene unresolvable.
#[test]
fn a_prototype_placed_directly_resolves_by_that_path() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![translate(4.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    // Both a child of `xf` and a prototype of `inst`.
    scene.connect("proto", None, "xf", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    assert_eq!(
        scene.world_transform("proto").unwrap()[12],
        4.0,
        "the direct placement is the answer"
    );
    assert!(scene.geometry_binding("proto").is_ok());
}

/// A prototype shared by two instancers has as little of a single
/// answer as a node with two parents.
#[test]
fn a_prototype_of_two_instancers_is_ambiguous() {
    let mut scene = Scene::default();
    scene.create("proto", "mesh").unwrap();
    for inst in ["one", "two"] {
        scene.create(inst, "instances").unwrap();
        scene.connect(inst, None, ".root", "objects").unwrap();
        scene.connect("proto", None, inst, "sourcemodels").unwrap();
    }

    assert!(matches!(
        scene.geometry_binding("proto"),
        Err(crate::ResolveError::MultipleParents { .. })
    ));
}

/// ɴsɪ orders instancing prototypes by the connection's `index`
/// attribute, which `modelindices` then selects into -- not by
/// connection order.
#[test]
fn instance_sources_are_ordered_by_their_index_attribute() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    for (handle, index) in [("third", 2), ("first", 0), ("second", 1)] {
        scene.create(handle, "mesh").unwrap();
        scene
            .connect_with_args(
                handle,
                None,
                "inst",
                "sourcemodels",
                vec![priority(0), index_arg(index)],
            )
            .unwrap();
    }

    assert_eq!(
        scene.instance_sources("inst"),
        vec!["first", "second", "third"],
        "connection order was third, first, second"
    );
}

/// `attributes` is ordered by ɴsɪ's precedence, and the shader must
/// agree with it. Picking the last maximal candidate instead of the
/// first returns a shader from a node that lost the ordering.
#[test]
fn the_shader_agrees_with_the_gathered_order() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("first", "attributes").unwrap();
    scene.create("second", "attributes").unwrap();
    scene.create("wanted", "shader").unwrap();
    scene.create("loser", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    // Both bound to the same node at the same priority, so only
    // connection order separates them.
    scene
        .connect("first", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("second", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("wanted", None, "first", "surfaceshader")
        .unwrap();
    scene
        .connect("loser", None, "second", "surfaceshader")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes[0], "first");
    assert_eq!(
        binding.surface_shader.as_deref(),
        Some("wanted"),
        "the shader must come from attributes[0], not the last match"
    );
}

/// ɴsɪ's `attributes` node has three shader slots. Rejecting the
/// other two made every displaced or volumetric scene unrecordable.
#[test]
fn displacement_and_volume_shaders_resolve_too() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.create("surf", "shader").unwrap();
    scene.create("disp", "shader").unwrap();
    scene.create("vol", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("surf", None, "attr", "surfaceshader")
        .unwrap();
    scene
        .connect("disp", None, "attr", "displacementshader")
        .unwrap();
    scene.connect("vol", None, "attr", "volumeshader").unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.surface_shader.as_deref(), Some("surf"));
    assert_eq!(binding.displacement_shader.as_deref(), Some("disp"));
    assert_eq!(binding.volume_shader.as_deref(), Some("vol"));
}

/// A prototype's own subtree has no world transform, but it does
/// have one relative to the prototype root -- which is the space the
/// per-instance matrix is applied in. Without this a backend has to
/// re-derive composition from the edge list, which is the walk this
/// crate exists to own.
#[test]
fn a_prototype_subtree_resolves_relative_to_the_prototype() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "transform").unwrap();
    scene
        .set_attribute("proto", vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene.create("part", "transform").unwrap();
    scene
        .set_attribute("part", vec![translate(0.0, 2.0, 0.0)])
        .unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene.connect("part", None, "proto", "objects").unwrap();

    // World space is unavailable, as it should be.
    assert!(scene.world_transform("part").is_err());

    // Relative to the prototype it is `part` then `proto`.
    let m = scene.relative_transform("part", "inst").unwrap();
    assert_eq!(&m[12..15], &[1.0, 2.0, 0.0]);

    // And excluding the prototype itself.
    let m = scene.relative_transform("part", "proto").unwrap();
    assert_eq!(&m[12..15], &[0.0, 2.0, 0.0]);
}

/// Composing *past* an `instances` node folds in the instancer's own
/// matrix and leaves out the per-instance one -- a plausible wrong
/// answer for the exact query this method exists to serve.
#[test]
fn relative_transform_refuses_to_cross_an_instancer() {
    let mut scene = Scene::default();
    scene.create("placer", "transform").unwrap();
    scene
        .set_attribute("placer", vec![translate(100.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inst", "instances").unwrap();
    scene
        .set_attribute("inst", vec![translate(1000.0, 0.0, 0.0)])
        .unwrap();
    scene.create("proto", "transform").unwrap();
    scene
        .set_attribute("proto", vec![translate(10.0, 0.0, 0.0)])
        .unwrap();
    scene.create("leaf", "mesh").unwrap();
    scene.connect("placer", None, ".root", "objects").unwrap();
    scene.connect("inst", None, "placer", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene.connect("leaf", None, "proto", "objects").unwrap();

    for ancestor in [".root", "placer"] {
        assert!(
            matches!(
                scene.relative_transform("leaf", ancestor),
                Err(ResolveError::Instanced { .. })
            ),
            "composing past the instancer to {ancestor} must refuse"
        );
    }

    // Stopping at the instancer is the supported query.
    assert_eq!(scene.relative_transform("leaf", "inst").unwrap()[12], 10.0);
}

/// A node that is not on the chain has no relative transform.
#[test]
fn relative_transform_rejects_a_node_off_the_chain() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("elsewhere", "transform").unwrap();
    scene.connect("a", None, ".root", "objects").unwrap();
    scene
        .connect("elsewhere", None, ".root", "objects")
        .unwrap();

    assert_eq!(
        scene.relative_transform("a", "elsewhere"),
        Err(crate::ResolveError::NotAnAncestor {
            handle: "a".to_string(),
            ancestor: "elsewhere".to_string(),
        })
    );
}

/// A cyclic chain has no binding either -- the walk that finds
/// ancestors is the same one that composes transforms.
#[test]
fn a_cycle_is_an_error_for_bindings_too() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, "b", "objects").unwrap();
    scene.connect("b", None, "a", "objects").unwrap();
    assert!(scene.geometry_binding("a").is_err());
}

/// The canonical ɴsɪ output chain:
/// driver -> layer -> screen -> camera.
fn scene_with_output() -> Scene {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    scene.create("scr", "screen").unwrap();
    scene.create("beauty", "outputlayer").unwrap();
    scene.create("drv", "outputdriver").unwrap();
    scene.connect("scr", None, "cam", "screens").unwrap();
    scene
        .connect("beauty", None, "scr", "outputlayers")
        .unwrap();
    scene
        .connect("drv", None, "beauty", "outputdrivers")
        .unwrap();
    scene
}

#[test]
fn resolves_the_whole_output_chain() {
    let scene = scene_with_output();
    let outputs = scene.render_outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].camera, "cam");
    assert_eq!(outputs[0].screen, "scr");
    assert_eq!(outputs[0].layers.len(), 1);
    assert_eq!(outputs[0].layers[0].handle, "beauty");
    assert_eq!(outputs[0].layers[0].drivers, vec!["drv".to_string()]);
}

/// A screen with no layers is still a render output -- the camera
/// and resolution are meaningful on their own.
#[test]
fn a_screen_without_layers_still_resolves() {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    scene.create("scr", "screen").unwrap();
    scene.connect("scr", None, "cam", "screens").unwrap();
    let outputs = scene.render_outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].layers.is_empty());
}

/// Several AOVs on one screen, in connection order.
#[test]
fn multiple_layers_keep_connection_order() {
    let mut scene = scene_with_output();
    scene.create("depth", "outputlayer").unwrap();
    scene.connect("depth", None, "scr", "outputlayers").unwrap();
    let outputs = scene.render_outputs();
    let names: Vec<&str> = outputs[0]
        .layers
        .iter()
        .map(|l| l.handle.as_str())
        .collect();
    assert_eq!(names, vec!["beauty", "depth"]);
}

/// One layer fanned out to two drivers -- a file and a display.
#[test]
fn a_layer_may_have_several_drivers() {
    let mut scene = scene_with_output();
    scene.create("drv2", "outputdriver").unwrap();
    scene
        .connect("drv2", None, "beauty", "outputdrivers")
        .unwrap();
    let outputs = scene.render_outputs();
    assert_eq!(outputs[0].layers[0].drivers, vec!["drv", "drv2"]);
}

#[test]
fn no_screen_means_no_outputs() {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    assert!(scene.render_outputs().is_empty());
}

/// Two cameras, two screens. Every other test uses one, which would
/// not catch a resolver that collapsed them.
#[test]
fn multiple_screens_yield_one_output_each() {
    let mut scene = Scene::default();
    for (cam, scr, layer) in [
        ("cam_a", "scr_a", "beauty_a"),
        ("cam_b", "scr_b", "beauty_b"),
    ] {
        scene.create(cam, "perspectivecamera").unwrap();
        scene.create(scr, "screen").unwrap();
        scene.create(layer, "outputlayer").unwrap();
        scene.connect(scr, None, cam, "screens").unwrap();
        scene.connect(layer, None, scr, "outputlayers").unwrap();
    }

    let outputs = scene.render_outputs();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].camera, "cam_a");
    assert_eq!(outputs[0].screen, "scr_a");
    assert_eq!(outputs[0].layers[0].handle, "beauty_a");
    assert_eq!(outputs[1].camera, "cam_b");
    assert_eq!(outputs[1].screen, "scr_b");
    assert_eq!(outputs[1].layers[0].handle, "beauty_b");
}

fn doubles(name: &str, values: Vec<f64>) -> OwnedArg {
    OwnedArg {
        name: name.to_string(),
        type_tag: Type::MatrixF64,
        array_length: 1,
        flags: 0,
        data: OwnedData::F64(values),
    }
}

fn integers(name: &str, values: Vec<i32>) -> OwnedArg {
    OwnedArg {
        name: name.to_string(),
        type_tag: Type::I32,
        array_length: 1,
        flags: 0,
        data: OwnedData::I32(values),
    }
}

fn instance_matrix(x: f64) -> Vec<f64> {
    vec![
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, 0.0,
        0.0, 1.0,
    ]
}

/// ɴsɪ gives an `instances` node "a transformation matrix for each
/// instance" and a `modelindices` "matched to the index attribute of
/// the model connection". Nothing paired them, so a backend could
/// read the matrices but not know which prototype each one drew.
///
/// The indices here are deliberately not their positions: matching
/// by position passes on a scene numbered 0, 1, 2 and silently draws
/// the wrong prototype on any other.
#[test]
fn instances_pair_their_matrix_with_their_prototype() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    for (handle, index) in [("five", 5), ("nine", 9)] {
        scene.create(handle, "mesh").unwrap();
        scene
            .connect_with_args(
                handle,
                None,
                "inst",
                "sourcemodels",
                vec![integers("index", vec![index])],
            )
            .unwrap();
    }

    let mut matrices = instance_matrix(10.0);
    matrices.extend(instance_matrix(20.0));
    scene
        .set_attribute(
            "inst",
            vec![
                doubles("transformationmatrices", matrices),
                // First instance draws index 9, second draws index 5.
                integers("modelindices", vec![9, 5]),
            ],
        )
        .unwrap();

    assert_eq!(scene.instance_sources("inst"), vec!["five", "nine"]);

    let placed = scene.instance_transforms("inst").unwrap();
    assert_eq!(placed.len(), 2);
    assert_eq!(placed[0].source, 1, "index 9 is `nine`, at position 1");
    assert_eq!(placed[0].transform[12], 10.0);
    assert_eq!(placed[1].source, 0, "index 5 is `five`, at position 0");
    assert_eq!(placed[1].transform[12], 20.0);
}

/// ɴsɪ: "a negative value will cause an instance to not be
/// rendered". A prototype connected at a negative index would
/// otherwise be matched by one.
#[test]
fn a_negative_model_index_is_not_rendered() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("hidden", "mesh").unwrap();
    scene
        .connect_with_args(
            "hidden",
            None,
            "inst",
            "sourcemodels",
            vec![integers("index", vec![-1])],
        )
        .unwrap();
    scene
        .set_attribute(
            "inst",
            vec![
                doubles("transformationmatrices", instance_matrix(1.0)),
                integers("modelindices", vec![-1]),
            ],
        )
        .unwrap();

    assert!(
        scene.instance_transforms("inst").unwrap().is_empty(),
        "a negative index is not a lookup key"
    );
}

/// ɴsɪ's `disabledinstances` is "a list of indices of instances which
/// are not to be rendered".
#[test]
fn disabled_instances_are_omitted() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("a", "mesh").unwrap();
    scene.connect("a", None, "inst", "sourcemodels").unwrap();
    let mut matrices = instance_matrix(1.0);
    matrices.extend(instance_matrix(2.0));
    scene
        .set_attribute(
            "inst",
            vec![
                doubles("transformationmatrices", matrices),
                integers("disabledinstances", vec![0]),
            ],
        )
        .unwrap();

    let placed = scene.instance_transforms("inst").unwrap();
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].transform[12], 2.0);
}

/// No matrices, no instances.
#[test]
fn an_instances_node_without_matrices_places_nothing() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    assert!(scene.instance_transforms("inst").unwrap().is_empty());
}

#[test]
fn resolves_instance_source_models() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto_a", "mesh").unwrap();
    scene.create("proto_b", "mesh").unwrap();
    scene
        .connect("proto_a", None, "inst", "sourcemodels")
        .unwrap();
    scene
        .connect("proto_b", None, "inst", "sourcemodels")
        .unwrap();
    assert_eq!(scene.instance_sources("inst"), vec!["proto_a", "proto_b"]);
}

#[test]
fn an_instances_node_with_no_sources_is_empty() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    assert!(scene.instance_sources("inst").is_empty());
}

/// Half a matrix is not a matrix. Silently keeping the whole ones was a
/// truncation the caller could not see.
#[test]
fn a_ragged_matrix_buffer_is_an_error() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("a", "mesh").unwrap();
    scene.connect("a", None, "inst", "sourcemodels").unwrap();
    let mut ragged = instance_matrix(1.0);
    ragged.push(0.0);
    scene
        .set_attribute("inst", vec![doubles("transformationmatrices", ragged)])
        .unwrap();

    assert!(matches!(
        scene.instance_transforms("inst"),
        Err(ResolveError::MalformedInstanceMatrices { values: 17, .. })
    ));
}

/// A `modelindices` entry naming no prototype is a malformed scene, not
/// an instance to skip.
#[test]
fn a_model_index_matching_no_prototype_is_an_error() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("a", "mesh").unwrap();
    scene.connect("a", None, "inst", "sourcemodels").unwrap();
    scene
        .set_attribute(
            "inst",
            vec![
                doubles("transformationmatrices", instance_matrix(1.0)),
                integers("modelindices", vec![7]),
            ],
        )
        .unwrap();

    assert!(matches!(
        scene.instance_transforms("inst"),
        Err(ResolveError::UnknownModelIndex { model: 7, .. })
    ));
}

/// A shader-network edge's `to_attr` is its *port* name, so it lands in
/// the same index bucket as a class of that name. Without the kind
/// filter, a port called `surfaceshader` resolved as the material.
#[test]
fn a_shader_network_port_does_not_resolve_as_its_namesake_class() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene.create("tex", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "mesh", "geometryattributes")
        .unwrap();
    // A *port* named like the class, not a node-level connection.
    scene
        .connect("tex", Some("outColor"), "attr", "surfaceshader")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(
        binding.surface_shader, None,
        "a port edge is carried, not resolved as the material"
    );
}

/// And the same at the attribute-binding position.
#[test]
fn a_port_named_like_a_binding_does_not_bind() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("tex", "shader").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("tex", Some("outColor"), "mesh", "geometryattributes")
        .unwrap();

    assert!(
        scene.geometry_binding("mesh").unwrap().is_none(),
        "a port edge must not become an attributes node"
    );
}

/// ɴsɪ requires distinct `index` attributes so the models "effectively
/// form an ordered list". Two at the same index is a malformed scene,
/// and picking the first was a guess.
#[test]
fn duplicate_model_indices_are_refused() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    for handle in ["a", "b"] {
        scene.create(handle, "mesh").unwrap();
        scene
            .connect_with_args(
                handle,
                None,
                "inst",
                "sourcemodels",
                vec![index_arg(0)],
            )
            .unwrap();
    }
    scene
        .set_attribute(
            "inst",
            vec![doubles("transformationmatrices", instance_matrix(1.0))],
        )
        .unwrap();

    assert!(matches!(
        scene.instance_transforms("inst"),
        Err(crate::ResolveError::DuplicateModelIndex { index: 0, .. })
    ));
}

// ---------------------------------------------------------------------
// `attribute_value`: ɴsɪ's two attribute-level precedence rules.
// ---------------------------------------------------------------------

/// `mesh -> xf -> .root`, with an `attributes` node on each level.
/// `near` sits on the geometry, `far` on the transform.
fn scene_with_two_attribute_levels() -> Scene {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("near", "attributes").unwrap();
    scene.create("far", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .connect("near", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("far", None, "xf", "geometryattributes")
        .unwrap();
    scene
}

/// The baseline the whole feature rests on: with no priority anywhere,
/// ɴsɪ takes "the definition that is the closest to the geometric
/// primitive".
#[test]
fn at_equal_priority_the_nearest_definition_wins() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility", vec![1])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

/// ɴsɪ: "the definition with the highest priority is selected". The
/// far node outranks proximity by setting `ATTR.priority`, which is
/// exactly what `Binding::attributes` alone cannot express.
#[test]
fn attr_priority_beats_proximity() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "far", "priority 10 outranks proximity");
    assert_eq!(value.priority, 10);
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![1])
    );
}

/// The priority is per attribute, not per node: a priority on one
/// attribute must not lift the node's other attributes with it.
#[test]
fn attr_priority_lifts_only_its_own_attribute() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute(
            "near",
            vec![integers("visibility", vec![0]), integers("matte", vec![0])],
        )
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10]),
                integers("matte", vec![1]),
            ],
        )
        .unwrap();

    assert_eq!(
        scene
            .attribute_value("mesh", "visibility")
            .unwrap()
            .unwrap()
            .node,
        "far",
    );
    assert_eq!(
        scene
            .attribute_value("mesh", "matte")
            .unwrap()
            .unwrap()
            .node,
        "near",
        "`matte` has no priority of its own, so proximity decides it",
    );
}

/// ɴsɪ: "If their priority is the same, the more specific attribute
/// (i.e. per ray type) is used."
#[test]
fn a_per_ray_visibility_beats_the_default() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute(
            "near",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.camera", vec![0]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.name, "visibility.camera");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

/// A per-ray query falls back to the default when nothing sets the ray
/// type: ɴsɪ's `visibility` "sets the default visibility for all ray
/// types". `arg.name` is how the caller tells which one answered.
#[test]
fn the_default_visibility_answers_a_per_ray_query() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.shadow").unwrap();
    let value = value.expect("the default covers every ray type");
    assert_eq!(value.name, "visibility");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

/// Specificity only breaks a *tie*: "the attribute with the highest
/// priority is used" comes first, so a prioritised default beats a
/// per-ray value.
#[test]
fn a_prioritised_default_beats_a_per_ray_visibility() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute(
            "near",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![5]),
                integers("visibility.camera", vec![0]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.name, "visibility", "priority 5 beats specificity");
    assert_eq!(value.priority, 5);
}

/// `visibility.set.subsurface` is a *connection* to a `set` node, not a
/// per-ray int. Falling back to `visibility` for it would answer a
/// connection query with a flag.
#[test]
fn visibility_set_subsurface_is_not_a_ray_type() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![1])])
        .unwrap();

    assert!(
        scene
            .attribute_value("mesh", "visibility.set.subsurface")
            .unwrap()
            .is_none(),
        "`set.subsurface` is not one of ɴsɪ's ray types",
    );
}

/// ɴsɪ declares `ATTR.priority` an `int`. Reinterpreting some other
/// layout as one would let a stray float silently reorder the scene.
#[test]
fn a_non_integer_priority_is_ignored() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                OwnedArg {
                    name: "visibility.priority".to_string(),
                    type_tag: Type::F32,
                    array_length: 1,
                    flags: 0,
                    data: OwnedData::F32(vec![10.0]),
                },
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near", "the float priority does not count");
    assert_eq!(value.priority, 0);
}

/// Nothing on the path defines it.
#[test]
fn an_undefined_attribute_resolves_to_none() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();

    assert!(scene.attribute_value("mesh", "matte").unwrap().is_none());
}

/// The walk is the same one `geometry_binding` does, so its failures
/// are this function's failures too.
#[test]
fn attribute_value_propagates_a_detached_path() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    assert!(matches!(
        scene.attribute_value("mesh", "visibility"),
        Err(ResolveError::Detached { .. })
    ));
}

/// The documented assumption, isolated: specificity is compared
/// *before* proximity, so a per-ray value on the far node beats a plain
/// `visibility` on the near one. ɴsɪ gives the specificity rule without
/// saying whether it outranks proximity, so this pins the choice rather
/// than leaving it to whichever candidate happened to be pushed first.
///
/// The same-node case cannot show this: there the push order already
/// puts the specific attribute ahead of the default.
#[test]
fn specificity_is_compared_before_proximity() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![1])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility.camera", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(
        value.node, "far",
        "the per-ray value outranks the nearer default"
    );
    assert_eq!(value.name, "visibility.camera");
}

/// 3Delight ignores an `int64` `ATTR.priority`, so this does too.
///
/// Rendered: `far` sets `visibility 1` with an `int64`
/// `visibility.priority` of 10 and still loses to `near`'s
/// `visibility 0`. An `int64` *is* accepted for the `visibility` value
/// itself, so the rejection is specific to the priority. The crate read
/// it until round 11, and nothing was red.
#[test]
fn an_int64_priority_is_ignored() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                OwnedArg {
                    name: "visibility.priority".to_string(),
                    type_tag: Type::I64,
                    array_length: 1,
                    flags: 0,
                    data: OwnedData::I64(vec![10]),
                },
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near", "an int64 priority does not count");
    assert_eq!(value.priority, 0);
}

/// A node setting only `visibility.priority` is a definition to
/// 3Delight -- of `visibility` at its ɴsɪ default -- and it wins.
/// Rendered (`B`, `C`): the geometry is visible over a farther
/// `visibility 0`, at priority `10` and at `0` alike. This crate does
/// not carry ɴsɪ's defaults, so it names the winner and returns no
/// value rather than the loser's.
#[test]
fn a_priority_without_its_attribute_is_a_definition() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility.priority", vec![10])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`near` defines it at the default");
    assert_eq!(value.node, "near");
    assert_eq!(value.name, "visibility");
    assert_eq!(value.arg, None, "the default is the backend's to supply");
    assert_eq!(value.priority, 10);
}

/// And it ranks on that priority, rather than merely winning where it
/// happens to be nearest: rendered (`E`), a `visibility.priority 10`
/// two levels up leaves the geometry visible even though the node
/// attached to the primitive itself sets `visibility 0`.
///
/// This is the case the crate answered **wrongly** before, not merely
/// incompletely: it returned `near`'s `visibility 0` and a backend
/// would have hidden an object 3Delight draws.
#[test]
fn a_defaulted_definition_outranks_a_nearer_value() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility.priority", vec![10])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`far` defines it at the default");
    assert_eq!(value.node, "far", "priority 10 outranks proximity");
    assert_eq!(value.arg, None);
    assert_eq!(value.priority, 10);
}

/// The converse, so the new candidate cannot simply always win:
/// rendered (`F`), a `visibility 0` at priority `20` two levels up
/// beats a lone `visibility.priority 10` on the primitive's own node,
/// and the geometry stays hidden.
#[test]
fn a_higher_priority_value_beats_a_defaulted_definition() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility.priority", vec![10])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![0]),
                integers("visibility.priority", vec![20]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`far` defines it");
    assert_eq!(value.node, "far");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
    assert_eq!(value.priority, 20);
}

/// A priority is exactly one `int`, in count as well as in type.
/// Rendered (`F1a`, `F1b`): with the priority written `"int" 2
/// [ 10 10 ]`, and again as the identical `"int[2]" 1 [ 10 10 ]`, the
/// geometry stays hidden -- 3Delight ranked nothing on it and the
/// nearer `visibility 0` answered -- while the one-value control
/// (`F1ctl`) shows it. Taking the first of several would rank a node
/// the renderer does not.
#[test]
fn a_multi_valued_priority_is_ignored() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10, 10]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near", "two ints are not a priority");
    assert_eq!(value.priority, 0);
}

/// A lone priority answers a per-ray query through the same
/// specificity fallback a real value gets. Rendered (`H`): `near`
/// carries only `visibility.priority 10`, `far` sets
/// `visibility.camera 0`, and the geometry is visible -- the defaulted
/// `visibility` outranks the more specific value on priority.
#[test]
fn a_lone_priority_answers_a_per_ray_query_through_the_fallback() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility.priority", vec![10])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility.camera", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("the lone priority defines the default");
    assert_eq!(value.node, "near");
    assert_eq!(value.name, "visibility", "the less specific key won");
    assert_eq!(value.arg, None);
    assert_eq!(value.priority, 10);
}

/// A priority 3Delight cannot read is not a definition either.
/// Rendered (`D`): with the lone priority written as an `int64` the
/// geometry is **hidden**, so the farther `visibility 0` answers -- if
/// the node counted at priority `0` it would win on proximity and the
/// geometry would be visible, as in `C`.
#[test]
fn an_unreadable_priority_alone_is_not_a_definition() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute(
            "near",
            vec![OwnedArg {
                name: "visibility.priority".to_string(),
                type_tag: Type::I64,
                array_length: 1,
                flags: 0,
                data: OwnedData::I64(vec![10]),
            }],
        )
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`far` defines it");
    assert_eq!(value.node, "far", "an int64 priority defines nothing");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

/// An `attributes` node with nothing on it is not a definition, which
/// is what makes the rule about the *priority* rather than the node.
/// Rendered (`A`): the geometry stays hidden.
#[test]
fn an_empty_attributes_node_is_not_a_definition() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("far", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`far` defines it");
    assert_eq!(value.node, "far");
}

/// A per-ray priority defines the per-ray attribute, not the default.
/// Rendered (`G`): a lone `visibility.camera.priority` makes the
/// geometry visible over a farther `visibility.camera 0`.
#[test]
fn a_per_ray_priority_alone_defines_the_per_ray_attribute() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute(
            "near",
            vec![integers("visibility.camera.priority", vec![10])],
        )
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility.camera", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("`near` defines it at the default");
    assert_eq!(value.node, "near");
    assert_eq!(value.name, "visibility.camera");
    assert_eq!(value.arg, None);
}

/// A defaulted definition is ranked by specificity like any other, not
/// waved through on proximity. Rendered (`H2`): at equal priority a
/// farther explicit `visibility.camera 0` beats a nearer lone
/// `visibility.priority`, and the geometry stays hidden.
#[test]
fn a_defaulted_definition_is_ranked_by_specificity() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility.priority", vec![0])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility.camera", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility.camera").unwrap();
    let value = value.expect("`far` defines it");
    assert_eq!(value.node, "far", "the per-ray value is more specific");
    assert_eq!(value.name, "visibility.camera");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

// ---------------------------------------------------------------------
// `shaderattributes`: ɴsɪ's other container, with its own rule.
// ---------------------------------------------------------------------

/// `mesh -> xf -> .root` with a `shaderattributes` node on each level.
fn scene_with_shader_attributes() -> Scene {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("near", "attributes").unwrap();
    scene.create("far", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .connect("near", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .connect("far", None, "xf", "shaderattributes")
        .unwrap();
    scene
}

/// ɴsɪ: "Priority is given to nodes attached closest to the geometric
/// primitive, with the highest priority given to attributes set
/// directly on the geometric primitive."
#[test]
fn the_nearest_shader_attribute_wins() {
    let mut scene = scene_with_shader_attributes();
    scene
        .set_attribute("near", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("tint", vec![2])])
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![1])
    );
}

/// An ancestor's shader attributes are inherited when nothing nearer
/// defines them.
#[test]
fn an_ancestor_shader_attribute_is_inherited() {
    let mut scene = scene_with_shader_attributes();
    scene
        .set_attribute("far", vec![integers("tint", vec![2])])
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    assert_eq!(value.expect("inherited").node, "far");
}

/// `ATTR.priority` belongs to the `geometryattributes` rule. ɴsɪ gives
/// this node proximity only, so honouring a priority here would invent
/// a rule the specification does not state.
#[test]
fn a_shader_attribute_ignores_attr_priority() {
    let mut scene = scene_with_shader_attributes();
    scene
        .set_attribute("near", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("tint", vec![2]),
                integers("tint.priority", vec![10]),
            ],
        )
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    let value = value.expect("defined on the path");
    assert_eq!(value.node, "near", "proximity only; no priority here");
    assert_eq!(value.priority, 0);
}

/// The two containers are separate. A `geometryattributes` node must not
/// answer a shader-attribute query, nor the reverse.
#[test]
fn the_two_attribute_containers_do_not_cross() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("geom", "attributes").unwrap();
    scene.create("shade", "attributes").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("geom", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("shade", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .set_attribute("geom", vec![integers("only_geom", vec![1])])
        .unwrap();
    scene
        .set_attribute("shade", vec![integers("only_shade", vec![1])])
        .unwrap();

    assert!(
        scene
            .shader_attribute_value("mesh", "only_geom")
            .unwrap()
            .is_none(),
        "a geometryattributes node is not a shader-attribute source",
    );
    assert!(
        scene
            .attribute_value("mesh", "only_shade")
            .unwrap()
            .is_none(),
        "a shaderattributes node is not a geometry-attribute source",
    );
    assert_eq!(
        scene.shader_attributes("mesh").unwrap(),
        // The geometry leads the list: it is a source in its own right,
        // and the `geometryattributes` node is not in this one at all.
        vec!["mesh".to_string(), "shade".to_string()],
    );
}

/// Nothing on the path provides one.
#[test]
fn an_undefined_shader_attribute_resolves_to_none() {
    let scene = scene_with_shader_attributes();
    assert!(
        scene
            .shader_attribute_value("mesh", "tint")
            .unwrap()
            .is_none()
    );
}

/// ɴsɪ: "with the highest priority given to attributes set directly on
/// the geometric primitive."
///
/// Rendered, `tint` on the mesh beats `tint` on an `attributes` node
/// attached to that same mesh -- in both directions, so the mesh is not
/// winning for having the larger value -- and beats one carrying a
/// `tint.priority` too. This crate returned the container's value, a
/// wrong answer rather than a missing one, until round 12.
#[test]
fn the_geometrys_own_shader_attribute_outranks_every_container() {
    let mut scene = scene_with_shader_attributes();
    scene
        .set_attribute("mesh", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute(
            "near",
            vec![
                integers("tint", vec![2]),
                integers("tint.priority", vec![10]),
            ],
        )
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    let value = value.expect("set on the primitive");
    assert_eq!(value.node, "mesh");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![1])
    );
}

/// The primitive is a source with no container present at all.
#[test]
fn a_shader_attribute_on_the_primitive_needs_no_container() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .set_attribute("mesh", vec![integers("tint", vec![7])])
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    assert_eq!(value.expect("set on the primitive").node, "mesh");
}

/// `shader_attributes` is a precedence-ordered walk: the geometry
/// first, then nearest, then the connection order within one level, and
/// a node on `.root` is included. Each of those was unpinned -- a
/// reversed list, a reversed within-level order and dropping `.root`
/// all left the suite green.
#[test]
fn shader_attribute_sources_are_in_precedence_order() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    for handle in ["near", "near2", "mid", "root_attrs"] {
        scene.create(handle, "attributes").unwrap();
    }
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    // Two on the geometry, in connection order.
    scene
        .connect("near", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .connect("near2", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .connect("mid", None, "xf", "shaderattributes")
        .unwrap();
    scene
        .connect("root_attrs", None, ".root", "shaderattributes")
        .unwrap();

    assert_eq!(
        scene.shader_attributes("mesh").unwrap(),
        vec![
            "mesh".to_string(),
            "near".to_string(),
            "near2".to_string(),
            "mid".to_string(),
            "root_attrs".to_string(),
        ],
    );
}

/// Within one level the first connection wins, and a `.root` node is
/// reachable when nothing nearer defines the attribute.
#[test]
fn the_first_connected_shader_attribute_wins_at_one_level() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("first", "attributes").unwrap();
    scene.create("second", "attributes").unwrap();
    scene.create("root_attrs", "attributes").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("first", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .connect("second", None, "mesh", "shaderattributes")
        .unwrap();
    scene
        .connect("root_attrs", None, ".root", "shaderattributes")
        .unwrap();
    scene
        .set_attribute("first", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute("second", vec![integers("tint", vec![2])])
        .unwrap();
    scene
        .set_attribute("root_attrs", vec![integers("other", vec![9])])
        .unwrap();

    assert_eq!(
        scene
            .shader_attribute_value("mesh", "tint")
            .unwrap()
            .unwrap()
            .node,
        "first",
    );
    assert_eq!(
        scene
            .shader_attribute_value("mesh", "other")
            .unwrap()
            .unwrap()
            .node,
        "root_attrs",
        "a node on `.root` is still a source",
    );
}

// ---------------------------------------------------------------------
// Set membership as an attribute source. Every expectation below was
// rendered in 3Delight, each direction mirrored.
// ---------------------------------------------------------------------

/// `mesh -> xf -> .root`, with `mesh` also a member of set `s`.
fn scene_with_a_set() -> Scene {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("s", "set").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("mesh", None, "s", "members").unwrap();
    scene
}

/// An `attributes` node on a set the geometry belongs to binds. ɴsɪ's
/// gathering text names only primitives and transforms; 3Delight
/// honours the set, and this crate silently missed it.
#[test]
fn an_attributes_node_on_a_set_binds() {
    let mut scene = scene_with_a_set();
    scene.create("sa", "attributes").unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["sa".to_string()]);
}

/// A container on the geometry outranks one on its set.
#[test]
fn the_geometrys_own_container_outranks_its_set() {
    let mut scene = scene_with_a_set();
    scene.create("near", "attributes").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene
        .connect("near", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute("sa", vec![integers("visibility", vec![1])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    assert_eq!(value.expect("defined").node, "near");
}

/// A container on the set outranks one on the transform above it.
#[test]
fn a_set_outranks_the_transform_above_it() {
    let mut scene = scene_with_a_set();
    scene.create("far", "attributes").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene
        .connect("far", None, "xf", "geometryattributes")
        .unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute("sa", vec![integers("visibility", vec![1])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined");
    assert_eq!(value.node, "sa");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![1])
    );
}

/// With two memberships the first connection wins.
#[test]
fn the_first_set_membership_wins() {
    let mut scene = scene_with_a_set();
    scene.create("s2", "set").unwrap();
    scene.connect("mesh", None, "s2", "members").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene.create("sa2", "attributes").unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .connect("sa2", None, "s2", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("sa", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute("sa2", vec![integers("visibility", vec![1])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    assert_eq!(value.expect("defined").node, "sa");
}

/// A set nested inside another set contributes nothing: only direct
/// membership counts. Walking transitively would apply attributes
/// 3Delight does not.
#[test]
fn a_nested_sets_attributes_are_not_inherited() {
    let mut scene = scene_with_a_set();
    scene.create("s2", "set").unwrap();
    scene.connect("s", None, "s2", "members").unwrap();
    scene.create("sa2", "attributes").unwrap();
    scene
        .connect("sa2", None, "s2", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("sa2", vec![integers("visibility", vec![0])])
        .unwrap();

    assert!(
        scene
            .attribute_value("mesh", "visibility")
            .unwrap()
            .is_none(),
        "the outer set is not a source for this geometry",
    );
}

/// `ATTR.priority` still outranks the whole ordering, from a set too.
#[test]
fn an_attr_priority_on_a_set_beats_the_geometrys_own() {
    let mut scene = scene_with_a_set();
    scene.create("near", "attributes").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene
        .connect("near", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "sa",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined");
    assert_eq!(value.node, "sa");
    assert_eq!(value.priority, 10);
}

/// The set path serves shader attributes too, at the same rank.
#[test]
fn a_set_provides_shader_attributes_below_the_geometry() {
    let mut scene = scene_with_a_set();
    scene.create("sa", "attributes").unwrap();
    scene.create("far", "attributes").unwrap();
    scene.connect("sa", None, "s", "shaderattributes").unwrap();
    scene
        .connect("far", None, "xf", "shaderattributes")
        .unwrap();
    scene
        .set_attribute("sa", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("tint", vec![2])])
        .unwrap();

    let value = scene.shader_attribute_value("mesh", "tint").unwrap();
    assert_eq!(value.expect("defined").node, "sa");
    assert_eq!(
        scene.shader_attributes("mesh").unwrap(),
        vec!["mesh".to_string(), "sa".to_string(), "far".to_string()],
    );
}

// ---------------------------------------------------------------------
// Deforming geometry: sample times of an arbitrary attribute.
// ---------------------------------------------------------------------

fn points(values: Vec<f32>) -> OwnedArg {
    OwnedArg {
        name: "P".to_string(),
        type_tag: Type::Point,
        array_length: 1,
        flags: 0,
        data: OwnedData::F32(values),
    }
}

/// The gap this closes: a mesh whose `P` is sampled under a *static*
/// transform has no motion times, so `motion_times` answers "static"
/// for something that plainly deforms.
#[test]
fn a_deforming_mesh_reports_its_sample_times() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .set_attribute_at_time("mesh", 0.0, vec![points(vec![0.0, 0.0, 0.0])])
        .unwrap();
    scene
        .set_attribute_at_time("mesh", 1.0, vec![points(vec![1.0, 0.0, 0.0])])
        .unwrap();

    assert!(
        scene.motion_times("mesh").unwrap().is_empty(),
        "the transform really is static",
    );
    assert_eq!(scene.attribute_times("mesh", "P").unwrap(), vec![0.0, 1.0]);

    let samples = scene.attribute_samples("mesh", "P").unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].0, 0.0);
    assert_eq!(samples[1].1.data, OwnedData::F32(vec![1.0, 0.0, 0.0]));
}

/// The times are per attribute, not per node.
#[test]
fn attribute_times_are_per_attribute() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene
        .set_attribute_at_time("mesh", 0.0, vec![points(vec![0.0])])
        .unwrap();
    scene
        .set_attribute_at_time("mesh", 1.0, vec![integers("N", vec![1])])
        .unwrap();

    assert_eq!(scene.attribute_times("mesh", "P").unwrap(), vec![0.0]);
    assert_eq!(scene.attribute_times("mesh", "N").unwrap(), vec![1.0]);
    assert!(scene.attribute_times("mesh", "absent").unwrap().is_empty());
}

/// A static value is not a sample. Reporting one would invent a time
/// the caller never set.
#[test]
fn a_static_attribute_has_no_sample_times() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene
        .set_attribute("mesh", vec![points(vec![0.0, 0.0, 0.0])])
        .unwrap();

    assert!(scene.attribute_times("mesh", "P").unwrap().is_empty());
    assert!(scene.attribute_samples("mesh", "P").unwrap().is_empty());
}

/// An unknown handle is a caller mistake, not a scene fact. Answering
/// "not sampled" would read as the latter.
#[test]
fn attribute_times_refuses_an_unknown_handle() {
    let scene = Scene::default();
    assert!(matches!(
        scene.attribute_times("nope", "P"),
        Err(ResolveError::UnknownHandle { .. })
    ));
    assert!(matches!(
        scene.attribute_samples("nope", "P"),
        Err(ResolveError::UnknownHandle { .. })
    ));
}

/// Samples come back in ascending time however they were recorded.
#[test]
fn attribute_samples_are_time_ordered() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    for time in [2.0, 0.0, 1.0] {
        scene
            .set_attribute_at_time(
                "mesh",
                time,
                vec![points(vec![time as f32])],
            )
            .unwrap();
    }

    assert_eq!(
        scene.attribute_times("mesh", "P").unwrap(),
        vec![0.0, 1.0, 2.0],
    );
    // This test was named for `attribute_samples` and never called it:
    // reversing that function alone left it green.
    let samples = scene.attribute_samples("mesh", "P").unwrap();
    let times: Vec<f64> = samples.iter().map(|(time, _)| *time).collect();
    assert_eq!(times, vec![0.0, 1.0, 2.0]);
    assert_eq!(samples[0].1.data, OwnedData::F32(vec![0.0]));
}

/// A set whose member is a *transform* on the chain is a source too.
///
/// Rendered: `mesh -> xf -> .root` with `xf` a member of `s`, and an
/// `attributes` node on `s` setting `visibility 0`, renders invisible.
/// The first version of the set walk looked only at sets of the
/// geometry and returned nothing here.
#[test]
fn a_set_on_a_transform_in_the_chain_is_gathered() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("s", "set").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("xf", None, "s", "members").unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("sa", vec![integers("visibility", vec![0])])
        .unwrap();

    let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
    assert_eq!(binding.attributes, vec!["sa".to_string()]);
    let value = scene.attribute_value("mesh", "visibility").unwrap();
    assert_eq!(value.expect("defined").node, "sa");
}

/// A set of the geometry outranks a set of its transform.
#[test]
fn a_set_of_the_geometry_outranks_a_set_of_its_transform() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("sm", "set").unwrap();
    scene.create("sx", "set").unwrap();
    scene.create("am", "attributes").unwrap();
    scene.create("ax", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("mesh", None, "sm", "members").unwrap();
    scene.connect("xf", None, "sx", "members").unwrap();
    scene
        .connect("am", None, "sm", "geometryattributes")
        .unwrap();
    scene
        .connect("ax", None, "sx", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("am", vec![integers("visibility", vec![1])])
        .unwrap();
    scene
        .set_attribute("ax", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined");
    assert_eq!(value.node, "am");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![1])
    );
}

/// A transform's own container outranks a set that transform belongs
/// to, the same way the geometry's does.
#[test]
fn a_transforms_own_container_outranks_its_set() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("s", "set").unwrap();
    scene.create("own", "attributes").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("xf", None, "s", "members").unwrap();
    scene
        .connect("own", None, "xf", "geometryattributes")
        .unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();

    assert_eq!(
        scene
            .geometry_binding("mesh")
            .unwrap()
            .expect("bound")
            .attributes,
        vec!["own".to_string(), "sa".to_string()],
    );
}

/// A set holding two nodes of the chain is one source, at its nearest
/// occurrence -- not one per member, which would list it twice and skew
/// every later rank.
#[test]
fn a_set_holding_two_chain_nodes_is_one_source() {
    let mut scene = Scene::default();
    scene.create("mesh", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("s", "set").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene.create("xa", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene.connect("mesh", None, "s", "members").unwrap();
    scene.connect("xf", None, "s", "members").unwrap();
    scene
        .connect("sa", None, "s", "geometryattributes")
        .unwrap();
    scene
        .connect("xa", None, "xf", "geometryattributes")
        .unwrap();

    assert_eq!(
        scene
            .geometry_binding("mesh")
            .unwrap()
            .expect("bound")
            .attributes,
        // `sa` once, and ahead of the transform's own node because the
        // set is reached at the mesh.
        vec!["sa".to_string(), "xa".to_string()],
    );
}

/// `.root` and `.global` exist whether or not they were created, so
/// they have no samples rather than being unknown. The answer used to
/// flip between `Err` and `Ok` depending on whether some unrelated
/// attribute had been set on `.root` first.
#[test]
fn a_reserved_handle_has_no_samples_rather_than_being_unknown() {
    let mut scene = Scene::default();
    assert_eq!(scene.attribute_times(".root", "P").unwrap(), Vec::new());
    assert!(scene.attribute_samples(".global", "P").unwrap().is_empty());

    // Materialising `.root` must not change the answer's kind.
    scene
        .set_attribute(".root", vec![integers("unrelated", vec![1])])
        .unwrap();
    assert_eq!(scene.attribute_times(".root", "P").unwrap(), Vec::new());
}

// ---------------------------------------------------------------------
// Interpolating a transform between motion samples.
// ---------------------------------------------------------------------

/// Halfway between two translations is the midpoint.
#[test]
fn a_transform_interpolates_between_its_samples() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(10.0, 0.0, 0.0)])
        .unwrap();

    let half = scene.world_transform_interpolated_at("mesh", 0.5).unwrap();
    assert_eq!(half[12], 5.0);

    let quarter = scene.world_transform_interpolated_at("mesh", 0.25).unwrap();
    assert_eq!(quarter[12], 2.5);
}

/// An exact sample is that sample, not a recomputation of it.
#[test]
fn interpolating_at_a_sample_returns_the_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(10.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.world_transform_interpolated_at("mesh", 1.0).unwrap(),
        scene.world_transform_at("mesh", 1.0).unwrap(),
    );
}

/// Outside the sampled range the end sample is **held**, because that
/// is what 3Delight does.
///
/// This test asserted the opposite. Rendered -- samples at t=0 and t=1
/// with the shutter open over [-1, 2] -- there is zero alpha beyond the
/// two sampled positions, where an extrapolating renderer would sweep
/// half again as far each way, and a peak at each end 2.7x the swept
/// middle where a third of the shutter is held. Refusing here would
/// have failed a backend on a scene the renderer renders.
///
/// A NaN still brackets nothing and is still refused.
#[test]
fn interpolating_outside_the_sampled_range_holds_the_end_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(10.0, 0.0, 0.0)])
        .unwrap();

    // Before the first sample, and after the last: held.
    assert_eq!(
        scene.world_transform_interpolated_at("mesh", -0.5).unwrap()[12],
        0.0,
    );
    assert_eq!(
        scene.world_transform_interpolated_at("mesh", 1.5).unwrap()[12],
        10.0,
    );

    // `-0.0` is the sample at `0.0`, as it is to the recorder and the
    // renderer, not a time before it. Held by the leading clamp here,
    // because this fixture starts at `0`. An *interior* `-0.0` needs
    // the normalising fold, which
    // `negative_zero_finds_an_interior_sample` covers -- a note here
    // once claimed that fold "guarded nothing", on the strength of this
    // fixture alone.
    assert_eq!(
        scene.world_transform_interpolated_at("mesh", -0.0).unwrap()[12],
        0.0,
    );

    // A NaN brackets nothing and names no sample.
    assert!(matches!(
        scene.world_transform_interpolated_at("mesh", f64::NAN),
        Err(ResolveError::MissingSampleAtTime { .. })
    ));
}

/// Each node is interpolated from its **own** samples and the results
/// composed. That is not the same as interpolating the composed world
/// matrices, and it is the accurate model: the two transforms here are
/// animated independently.
#[test]
fn each_node_interpolates_from_its_own_samples() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene.create("inner", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("mesh", None, "inner", "objects").unwrap();

    // `outer` scales 1 -> 3; `inner` translates 0 -> 4 along x.
    scene
        .set_attribute_at_time("outer", 0.0, vec![scale(1.0)])
        .unwrap();
    scene
        .set_attribute_at_time("outer", 1.0, vec![scale(3.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 1.0, vec![translate(4.0, 0.0, 0.0)])
        .unwrap();

    // At t=0.5: scale 2, translate 2 -> the composed x offset is 4.
    // Interpolating the *composed* matrices instead would give
    // (0 + 3*4)/2 = 6, so this discriminates the two models.
    let half = scene.world_transform_interpolated_at("mesh", 0.5).unwrap();
    assert_eq!(half[12], 4.0);
}

/// A chain mixing static and sampled nodes composes both.
///
/// A static node is constant, so it contributes its matrix at every
/// time. Dropping static nodes from the interpolated walk left all 180
/// tests green, so nothing covered this.
#[test]
fn interpolation_keeps_the_static_nodes_of_a_chain() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene.create("mid", "transform").unwrap();
    scene.create("inner", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();
    scene.connect("mid", None, "outer", "objects").unwrap();
    scene.connect("inner", None, "mid", "objects").unwrap();
    scene.connect("mesh", None, "inner", "objects").unwrap();

    // Sampled, static, sampled.
    scene
        .set_attribute_at_time("outer", 0.0, vec![scale(1.0)])
        .unwrap();
    scene
        .set_attribute_at_time("outer", 1.0, vec![scale(3.0)])
        .unwrap();
    scene
        .set_attribute("mid", vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 1.0, vec![translate(4.0, 0.0, 0.0)])
        .unwrap();

    // t=0.5: scale 2, then the static +1, then the interpolated +2.
    // (1 + 2) * 2 = 6. Dropping the static node gives 4.
    let half = scene.world_transform_interpolated_at("mesh", 0.5).unwrap();
    assert_eq!(half[12], 6.0);

    // And at a shared sample it must agree with the exact accessor.
    for time in [0.0, 1.0] {
        assert_eq!(
            scene.world_transform_interpolated_at("mesh", time).unwrap(),
            scene.world_transform_at("mesh", time).unwrap(),
            "the two accessors disagree at sample {time}",
        );
    }
}

/// A node with a single sample is constant at it, rather than having no
/// bracketing pair. `world_transform_at` answers at that sample, so
/// refusing here would have made the two accessors disagree.
#[test]
fn a_single_sample_node_is_constant() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("mesh", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.25, vec![translate(7.0, 0.0, 0.0)])
        .unwrap();

    for time in [0.0, 0.25, 9.0] {
        assert_eq!(
            scene.world_transform_interpolated_at("mesh", time).unwrap()[12],
            7.0,
            "one sample applies at every time, including {time}",
        );
    }
}

// ---------------------------------------------------------------------
// Lightweight instancing: one placement per path.
// ---------------------------------------------------------------------

/// `q` under two transforms is drawn twice, once per path, each with
/// its own transform. Rendered: a quad under transforms translated -2
/// and +2 appears at both positions, where one parent gives one.
#[test]
fn a_two_parent_geometry_has_two_placements() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene
        .set_attribute("xfA", vec![translate(-2.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute("xfB", vec![translate(2.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();

    let placements = scene.placements("q").unwrap();
    assert_eq!(placements.len(), 2);
    assert_eq!(placements[0].transform[12], -2.0);
    assert_eq!(placements[1].transform[12], 2.0);
    // In connection order, and the path names which is which.
    assert_eq!(placements[0].path, vec!["q", "xfA", ".root"]);
    assert_eq!(placements[1].path, vec!["q", "xfB", ".root"]);

    // The single-answer accessors still refuse, because there is none.
    assert!(matches!(
        scene.world_transform("q"),
        Err(ResolveError::MultipleParents { .. })
    ));
}

/// Each path carries its **own** attributes. Rendered: `visibility 1`
/// on one parent and `visibility 0` on the other draws one copy, not
/// two and not none -- so a per-path transform without a per-path
/// binding would hand a backend the wrong material or the wrong
/// visibility for one of the two.
#[test]
fn each_placement_binds_along_its_own_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.create("visA", "attributes").unwrap();
    scene.create("visB", "attributes").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene
        .connect("visA", None, "xfA", "geometryattributes")
        .unwrap();
    scene
        .connect("visB", None, "xfB", "geometryattributes")
        .unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();

    let placements = scene.placements("q").unwrap();
    assert_eq!(
        placements[0].binding.as_ref().unwrap().attributes,
        vec!["visA".to_string()],
    );
    assert_eq!(
        placements[1].binding.as_ref().unwrap().attributes,
        vec!["visB".to_string()],
    );
}

/// A singly-placed geometry yields one placement that agrees with the
/// single-answer accessors, so a backend can use this alone.
#[test]
fn a_single_placement_agrees_with_the_single_answer_accessors() {
    let scene = scene_with_material();

    let placements = scene.placements("mesh").unwrap();
    assert_eq!(placements.len(), 1);
    assert_eq!(
        placements[0].transform,
        scene.world_transform("mesh").unwrap()
    );
    assert_eq!(
        placements[0].binding,
        scene.geometry_binding("mesh").unwrap(),
    );
}

/// A geometry reaching no root has no placements, and says so rather
/// than returning an empty list a caller would read as "not drawn but
/// fine".
#[test]
fn a_detached_geometry_has_no_placements() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    assert!(matches!(
        scene.placements("q"),
        Err(ResolveError::Detached { .. })
    ));
}

/// A cycle on a path is refused rather than walked forever.
#[test]
fn a_cyclic_placement_path_is_refused() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, "b", "objects").unwrap();
    scene.connect("b", None, "a", "objects").unwrap();

    assert!(matches!(
        scene.placements("a"),
        Err(ResolveError::Cycle { .. })
    ));
}

/// A prototype reached only through an `instances` node has no *direct*
/// placement, and is not detached: 3Delight draws it, at the matrices
/// the instancer carries.
///
/// This returned `Detached` -- "not in the scene" about something that
/// renders -- contradicting both `placements`' own documentation and
/// `an_instancing_prototype_is_not_detached`. Rendered: a quad reached
/// only through an instancer translated `+15` appears at that offset.
#[test]
fn an_instancer_only_prototype_has_no_direct_placements() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene.connect("q", None, "inst", "sourcemodels").unwrap();

    assert_eq!(
        scene.placements("q").unwrap(),
        Vec::new(),
        "no direct placements; the instancer carries them",
    );

    // A node reaching nothing at all is still an error, so the empty
    // list means "ask the instancer", not "not drawn".
    scene.create("orphan", "mesh").unwrap();
    assert!(matches!(
        scene.placements("orphan"),
        Err(ResolveError::Detached { .. })
    ));
}

/// A geometry both directly placed *and* instanced keeps its direct
/// placement. 3Delight draws both copies.
#[test]
fn a_directly_placed_prototype_keeps_its_placement() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "inst", "sourcemodels").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .set_attribute("xf", vec![translate(3.0, 0.0, 0.0)])
        .unwrap();

    let placements = scene.placements("q").unwrap();
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].path, vec!["q", "xf", ".root"]);
    assert_eq!(placements[0].transform[12], 3.0);
}

/// Composition order, with matrices that do **not** commute.
///
/// Every earlier placement fixture used pure translations, so reversing
/// `mul` in the composition changed nothing and the mutation survived.
#[test]
fn placement_composition_order_matches_world_transform() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("outer", "transform").unwrap();
    scene.create("inner", "transform").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("q", None, "inner", "objects").unwrap();
    scene.set_attribute("outer", vec![scale(2.0)]).unwrap();
    scene
        .set_attribute("inner", vec![translate(5.0, 0.0, 0.0)])
        .unwrap();

    // scale-then-translate gives 10; the reverse order gives 5.
    let placements = scene.placements("q").unwrap();
    assert_eq!(placements[0].transform[12], 10.0);
    assert_eq!(
        placements[0].transform,
        scene.world_transform("q").unwrap(),
        "one composition, so the two cannot disagree",
    );
}

/// A placement includes the node's own matrix, as `world_transform`
/// does. Dropping the first entry of the path went unnoticed because
/// every fixture asked about a geometry with no matrix of its own.
#[test]
fn a_placement_includes_the_nodes_own_matrix() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("parent", "transform").unwrap();
    scene.connect("parent", None, ".root", "objects").unwrap();
    scene.connect("xf", None, "parent", "objects").unwrap();
    scene
        .set_attribute("xf", vec![translate(4.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute("parent", vec![translate(1.0, 0.0, 0.0)])
        .unwrap();

    let placements = scene.placements("xf").unwrap();
    assert_eq!(placements[0].transform[12], 5.0);
    assert_eq!(
        placements[0].transform,
        scene.world_transform("xf").unwrap(),
    );
}

/// A motion-sampled node on the path is refused, not silently treated
/// as identity. `local_transform` reads only the static attributes, so
/// deleting the check yielded identity for a moving parent.
#[test]
fn a_sampled_transform_refuses_a_placement() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(9.0, 0.0, 0.0)])
        .unwrap();

    assert!(matches!(
        scene.placements("q"),
        Err(ResolveError::MotionSampledTransform { .. })
    ));
}

/// The walk does not branch through an `instances` connection: that is
/// not a path, and walking it would compose the instancer's identity in
/// place of the per-instance matrices.
#[test]
fn the_placement_walk_does_not_follow_an_instancer() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("inst", None, "xf", "objects").unwrap();
    scene.connect("q", None, "inst", "sourcemodels").unwrap();
    scene
        .set_attribute("xf", vec![translate(7.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.placements("q").unwrap(),
        Vec::new(),
        "reached only through the instancer, so no direct placement",
    );
}

/// A deep chain must not abort the process.
///
/// The walk was recursive and, measured, overflowed the stack at about
/// 10 000 nodes on a spawned thread -- which is what a test harness and
/// most backends use. A stack overflow kills the process rather than
/// returning an error a caller can handle, so depth is a correctness
/// property here, not a performance one. 12 000 is past the recursive
/// limit and cheap to build.
#[test]
fn a_deep_chain_does_not_overflow_the_stack() {
    const DEPTH: usize = 12_000;

    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    for level in 0..DEPTH {
        scene.create(&format!("xf{level}"), "transform").unwrap();
    }
    scene.connect("xf0", None, ".root", "objects").unwrap();
    for level in 1..DEPTH {
        scene
            .connect(
                &format!("xf{level}"),
                None,
                &format!("xf{}", level - 1),
                "objects",
            )
            .unwrap();
    }
    scene
        .connect("q", None, &format!("xf{}", DEPTH - 1), "objects")
        .unwrap();

    let placements = scene.placements("q").unwrap();
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].path.len(), DEPTH + 2);
}

// ---------------------------------------------------------------------
// A moving instanced geometry, which no accessor could answer.
// ---------------------------------------------------------------------

/// A geometry under two *moving* parents.
///
/// `placements` refuses a sampled node and
/// `world_transform_interpolated_at` refuses a multi-parent one, so
/// this -- a crowd, foliage, a particle instance -- had no answer from
/// either.
#[test]
fn a_moving_instanced_geometry_resolves_per_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();
    scene
        .set_attribute_at_time("xfA", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xfA", 1.0, vec![translate(10.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xfB", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xfB", 1.0, vec![translate(-4.0, 0.0, 0.0)])
        .unwrap();

    // Neither of the older accessors can answer this.
    assert!(matches!(
        scene.placements("q"),
        Err(ResolveError::MotionSampledTransform { .. })
    ));
    assert!(matches!(
        scene.world_transform_interpolated_at("q", 0.5),
        Err(ResolveError::MultipleParents { .. })
    ));

    let half = scene.placements_at("q", 0.5).unwrap();
    assert_eq!(half.len(), 2);
    assert_eq!(half[0].transform[12], 5.0);
    assert_eq!(half[1].transform[12], -2.0);

    // And the end sample is held, as the interpolating accessor does.
    let after = scene.placements_at("q", 9.0).unwrap();
    assert_eq!(after[0].transform[12], 10.0);
}

/// A static scene answers the same from both, so a backend can use
/// `placements_at` alone.
#[test]
fn placements_at_agrees_with_placements_on_a_static_scene() {
    let scene = scene_with_material();
    assert_eq!(
        scene.placements_at("mesh", 0.25).unwrap(),
        scene.placements("mesh").unwrap(),
    );
}

/// ɴsɪ's attribute rules apply *along a placement's path*.
///
/// `attribute_value` takes a geometry, so it refuses a multi-parent
/// node -- meaning the `ATTR.priority` and `visibility.<ray>` rules
/// could not be applied to an instanced object at all.
#[test]
fn attribute_rules_apply_along_a_placement_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.create("attrA", "attributes").unwrap();
    scene.create("attrB", "attributes").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();
    scene
        .connect("attrA", None, "xfA", "geometryattributes")
        .unwrap();
    scene
        .connect("attrB", None, "xfB", "geometryattributes")
        .unwrap();
    // A path where the per-ray value must beat the default.
    scene
        .set_attribute(
            "attrA",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.camera", vec![0]),
            ],
        )
        .unwrap();
    scene
        .set_attribute("attrB", vec![integers("visibility", vec![1])])
        .unwrap();

    assert!(matches!(
        scene.attribute_value("q", "visibility.camera"),
        Err(ResolveError::MultipleParents { .. })
    ));

    let placements = scene.placements("q").unwrap();
    let a = scene
        .attribute_value_along(&placements[0].path, "visibility.camera")
        .expect("defined");
    assert_eq!(a.node, "attrA");
    assert_eq!(a.name, "visibility.camera", "specificity still applies");

    let b = scene
        .attribute_value_along(&placements[1].path, "visibility.camera")
        .expect("defined");
    assert_eq!(b.node, "attrB");
    assert_eq!(b.name, "visibility", "falls back to the default");
}

/// The same for `shaderattributes`, including the primitive's own
/// attributes outranking every container.
#[test]
fn shader_attributes_resolve_along_a_placement_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.create("saA", "attributes").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();
    scene
        .connect("saA", None, "xfA", "shaderattributes")
        .unwrap();
    scene
        .set_attribute("saA", vec![integers("tint", vec![3])])
        .unwrap();

    let placements = scene.placements("q").unwrap();
    assert_eq!(
        scene
            .shader_attribute_value_along(&placements[0].path, "tint")
            .expect("on the A path")
            .node,
        "saA",
    );
    assert!(
        scene
            .shader_attribute_value_along(&placements[1].path, "tint")
            .is_none(),
        "nothing on the B path provides it",
    );

    // The primitive's own attribute still outranks the container.
    scene
        .set_attribute("q", vec![integers("tint", vec![9])])
        .unwrap();
    let placements = scene.placements("q").unwrap();
    assert_eq!(
        scene
            .shader_attribute_value_along(&placements[0].path, "tint")
            .expect("on the primitive")
            .node,
        "q",
    );
}

/// The interpolated composition, with matrices that do **not** commute.
///
/// `placements_at` held a verbatim copy of `world_transform_interpolated_at`'s
/// fold -- the drift the sharing was introduced to prevent -- and
/// nothing constrained it: reversing the multiplication and reversing
/// the path both left the suite green, because every fixture reaching
/// it used translations.
#[test]
fn interpolated_composition_order_is_pinned() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("outer", "transform").unwrap();
    scene.create("inner", "transform").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("q", None, "inner", "objects").unwrap();
    scene.set_attribute("outer", vec![scale(2.0)]).unwrap();
    scene
        .set_attribute_at_time("inner", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("inner", 1.0, vec![translate(6.0, 0.0, 0.0)])
        .unwrap();

    // At t=0.5 the inner translate is 3, scaled by 2 => 6.
    // The reversed order gives 3, and a reversed path gives 3 too.
    let half = scene.placements_at("q", 0.5).unwrap();
    assert_eq!(half[0].transform[12], 6.0);
    assert_eq!(half[0].transform[0], 2.0);
    assert_eq!(
        half[0].transform,
        scene.world_transform_interpolated_at("q", 0.5).unwrap(),
        "one fold, so the two cannot disagree",
    );
}

/// A *moving instancer* is not an empty scene.
///
/// `instance_transforms` read only the static attributes, so an
/// instancer whose `transformationmatrices` are sampled -- how a crowd
/// or a particle system moves -- came back as an empty list,
/// indistinguishable from "no instances", while 3Delight renders the
/// instances. It refuses now, and `instance_transforms_at` answers.
#[test]
fn a_moving_instancer_is_refused_not_reported_empty() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![doubles("transformationmatrices", instance_matrix(0.0))],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![doubles("transformationmatrices", instance_matrix(8.0))],
        )
        .unwrap();

    assert!(
        matches!(
            scene.instance_transforms("inst"),
            Err(ResolveError::MotionSampledTransform { .. })
        ),
        "an empty list would read as `no instances`",
    );

    let half = scene.instance_transforms_at("inst", 0.5).unwrap();
    assert_eq!(half.len(), 1);
    assert_eq!(half[0].transform[12], 4.0);

    // Held outside the sampled range, as transforms are.
    let after = scene.instance_transforms_at("inst", 5.0).unwrap();
    assert_eq!(after[0].transform[12], 8.0);
}

/// A static instancer answers the same either way, so a backend can
/// use the time-taking form alone.
#[test]
fn instance_transforms_at_agrees_on_a_static_instancer() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene
        .set_attribute(
            "inst",
            vec![doubles("transformationmatrices", instance_matrix(2.0))],
        )
        .unwrap();

    assert_eq!(
        scene.instance_transforms_at("inst", 0.75).unwrap(),
        scene.instance_transforms("inst").unwrap(),
    );
}

/// `ATTR.priority` applies along a placement path too. The first
/// version of the along-path test covered specificity and the fallback
/// but not priority, which was carried only by the shared body.
#[test]
fn attr_priority_applies_along_a_placement_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("near", "attributes").unwrap();
    scene.create("far", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .connect("near", None, "q", "geometryattributes")
        .unwrap();
    scene
        .connect("far", None, "xf", "geometryattributes")
        .unwrap();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute(
            "far",
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10]),
            ],
        )
        .unwrap();

    let placements = scene.placements("q").unwrap();
    let value = scene
        .attribute_value_along(&placements[0].path, "visibility")
        .expect("defined");
    assert_eq!(value.node, "far", "priority outranks proximity here too");
    assert_eq!(value.priority, 10);
}

/// Nearest wins along a placement path for shader attributes as well.
/// The earlier test had one container per path, so reversing the walk
/// changed nothing.
#[test]
fn the_nearest_shader_attribute_wins_along_a_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.create("near", "attributes").unwrap();
    scene.create("far", "attributes").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .connect("near", None, "q", "shaderattributes")
        .unwrap();
    scene
        .connect("far", None, "xf", "shaderattributes")
        .unwrap();
    scene
        .set_attribute("near", vec![integers("tint", vec![1])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("tint", vec![2])])
        .unwrap();

    let placements = scene.placements("q").unwrap();
    let value = scene
        .shader_attribute_value_along(&placements[0].path, "tint")
        .expect("defined");
    assert_eq!(value.node, "near");
}

/// Asking at an *interior* sample time must answer, not error.
///
/// The instancer's sampling logic was a copy of the transform's with
/// the exact-hit branch dropped, so a query at shutter centre -- the
/// single most ordinary one -- errored on a scene 3Delight renders.
/// Both now read one `locate_sample`.
#[test]
fn an_interior_sample_time_is_answered_for_instances_and_transforms() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.create("xf", "transform").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    for (time, x) in [(0.0, 0.0), (0.5, 4.0), (1.0, 10.0)] {
        scene
            .set_attribute_at_time(
                "inst",
                time,
                vec![doubles("transformationmatrices", instance_matrix(x))],
            )
            .unwrap();
        scene
            .set_attribute_at_time("xf", time, vec![translate(x, 0.0, 0.0)])
            .unwrap();
    }

    // The interior sample, exactly.
    assert_eq!(
        scene.instance_transforms_at("inst", 0.5).unwrap()[0].transform[12],
        4.0,
    );
    assert_eq!(
        scene.world_transform_interpolated_at("xf", 0.5).unwrap()[12],
        4.0,
    );

    // And between two of them, which must not be the same answer.
    assert_eq!(
        scene.instance_transforms_at("inst", 0.25).unwrap()[0].transform[12],
        2.0,
    );
}

/// Sampled `disabledinstances` and `modelindices` are honoured, and
/// the **last** one defined applies for the whole shutter.
///
/// 3Delight does not sample these: `SetAttributeAtTime` on them behaves
/// like an overwriting `SetAttribute`. Rendered with two instances --
/// `disabledinstances [1]` at `t=0` then `[0]` at `t=1` -- it draws
/// instance **1**, the `t=1` value applying throughout; mirroring the
/// values mirrors the result, and moving the shutter changes nothing.
///
/// This crate first held the *earlier* sample, a step, and so returned
/// the discarded value in every case that discriminates. Reading only
/// the static attributes, before that, reported every instance enabled.
#[test]
fn sampled_instance_indices_take_their_last_value() {
    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();

    let build = |disabled_at: [(f64, i32); 2]| {
        let mut scene = Scene::default();
        scene.create("inst", "instances").unwrap();
        scene.create("proto", "mesh").unwrap();
        scene.connect("inst", None, ".root", "objects").unwrap();
        scene
            .connect("proto", None, "inst", "sourcemodels")
            .unwrap();
        scene
            .set_attribute(
                "inst",
                vec![doubles("transformationmatrices", two.clone())],
            )
            .unwrap();
        for (time, value) in disabled_at {
            scene
                .set_attribute_at_time(
                    "inst",
                    time,
                    vec![integers("disabledinstances", vec![value])],
                )
                .unwrap();
        }
        scene
    };

    // `[1]` then `[0]`: instance 0 is disabled, so instance 1 is drawn.
    let scene = build([(0.0, 1), (1.0, 0)]);
    for time in [0.0, 0.5, 1.0, 9.0] {
        let at = scene.instance_transforms_at("inst", time).unwrap();
        assert_eq!(at.len(), 1, "at {time}");
        assert_eq!(
            at[0].transform[12], 1.0,
            "the last value applies at {time}"
        );
    }

    // Mirrored, as the render mirrors.
    let scene = build([(0.0, 0), (1.0, 1)]);
    let at = scene.instance_transforms_at("inst", 0.5).unwrap();
    assert_eq!(at[0].transform[12], -1.0);

    // And the static reading answers rather than refusing: the value
    // does not depend on a time, so there is nothing to refuse.
    let statically = scene.instance_transforms("inst").unwrap();
    assert_eq!(statically.len(), 1);
    assert_eq!(statically[0].transform[12], -1.0);
}

/// A sample that changes the instance count is dropped, not blended.
///
/// 3Delight refuses the change rather than the instancer -- `E6023 ...
/// incompatible with its definition at previously defined time steps
/// and will be ignored` -- and renders the first sample's set, static.
/// Two sharp bands at the `t=0` positions, no third instance, no blur.
#[test]
fn a_sample_changing_the_instance_count_is_ignored() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();
    let three = [
        instance_matrix(-5.0),
        instance_matrix(5.0),
        instance_matrix(0.0),
    ]
    .concat();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![doubles("transformationmatrices", two)],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![doubles("transformationmatrices", three)],
        )
        .unwrap();

    for time in [0.0, 0.5, 1.0, 9.0] {
        let at = scene.instance_transforms_at("inst", time).unwrap();
        assert_eq!(at.len(), 2, "the t=1 sample is ignored, at {time}");
        assert_eq!(at[0].transform[12], -1.0, "and the set stays static");
        assert_eq!(at[1].transform[12], 1.0);
    }
}

/// `shader_attribute_value` refuses an unreachable geometry.
///
/// It used to answer from the geometry's own attributes *before*
/// walking the chain, so a detached, cyclic or multi-parent node
/// returned `Ok(Some(..))`. Routing it through the path form made it
/// walk first, which agrees with `attribute_value` and with its own
/// documented errors -- but the commit that did it said "no behaviour
/// change", and nothing pinned the difference either way.
#[test]
fn shader_attribute_value_refuses_an_unreachable_geometry() {
    // Detached.
    let mut scene = Scene::default();
    scene.create("det", "mesh").unwrap();
    scene
        .set_attribute("det", vec![integers("tint", vec![1])])
        .unwrap();
    assert!(matches!(
        scene.shader_attribute_value("det", "tint"),
        Err(ResolveError::Detached { .. })
    ));
    assert!(matches!(
        scene.attribute_value("det", "tint"),
        Err(ResolveError::Detached { .. })
    ));

    // More than one parent.
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, ".root", "objects").unwrap();
    scene.connect("b", None, ".root", "objects").unwrap();
    scene.connect("q", None, "a", "objects").unwrap();
    scene.connect("q", None, "b", "objects").unwrap();
    scene
        .set_attribute("q", vec![integers("tint", vec![1])])
        .unwrap();
    assert!(matches!(
        scene.shader_attribute_value("q", "tint"),
        Err(ResolveError::MultipleParents { .. })
    ));

    // ...and the path form still answers, which is the way out.
    let placements = scene.placements("q").unwrap();
    assert_eq!(
        scene
            .shader_attribute_value_along(&placements[0].path, "tint")
            .expect("on the primitive")
            .node,
        "q",
    );
}

/// `-0.0` names the sample at `0.0`, including when it is *interior*.
///
/// The recorder folds the two when storing and the renderer reads `-0`
/// as `+0`. With samples at `-1`, `0` and `1`, a query at `-0.0` missed
/// the exact hit under `total_cmp`, was not clamped by either end, and
/// bracketed no pair -- so it errored on a sample that plainly exists.
/// The earlier test could not catch it: its fixture starts at `0`, so
/// the leading clamp answered regardless.
#[test]
fn negative_zero_finds_an_interior_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    for (time, x) in [(-1.0, -8.0), (0.0, 5.0), (1.0, 20.0)] {
        scene
            .set_attribute_at_time("xf", time, vec![translate(x, 0.0, 0.0)])
            .unwrap();
        scene
            .set_attribute_at_time(
                "inst",
                time,
                vec![doubles("transformationmatrices", instance_matrix(x))],
            )
            .unwrap();
    }

    assert_eq!(
        scene.world_transform_interpolated_at("xf", -0.0).unwrap()[12],
        5.0,
    );
    assert_eq!(
        scene.instance_transforms_at("inst", -0.0).unwrap()[0].transform[12],
        5.0,
    );
    assert_eq!(
        scene.placements_at("q", -0.0).unwrap()[0].transform[12],
        5.0,
    );

    // And the *non*-interpolating accessor, which has its own exact-hit
    // scan and cannot share `locate_sample` -- it must refuse where that
    // one clamps. Folding in only one place left these two disagreeing
    // on this scene, one answering and one naming a sample the recorder
    // had already folded.
    assert_eq!(scene.world_transform_at("xf", -0.0).unwrap()[12], 5.0);
}

/// Sampled `modelindices` takes its last value too.
///
/// Nothing covered it: returning `None` for `modelindices` outright
/// left the whole suite green, so the rule was carried only by its
/// sibling. Rendered with three prototypes and `modelindices` `[0 1]`
/// at `t=0`, `[1 2]` at `t=0.5` and `[2 0]` at `t=1`, 3Delight draws
/// instance 0 from prototype **2** and instance 1 from prototype **0**
/// -- the last value, at every time.
#[test]
fn sampled_model_indices_take_their_last_value() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    for (handle, index) in [("a", 0), ("b", 1), ("c", 2)] {
        scene.create(handle, "mesh").unwrap();
        scene
            .connect_with_args(
                handle,
                None,
                "inst",
                "sourcemodels",
                vec![integers("index", vec![index])],
            )
            .unwrap();
    }
    scene.connect("inst", None, ".root", "objects").unwrap();

    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();
    scene
        .set_attribute("inst", vec![doubles("transformationmatrices", two)])
        .unwrap();
    for (time, indices) in
        [(0.0, vec![0, 1]), (0.5, vec![1, 2]), (1.0, vec![2, 0])]
    {
        scene
            .set_attribute_at_time(
                "inst",
                time,
                vec![integers("modelindices", indices)],
            )
            .unwrap();
    }

    for time in [0.0, 0.25, 1.0] {
        let at = scene.instance_transforms_at("inst", time).unwrap();
        assert_eq!(at.len(), 2);
        // `source` is a position in `instance_sources`, and the
        // prototypes sort by their `index`: a=0, b=1, c=2.
        assert_eq!(at[0].source, 2, "the last value applies at {time}");
        assert_eq!(at[1].source, 0);
    }
}

/// A wrong-typed later sample clears the attribute rather than being
/// skipped.
///
/// Rendered: `disabledinstances` as a good `int [1]` at `t=0` followed
/// by an `int64` at `t=1` draws **both** instances -- 3Delight warns
/// and treats the attribute as unset. Matching the type inside the
/// lookup skipped the `int64` and answered `[1]` from `t=0`, which is
/// the discarded sample.
#[test]
fn a_wrong_typed_later_sample_clears_the_attribute() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();
    scene
        .set_attribute("inst", vec![doubles("transformationmatrices", two)])
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![integers("disabledinstances", vec![1])],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![OwnedArg {
                name: "disabledinstances".to_string(),
                type_tag: Type::I64,
                array_length: 1,
                flags: 0,
                data: OwnedData::I64(vec![0]),
            }],
        )
        .unwrap();

    let at = scene.instance_transforms_at("inst", 0.5).unwrap();
    assert_eq!(at.len(), 2, "the int64 clears it; nothing is disabled");
}

/// Survivors come back in **time** order however they were set.
///
/// Call order decides *which* samples survive; interpolation then
/// needs them on the timeline. A reviewer deleted the sort and the
/// whole suite stayed green: every out-of-order fixture until now had
/// one survivor, or survivors that were already in time order. Both
/// sampled paths are pinned here, because they were two copies of this
/// mistake waiting to happen.
#[test]
fn survivors_are_ordered_by_time_however_they_were_set() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    // The later time is set first.
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.5).unwrap()[12],
        0.0,
        "halfway between -1 and +1",
    );
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.25).unwrap()[12],
        -0.5,
        "a quarter of the way, which an unsorted pair gets wrong",
    );

    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![doubles("transformationmatrices", instance_matrix(1.0))],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![doubles("transformationmatrices", instance_matrix(-1.0))],
        )
        .unwrap();

    let at = scene.instance_transforms_at("inst", 0.25).unwrap();
    assert_eq!(at.len(), 1);
    assert_eq!(at[0].transform[12], -0.5, "the instancer sorts too");
}

/// The last **defined** sample wins, not the last by time.
///
/// A stream that sets `t=1` before `t=0` separates the two, and
/// `time_attrs` is sorted by time, so `Node::sample_order` is what
/// carries the difference.
///
/// Rendered: `disabledinstances [0]` at `t=1` defined first, then `[1]`
/// at `t=0`, draws instance **0** -- the `t=0` value, because it was
/// defined last. This crate answered instance 1 until the order was
/// recorded, which was an `Open` row and a wrong answer.
#[test]
fn the_last_defined_sample_wins_not_the_last_by_time() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();
    scene
        .set_attribute("inst", vec![doubles("transformationmatrices", two)])
        .unwrap();

    // Defined later-time first, as an out-of-order stream would.
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![integers("disabledinstances", vec![0])],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![integers("disabledinstances", vec![1])],
        )
        .unwrap();

    let at = scene.instance_transforms_at("inst", 0.5).unwrap();
    assert_eq!(at.len(), 1);
    assert_eq!(
        at[0].transform[12], -1.0,
        "the `t=0` sample `[1]` was defined last, so instance 1 is \
         disabled and instance 0 draws at x=-1 -- what 3Delight renders",
    );
}

/// A wrong-typed last sample unsets a *transform* too, at every time.
///
/// The rule was stated in three scans and only one had it. Rendered: a
/// good `doublematrix` at `t=0` followed by a `float` at `t=1` makes
/// 3Delight draw the node at **identity** -- the attribute is unset,
/// not held at the discarded `t=0` sample. This crate gave three
/// different answers for that one scene: the interpolating accessor
/// held `t=0`, the exact one erred at `t=0.5`, and
/// `world_transform_samples` reported a motion sweep the renderer does
/// not draw.
#[test]
fn a_wrong_typed_last_transform_sample_unsets_it() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time(
            "xf",
            1.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();

    // Unset, so the node has no transform: identity, at every time and
    // through every accessor.
    assert_eq!(scene.world_transform("q").unwrap(), super::IDENTITY);
    for time in [0.0, 0.5, 1.0] {
        assert_eq!(
            scene.world_transform_at("q", time).unwrap(),
            super::IDENTITY,
            "exact accessor at {time}",
        );
        assert_eq!(
            scene.world_transform_interpolated_at("q", time).unwrap(),
            super::IDENTITY,
            "interpolating accessor at {time}",
        );
    }
}

/// And a wrong-typed last sample unsets an instancer's matrices, which
/// 3Delight renders as nothing at all rather than as the earlier set.
#[test]
fn a_wrong_typed_last_matrices_sample_draws_nothing() {
    let mut scene = Scene::default();
    scene.create("inst", "instances").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();

    let two = [instance_matrix(-1.0), instance_matrix(1.0)].concat();
    scene
        .set_attribute_at_time(
            "inst",
            0.0,
            vec![doubles("transformationmatrices", two)],
        )
        .unwrap();
    scene
        .set_attribute_at_time(
            "inst",
            1.0,
            vec![integers("transformationmatrices", vec![0])],
        )
        .unwrap();

    for time in [0.0, 0.5, 1.0] {
        assert!(
            scene
                .instance_transforms_at("inst", time)
                .unwrap()
                .is_empty(),
            "unset at {time}; the renderer draws nothing",
        );
    }
}

/// A wrong-typed sample that is *not* last is dropped, and the good
/// last one answers.
#[test]
fn a_wrong_typed_earlier_sample_is_dropped() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    scene
        .set_attribute_at_time(
            "xf",
            0.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(4.0, 0.0, 0.0)])
        .unwrap();

    // Only the good sample remains, so it is the whole animation.
    assert_eq!(scene.world_transform_at("q", 1.0).unwrap()[12], 4.0);
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.0).unwrap()[12],
        4.0,
        "one sample is constant, held at every time",
    );

    // And the dropped sample's own time names nothing. This is the
    // only place the drop is observable: every other path filters
    // again downstream, so keeping the unreadable sample in the list
    // changed no other answer.
    assert!(
        matches!(
            scene.world_transform_at("q", 0.0),
            Err(ResolveError::MissingSampleAtTime { .. })
        ),
        "the unreadable sample at t=0 was dropped, so no sample is there",
    );
    assert_eq!(
        scene.motion_times("q").unwrap(),
        vec![1.0],
        "and it is not a motion time either",
    );
}

/// `motion_times` and `attribute_times` disagree on a wrong-typed
/// transform sample, **by design**.
///
/// `motion_times` knows `transformationmatrix` is a `doublematrix`, so
/// it applies ɴsɪ's typing rule and drops what it cannot read.
/// `attribute_times` takes any attribute name and this crate does not
/// carry ɴsɪ's type for each one, so "unreadable" has no meaning there:
/// it reports what was recorded, and `attribute_samples` hands over the
/// arguments for a caller that knows the type.
///
/// Pinned because the two answering differently on one scene is the
/// shape of every defect these rounds have found, and this is the one
/// place it is intended.
#[test]
fn motion_times_and_attribute_times_differ_on_an_unreadable_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene
        .set_attribute_at_time(
            "xf",
            0.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(2.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.motion_times("xf").unwrap(),
        vec![1.0],
        "the unreadable sample is not a motion time",
    );
    assert_eq!(
        scene.attribute_times("xf", "transformationmatrix").unwrap(),
        vec![0.0, 1.0],
        "but it was recorded, and this reports what was recorded",
    );
    assert_eq!(
        scene
            .attribute_samples("xf", "transformationmatrix")
            .unwrap()
            .len(),
        2,
        "with the arguments, so a caller that knows the type can judge",
    );
}

/// An unreadable sample **discards every sample before it**.
///
/// This is the scene that settles the rule, and nothing in the suite
/// distinguished it before: a good matrix at `t=0`, a `float` at `t=1`,
/// a good one at `t=2` renders as a **static** object at the `t=2`
/// matrix -- one lit band, where the control without the `float` sweeps
/// across four. Keeping the two good samples and dropping the
/// unreadable one, which this rule first said, produces that sweep.
///
/// `a_wrong_typed_earlier_sample_is_dropped` cannot catch it: with
/// wrong@0 and good@1 both rules give the same answer.
#[test]
fn an_unreadable_sample_discards_the_ones_before_it() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time(
            "xf",
            1.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();
    scene
        .set_attribute_at_time("xf", 2.0, vec![translate(-3.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.motion_times("q").unwrap(),
        vec![2.0],
        "only the samples after the unreadable one survive",
    );
    // One surviving sample is constant: the same matrix at every time.
    for time in [0.0, 1.0, 2.0, 9.0] {
        assert_eq!(
            scene.world_transform_interpolated_at("q", time).unwrap()[12],
            -3.0,
            "static at the t=2 matrix, at {time}",
        );
    }

    // A fourth sample after the survivor, so this separates "keep the
    // tail" from "keep only the last named" -- with a single survivor
    // both give the same answer, and 16 unrelated tests were carrying
    // that distinction.
    scene
        .set_attribute_at_time("xf", 3.0, vec![translate(-9.0, 0.0, 0.0)])
        .unwrap();
    assert_eq!(
        scene.motion_times("q").unwrap(),
        vec![2.0, 3.0],
        "the whole surviving tail, not just the last sample",
    );
    assert_eq!(
        scene.world_transform_interpolated_at("q", 2.5).unwrap()[12],
        -6.0,
        "halfway between the two survivors",
    );
}

/// Sixteen `double`s are not a `doublematrix`.
///
/// `matrix_of` matched the payload and ignored the declared type, so a
/// `"transformationmatrix" "double" 16 [...]` read as a matrix. 3Delight
/// warns `E6007` and draws the node at identity. Six sites share this
/// predicate, so the leniency defined the typing rule everywhere.
#[test]
fn a_double_typed_matrix_is_not_a_matrix() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    #[rustfmt::skip]
    let sixteen = vec![
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        -1.5, 0.0, 0.0, 1.0,
    ];
    scene
        .set_attribute(
            "xf",
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F64,
                array_length: 1,
                flags: 0,
                data: OwnedData::F64(sixteen),
            }],
        )
        .unwrap();

    assert_eq!(
        scene.world_transform("q").unwrap(),
        super::IDENTITY,
        "the declared type is not `doublematrix`, so there is no matrix",
    );
}

/// A count-changing sample drops **only itself**, unlike an unreadable
/// one, which discards every sample before it.
///
/// The two rules sit next to each other and behave oppositely, and only
/// one was tested. Rendered: three matrices at `t=0`, two at `t=0.5`,
/// three at `t=1` gives an image **identical** to the same scene
/// without the middle sample -- so `E6023` drops that sample and the
/// neighbours still blur between them. `E6007`, the type error, does
/// the other thing.
///
/// The earlier count test used two samples with the change last, where
/// "drop it" and "discard before it" agree.
#[test]
fn a_count_change_drops_only_itself_unlike_a_type_error() {
    let three_at = |x: f64| {
        [
            instance_matrix(-x),
            instance_matrix(x),
            instance_matrix(0.0),
        ]
        .concat()
    };

    let build = |with_middle: bool| {
        let mut scene = Scene::default();
        scene.create("inst", "instances").unwrap();
        scene.create("proto", "mesh").unwrap();
        scene.connect("inst", None, ".root", "objects").unwrap();
        scene
            .connect("proto", None, "inst", "sourcemodels")
            .unwrap();
        scene
            .set_attribute_at_time(
                "inst",
                0.0,
                vec![doubles("transformationmatrices", three_at(1.0))],
            )
            .unwrap();
        if with_middle {
            let two = [instance_matrix(-0.2), instance_matrix(0.2)].concat();
            scene
                .set_attribute_at_time(
                    "inst",
                    0.5,
                    vec![doubles("transformationmatrices", two)],
                )
                .unwrap();
        }
        scene
            .set_attribute_at_time(
                "inst",
                1.0,
                vec![doubles("transformationmatrices", three_at(1.5))],
            )
            .unwrap();
        scene
    };

    // Identical answers with and without the mismatched middle sample,
    // as the renders are identical.
    for time in [0.0, 0.25, 0.75, 1.0] {
        assert_eq!(
            build(true).instance_transforms_at("inst", time).unwrap(),
            build(false).instance_transforms_at("inst", time).unwrap(),
            "the count-changing sample is dropped, not a barrier, at {time}",
        );
    }

    // And it still blurs across the middle, which "discard before"
    // would have collapsed to the `t=1` set.
    let mid = build(true).instance_transforms_at("inst", 0.5).unwrap();
    assert_eq!(mid.len(), 3);
    assert_eq!(mid[0].transform[12], -1.25, "halfway between -1 and -1.5");
}

/// A good sample re-set at the **same time** as an unreadable one: this
/// crate sweeps, 3Delight draws static.
///
/// In time order: a good matrix at `t=0`, a `float` at `t=1`, a good
/// matrix at `t=1`. Rendered, 3Delight draws a **static** object at the
/// `t=1` matrix -- the `float` unset the attribute when it arrived, and
/// the good sample at the same time re-set it alone. This crate reports
/// a sweep from `t=0`.
///
/// A re-set at a time already recorded is another **call**, so what it
/// superseded is still part of the record -- and that is the
/// difference between `good` replacing `good`, which sweeps, and
/// `good` replacing an unreadable sample, which does not.
///
/// Rendered, in time order: good at `t=0`, a `float` at `t=1`, good at
/// `t=1` draws a **static** object at the `t=1` matrix, because the
/// `float` unset the attribute on arrival and the good sample re-set
/// it alone. This crate reported a sweep from `t=0` for as long as
/// `set_attribute_at_time` replaced the value in a slot keyed by time,
/// which erased the `float` before any rule could see it. The last of
/// the three divergences a call log closes.
#[test]
fn a_same_time_reset_after_an_unreadable_sample_stands_alone() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time(
            "xf",
            1.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();
    // Same time, superseding the unreadable one.
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(-3.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.motion_times("q").unwrap(),
        vec![1.0],
        "the t=0 sample did not survive the float",
    );
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.5).unwrap()[12],
        -3.0,
        "static at the t=1 matrix, which is what 3Delight draws",
    );

    // And the other direction: a readable sample superseding a
    // readable one merely replaces it, and the sweep survives.
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(-9.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(-3.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(scene.motion_times("q").unwrap(), vec![0.0, 1.0]);
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.5).unwrap()[12],
        -2.25,
        "halfway between -1.5 and the re-set -3.0",
    );
}

/// An unreadable sample defined *before* a good one at an earlier time.
///
/// Rendered: `float`@1 defined first, then a good matrix@0, draws
/// **static at the t=0 matrix** -- pixel-identical to the good sample
/// alone. The `float` was superseded by a later call, so it never
/// unset anything. This crate sorts by time, sees the `float` last, and
/// answers identity.
///
/// `E6007` acts at **call** time, so a later definition supersedes an
/// unreadable one whatever their times.
///
/// The `float` is at `t=1` and defined *first*; the good matrix at
/// `t=0` comes after and rebuilds the attribute. 3Delight draws static
/// at the `t=0` matrix. Reading the rule over time-sorted samples made
/// the `float` the last sample and unset the attribute, which is why
/// `Node::sample_order` exists.
#[test]
fn a_later_definition_supersedes_an_unreadable_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    // Defined first, at the later time.
    scene
        .set_attribute_at_time(
            "xf",
            1.0,
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::F32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(vec![0.5]),
            }],
        )
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.0).unwrap()[12],
        -1.5,
        "the float was superseded by a later call, so the t=0 matrix \
         stands alone and is held -- what 3Delight draws",
    );
}

/// `Sampled::samples` must look up the attribute it was asked for.
///
/// Nothing pinned the `name` argument: reading the slot's first value
/// instead survived the whole suite, because no fixture recorded two
/// attributes in one time slot with the non-queried one first.
#[test]
fn sampled_reads_the_attribute_it_was_asked_for() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    // Two attributes in one slot, the queried one second.
    for (time, x) in [(0.0, -1.5), (1.0, -3.0)] {
        scene
            .set_attribute_at_time(
                "xf",
                time,
                vec![
                    integers("disabledinstances", vec![0]),
                    translate(x, 0.0, 0.0),
                ],
            )
            .unwrap();
    }

    assert_eq!(scene.motion_times("q").unwrap(), vec![0.0, 1.0]);
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.5).unwrap()[12],
        -2.25,
        "the transform, not whichever attribute happens to be first",
    );
}

/// The **truncation** path, in all four call orders 3Delight
/// distinguishes.
///
/// One `float` and two good matrices, and where the `float` falls in
/// the *call* order decides what survives it. All four build
/// byte-identical `time_attrs`, since that is sorted by time -- so
/// nothing but `Node::sample_order` can tell them apart, and this crate
/// gave all four the same answer until it recorded one.
///
/// Rendered, each explained by "an unreadable call unsets the attribute
/// at call time, and what comes after rebuilds it":
///
///   float, g0, g1  -> sweeps        (both goods rebuilt it)
///   g0, float, g1  -> static -3.0   (only g1 survives)
///   g1, float, g0  -> static -1.5   (only g0 survives)
///   g0, g1, float  -> identity      (nothing rebuilt it)
#[test]
fn an_unreadable_sample_discards_only_what_was_defined_before_it() {
    let float = || OwnedArg {
        name: "transformationmatrix".to_string(),
        type_tag: Type::F32,
        array_length: 1,
        flags: 0,
        data: OwnedData::F32(vec![0.5]),
    };

    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();

    scene
        .set_attribute_at_time("xf", 0.5, vec![float()])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![translate(-1.5, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![translate(-3.0, 0.0, 0.0)])
        .unwrap();

    assert_eq!(
        scene.motion_times("q").unwrap(),
        vec![0.0, 1.0],
        "both goods were defined after the float, so both survive",
    );
    assert_eq!(
        scene.world_transform_interpolated_at("q", 0.0).unwrap()[12],
        -1.5,
        "and the sweep starts at the t=0 matrix",
    );

    // The same three calls in the three other orders.
    type Calls = [(f64, Option<f64>); 3];
    let orders: [(Calls, f64); 3] = [
        ([(0.0, Some(-1.5)), (0.5, None), (1.0, Some(-3.0))], -3.0),
        ([(1.0, Some(-3.0)), (0.5, None), (0.0, Some(-1.5))], -1.5),
        ([(0.0, Some(-1.5)), (1.0, Some(-3.0)), (0.5, None)], 0.0),
    ];
    for (calls, expected) in orders {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.create("q", "mesh").unwrap();
        scene.connect("xf", None, ".root", "objects").unwrap();
        scene.connect("q", None, "xf", "objects").unwrap();
        for (time, x) in calls {
            let arg = match x {
                Some(x) => translate(x, 0.0, 0.0),
                None => float(),
            };
            scene.set_attribute_at_time("xf", time, vec![arg]).unwrap();
        }
        assert_eq!(
            scene.world_transform_interpolated_at("q", 0.0).unwrap()[12],
            expected,
            "call order decides what the float discarded",
        );
    }
}

/// An attribute set only through `SetAttributeAtTime` is gathered.
///
/// Rendered: an `attributes` node whose `visibility` is `0` set **only**
/// with `SetAttributeAtTime` hides the object -- alpha 0.000, identical
/// to setting it statically, where a scene with nothing set renders at
/// 1.000. `SetAttributeAtTime` on an attribute that is not motion data
/// sets it for the whole shutter.
///
/// This crate read `node.attrs` alone and answered "not set", which is
/// a silent wrong answer: a backend would have drawn a hidden object.
/// The same rule was already applied to an instancer's `modelindices`
/// and `disabledinstances`, and to nothing else.
#[test]
fn an_attribute_set_only_at_a_time_is_gathered() {
    let mut scene = Scene::default();
    scene.create("m", "mesh").unwrap();
    scene.create("a", "attributes").unwrap();
    scene.connect("m", None, ".root", "objects").unwrap();
    scene.connect("a", None, "m", "geometryattributes").unwrap();
    scene
        .set_attribute_at_time("a", 0.0, vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("m", "visibility").unwrap();
    let value = value.expect("3Delight honours it; so must this");
    assert_eq!(value.node, "a");
    assert_eq!(
        value.arg.expect("a recorded value").data,
        OwnedData::I32(vec![0])
    );
}

/// And its `ATTR.priority` is read the same way, so a sampled priority
/// still outranks proximity.
#[test]
fn a_sampled_attr_priority_is_read() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility", vec![0])])
        .unwrap();
    scene
        .set_attribute_at_time(
            "far",
            0.0,
            vec![
                integers("visibility", vec![1]),
                integers("visibility.priority", vec![10]),
            ],
        )
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("defined");
    assert_eq!(value.node, "far", "the sampled priority still wins");
    assert_eq!(value.priority, 10);
}

/// The same for a shader attribute, including on the primitive itself.
#[test]
fn a_sampled_shader_attribute_is_gathered() {
    let mut scene = Scene::default();
    scene.create("m", "mesh").unwrap();
    scene.create("sa", "attributes").unwrap();
    scene.connect("m", None, ".root", "objects").unwrap();
    scene.connect("sa", None, "m", "shaderattributes").unwrap();
    scene
        .set_attribute_at_time("sa", 0.0, vec![integers("tint", vec![5])])
        .unwrap();

    assert_eq!(
        scene
            .shader_attribute_value("m", "tint")
            .unwrap()
            .unwrap()
            .node,
        "sa",
    );

    // And on the primitive, which outranks every container.
    scene
        .set_attribute_at_time("m", 0.0, vec![integers("tint", vec![9])])
        .unwrap();
    assert_eq!(
        scene
            .shader_attribute_value("m", "tint")
            .unwrap()
            .unwrap()
            .node,
        "m",
    );
}

/// A multi-parent geometry can get its motion times, per path.
///
/// `motion_times` walks the chain itself and so refuses one; a backend
/// emitting per-sample transforms for an instanced object had to
/// re-walk the chain by hand. `motion_times_along` takes the path a
/// `Placement` already carries.
#[test]
fn motion_times_are_available_along_a_placement_path() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.create("xfA", "transform").unwrap();
    scene.create("xfB", "transform").unwrap();
    scene.connect("xfA", None, ".root", "objects").unwrap();
    scene.connect("xfB", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xfA", "objects").unwrap();
    scene.connect("q", None, "xfB", "objects").unwrap();
    scene
        .set_attribute_at_time("xfA", 0.0, vec![translate(0.0, 0.0, 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xfA", 1.0, vec![translate(10.0, 0.0, 0.0)])
        .unwrap();
    // The B path is static.
    scene
        .set_attribute("xfB", vec![translate(-4.0, 0.0, 0.0)])
        .unwrap();

    assert!(matches!(
        scene.motion_times("q"),
        Err(ResolveError::MultipleParents { .. })
    ));

    let placements = scene.placements_at("q", 0.0).unwrap();
    assert_eq!(
        scene.motion_times_along(&placements[0].path),
        vec![0.0, 1.0],
        "the moving path",
    );
    assert!(
        scene.motion_times_along(&placements[1].path).is_empty(),
        "the static path",
    );
}
