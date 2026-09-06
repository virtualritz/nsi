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

/// A chain whose nodes are sampled at different times has no answer
/// without interpolation, and says so rather than composing a
/// mismatched pair.
#[test]
fn a_chain_sampled_at_different_times_is_an_error() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene
        .set_attribute_at_time("outer", 0.25, vec![translate(1.0, 0.0, 0.0)])
        .unwrap();
    scene.create("inner", "transform").unwrap();
    scene
        .set_attribute_at_time("inner", 0.75, vec![translate(2.0, 0.0, 0.0)])
        .unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();

    assert!(scene.world_transform_samples("inner").is_err());
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
    assert_eq!(value.arg.data, OwnedData::I32(vec![0]));
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
    assert_eq!(value.arg.data, OwnedData::I32(vec![1]));
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
    assert_eq!(value.arg.name, "visibility.camera");
    assert_eq!(value.arg.data, OwnedData::I32(vec![0]));
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
    assert_eq!(value.arg.name, "visibility");
    assert_eq!(value.arg.data, OwnedData::I32(vec![0]));
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
    assert_eq!(value.arg.name, "visibility", "priority 5 beats specificity");
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
    assert_eq!(value.arg.name, "visibility.camera");
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

/// The documented divergence, pinned so it cannot change silently.
///
/// A node setting only `visibility.priority` is a definition to
/// 3Delight -- of `visibility` at its default -- and that node wins.
/// This crate has no value to return for it and skips it, so the
/// farther node answers instead. If this ever starts returning `near`,
/// the `Open` row in `contracts/resolution.md` has been closed and the
/// docs must follow.
#[test]
fn a_priority_without_its_attribute_is_skipped() {
    let mut scene = scene_with_two_attribute_levels();
    scene
        .set_attribute("near", vec![integers("visibility.priority", vec![10])])
        .unwrap();
    scene
        .set_attribute("far", vec![integers("visibility", vec![0])])
        .unwrap();

    let value = scene.attribute_value("mesh", "visibility").unwrap();
    let value = value.expect("`far` defines it");
    assert_eq!(
        value.node, "far",
        "3Delight answers `near` at the default; this crate cannot",
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
    assert_eq!(value.arg.data, OwnedData::I32(vec![1]));
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
        vec!["shade".to_string()],
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
