//! Tests for [`super`].
//!
//! Separate file per the workspace rule: source files do not grow
//! inline `#[cfg(test)]` modules.

use super::*;
use crate::OwnedData;
use nsi_trait::Type;

/// An ɴsɪ `"strength"` connection argument.
fn strength(value: i32) -> OwnedArg {
    OwnedArg {
        name: "strength".to_string(),
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

fn arg(name: &str, value: f32) -> OwnedArg {
    OwnedArg {
        name: name.to_string(),
        type_tag: Type::F32,
        array_length: 1,
        flags: 0,
        data: OwnedData::F32(vec![value]),
    }
}

#[test]
fn creates_and_finds_a_node() {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    assert_eq!(scene.nodes["cam"].node_type, "perspectivecamera");
}

#[test]
fn set_attribute_overwrites_by_name() {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    scene.set_attribute("cam", vec![arg("fov", 45.0)]).unwrap();
    scene.set_attribute("cam", vec![arg("fov", 60.0)]).unwrap();
    assert_eq!(scene.nodes["cam"].attrs.len(), 1);
    assert_eq!(
        scene.nodes["cam"].attrs["fov"].data,
        OwnedData::F32(vec![60.0])
    );
}

#[test]
fn time_samples_are_kept_separately_and_sorted() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![arg("t", 1.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)])
        .unwrap();
    let times: Vec<f64> = scene.nodes["xf"]
        .time_attrs
        .iter()
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(times, vec![0.0, 1.0]);
    assert!(scene.nodes["xf"].attrs.is_empty());
}

#[test]
fn delete_removes_the_node_and_its_edges() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("mesh", "mesh").unwrap();
    scene
        .connect("mesh", None, "xf", "objects")
        .expect("known attribute");
    scene.delete("xf").unwrap();
    assert!(!scene.nodes.contains_key("xf"));
    assert!(scene.edges.is_empty());
}

#[test]
fn delete_attribute_removes_one_key() {
    let mut scene = Scene::default();
    scene.create("cam", "perspectivecamera").unwrap();
    scene
        .set_attribute("cam", vec![arg("fov", 45.0), arg("fs", 1.0)])
        .unwrap();
    scene.delete_attribute("cam", "fov");
    assert!(!scene.nodes["cam"].attrs.contains_key("fov"));
    assert!(scene.nodes["cam"].attrs.contains_key("fs"));
}

/// Node order is insertion order. The `.nsi` stream diff against
/// 3Delight is meaningless if replay reorders nodes.
#[test]
fn node_order_is_insertion_order() {
    let mut scene = Scene::default();
    for handle in ["z", "a", "m"] {
        scene.create(handle, "transform").unwrap();
    }
    let order: Vec<&str> = scene.nodes.keys().map(String::as_str).collect();
    assert_eq!(order, vec!["z", "a", "m"]);
}

/// An unmapped destination must propagate, not be swallowed.
#[test]
fn connect_carries_an_unlisted_destination() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, "b", "nonsense").unwrap();
    assert_eq!(
        scene.edges[0].kind,
        EdgeKind::Other {
            to_attr: "nonsense".to_string()
        }
    );
}

/// `delete_attribute` walks the motion samples too. Only the static
/// path was proven before, and the two are separate tables.
#[test]
fn delete_attribute_removes_from_every_time_sample() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.set_attribute("xf", vec![arg("t", 9.0)]).unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![arg("t", 1.0), arg("keep", 2.0)])
        .unwrap();

    scene.delete_attribute("xf", "t");

    let node = &scene.nodes["xf"];
    assert!(!node.attrs.contains_key("t"), "static copy removed");
    for (time, attrs) in &node.time_attrs {
        assert!(!attrs.contains_key("t"), "sample at {time} still has it");
    }
    assert!(node.time_attrs[1].1.contains_key("keep"));
}

/// `disconnect` removes the edge it names and leaves the others.
#[test]
fn disconnect_removes_only_the_named_edge() {
    let mut scene = Scene::default();
    for handle in ["a", "b"] {
        scene.create(handle, "transform").unwrap();
        scene.connect(handle, None, ".root", "objects").unwrap();
    }

    scene.disconnect("a", None, ".root", "objects").unwrap();

    assert_eq!(scene.edges.len(), 1);
    assert_eq!(scene.edges[0].from, "b");
}

/// Classification is how `disconnect` identifies the edge, so an
/// unlisted destination removes exactly the edge that carries it.
#[test]
fn disconnect_removes_an_unlisted_destination() {
    let mut scene = Scene::default();
    scene.create("a", "transform").unwrap();
    scene.create("b", "transform").unwrap();
    scene.connect("a", None, "b", "nonsense").unwrap();
    scene.disconnect("a", None, "b", "nonsense").unwrap();
    assert!(scene.edges.is_empty());
}

/// ɴsɪ: `NSISetAttribute` "replaces any value previously set by
/// NSISetAttribute or NSISetAttributeAtTime". Leaving the samples behind
/// makes a node that was set static after being sampled look
/// motion-blurred to the resolver forever.
#[test]
fn a_static_set_clears_the_motion_samples_of_that_name() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 1.0, vec![arg("t", 1.0), arg("keep", 9.0)])
        .unwrap();

    scene.set_attribute("xf", vec![arg("t", 5.0)]).unwrap();

    let node = &scene.nodes["xf"];
    assert_eq!(node.attrs["t"].data, OwnedData::F32(vec![5.0]));
    for (time, sample) in &node.time_attrs {
        assert!(!sample.contains_key("t"), "sample at {time} survived");
    }
    assert!(
        node.time_attrs.iter().any(|(_, s)| s.contains_key("keep")),
        "an unrelated sampled attribute is untouched"
    );
}

/// And the other direction: `NSISetAttributeAtTime` "replaces any
/// value previously set by NSISetAttribute".
#[test]
fn a_sampled_set_clears_the_static_value_of_that_name() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute("xf", vec![arg("t", 5.0), arg("keep", 9.0)])
        .unwrap();

    scene
        .set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)])
        .unwrap();

    let node = &scene.nodes["xf"];
    assert!(!node.attrs.contains_key("t"), "static value replaced");
    assert!(node.attrs.contains_key("keep"), "others untouched");
}

/// ɴsɪ: "It is not an error to create a connection which already
/// exists." Recording it twice would make the node look like it had
/// two parents, failing resolution for it and everything beneath it.
#[test]
fn a_repeated_connect_updates_rather_than_duplicates() {
    let mut scene = Scene::default();
    scene.create("grp", "transform").unwrap();
    scene.connect("grp", None, ".root", "objects").unwrap();
    scene
        .connect_with_args("grp", None, ".root", "objects", vec![priority(4)])
        .unwrap();

    assert_eq!(scene.edges.len(), 1, "one edge, not two parents");
    assert_eq!(scene.edges[0].priority(), 4, "arguments updated");
}

/// ɴsɪ: "the handle for either node, as well as any or all of the
/// attributes, may be the special value `.all`". The documented
/// example is disconnecting everything from the scene root.
#[test]
fn disconnect_all_matches_every_source() {
    let mut scene = Scene::default();
    for handle in ["a", "b", "c", "other"] {
        scene.create(handle, "transform").unwrap();
    }
    scene.connect("a", None, ".root", "objects").unwrap();
    scene.connect("b", None, ".root", "objects").unwrap();
    scene.connect("c", None, "other", "objects").unwrap();

    scene
        .disconnect(crate::ALL, None, ".root", "objects")
        .unwrap();

    assert_eq!(scene.edges.len(), 1);
    assert_eq!(scene.edges[0].to, "other");
}

/// `.all` in the destination handle, and in the attribute name.
#[test]
fn disconnect_all_matches_destinations_and_attributes() {
    let mut scene = Scene::default();
    for handle in ["a", "b", "x", "y", "z"] {
        scene.create(handle, "transform").unwrap();
    }
    scene.connect("a", None, "x", "objects").unwrap();
    scene.connect("a", None, "y", "geometryattributes").unwrap();
    scene.connect("b", None, "z", "objects").unwrap();

    // Every attribute of `a`, whatever it connects to.
    scene.disconnect("a", None, crate::ALL, crate::ALL).unwrap();

    assert_eq!(scene.edges.len(), 1);
    assert_eq!(scene.edges[0].from, "b");
}

/// `.all` as `to_attr` must not be classified -- it names no single
/// class -- and must not error.
#[test]
fn disconnect_with_an_all_attribute_is_not_a_classify_error() {
    let mut scene = Scene::default();
    scene.create("a", "screen").unwrap();
    scene.create("x", "perspectivecamera").unwrap();
    scene.connect("a", None, "x", "screens").unwrap();
    assert!(scene.disconnect("a", None, "x", crate::ALL).is_ok());
    assert!(scene.edges.is_empty());
}

/// 3Delight answers `set_attribute` on an unknown handle with
/// "unknown node handle". Fabricating one is worse than rejecting
/// it: the node joins the scene, satisfies later `connect` calls --
/// defeating the check those do -- and replays as a `Create` the
/// renderer never wrote. The stream gate cannot see it, because a
/// call log records the call either way.
#[test]
fn setting_an_attribute_on_an_uncreated_handle_is_an_error() {
    let mut scene = Scene::default();

    assert_eq!(
        scene.set_attribute("ghost", vec![arg("fov", 45.0)]),
        Err(RecordError::UnknownHandle {
            handle: "ghost".to_string()
        })
    );
    assert_eq!(
        scene.set_attribute_at_time("ghost", 0.0, vec![arg("t", 1.0)]),
        Err(RecordError::UnknownHandle {
            handle: "ghost".to_string()
        })
    );
    assert!(scene.node("ghost").is_none(), "nothing fabricated");

    // And so the connection check cannot be walked around.
    scene.create("real", "transform").unwrap();
    assert!(scene.connect("real", None, "ghost", "objects").is_err());
}

/// The reserved handles are the exception: ɴsɪ says they "don't need
/// to be created", and every scene sets `.global`.
#[test]
fn the_reserved_handles_take_attributes_without_a_create() {
    let mut scene = Scene::default();
    assert!(
        scene
            .set_attribute(crate::GLOBAL, vec![arg("quality", 1.0)])
            .is_ok()
    );
    assert!(scene.node(crate::GLOBAL).is_some());
}

/// ɴsɪ: a recursive delete removes "nodes which connect to the
/// specified node", which is what makes deleting a whole shader
/// network one call.
#[test]
fn a_recursive_delete_takes_the_network_with_it() {
    let mut scene = Scene::default();
    for handle in ["attr", "surface", "texture"] {
        scene.create(handle, "shader").unwrap();
    }
    scene.create("mesh", "mesh").unwrap();
    scene.connect("mesh", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "mesh", "geometryattributes")
        .unwrap();
    scene
        .connect("surface", None, "attr", "surfaceshader")
        .unwrap();
    scene
        .connect("texture", Some("out"), "surface", "color")
        .unwrap();

    scene.delete_recursive("attr").unwrap();

    assert!(scene.node("attr").is_none());
    assert!(scene.node("surface").is_none(), "network followed");
    assert!(scene.node("texture").is_none(), "and transitively");
    assert!(scene.node("mesh").is_some(), "the geometry survives");
}

/// ɴsɪ: a node is spared when it "also has connections which do not
/// eventually lead to the specified node".
#[test]
fn a_recursive_delete_spares_a_node_used_elsewhere() {
    let mut scene = Scene::default();
    scene.create("shared", "shader").unwrap();
    scene.create("a", "attributes").unwrap();
    scene.create("b", "attributes").unwrap();
    scene.connect("shared", None, "a", "surfaceshader").unwrap();
    scene.connect("shared", None, "b", "surfaceshader").unwrap();

    scene.delete_recursive("a").unwrap();

    assert!(scene.node("a").is_none());
    assert!(scene.node("shared").is_some(), "still feeds `b`");
    assert!(scene.node("b").is_some());
}

/// ɴsɪ: "a connection with a strength greater than 0 will block the
/// progression of a recursive NSIDelete."
#[test]
fn strength_blocks_a_recursive_delete() {
    let mut scene = Scene::default();
    scene.create("keep", "shader").unwrap();
    scene.create("go", "shader").unwrap();
    scene.create("attr", "attributes").unwrap();
    scene
        .connect_with_args(
            "keep",
            None,
            "attr",
            "surfaceshader",
            vec![strength(1)],
        )
        .unwrap();
    scene
        .connect("go", None, "attr", "displacementshader")
        .unwrap();

    scene.delete_recursive("attr").unwrap();

    assert!(scene.node("attr").is_none());
    assert!(scene.node("keep").is_some(), "strength blocked it");
    assert!(scene.node("go").is_none(), "the weak connection did not");
}

/// The strength rule has to hold transitively. Checking it only where a
/// candidate is first discovered lets the same node be swept in through
/// a second, weak path -- and ɴsɪ spares a node whose connection to the
/// deleted one has strength, however it was reached.
#[test]
fn strength_blocks_a_recursive_delete_through_a_second_path() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("relay", "attributes").unwrap();
    scene.create("keep", "shader").unwrap();
    // `relay` is swept in: its only connection leads to `attr`.
    scene
        .connect("relay", None, "attr", "surfaceshader")
        .unwrap();
    // `keep` holds `attr` strongly, but also feeds `relay` weakly.
    scene
        .connect_with_args(
            "keep",
            None,
            "attr",
            "displacementshader",
            vec![strength(1)],
        )
        .unwrap();
    scene
        .connect("keep", None, "relay", "surfaceshader")
        .unwrap();

    scene.delete_recursive("attr").unwrap();

    assert!(scene.node("attr").is_none());
    assert!(scene.node("relay").is_none(), "only fed the deleted node");
    assert!(
        scene.node("keep").is_some(),
        "its strong connection spares it, whichever path found it"
    );
}

/// A non-recursive delete still removes only the node named.
#[test]
fn a_plain_delete_is_not_recursive() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();

    scene.delete("attr").unwrap();

    assert!(scene.node("shader").is_some());
}

/// The reserved nodes are not deletable either way.
#[test]
fn a_recursive_delete_still_refuses_the_reserved_nodes() {
    let mut scene = Scene::default();
    assert!(scene.delete_recursive(crate::ROOT).is_err());
}

/// ɴsɪ: "the nodes on which the connection is performed must
/// exist." A connection between handles that were never created
/// builds a graph whose nodes are missing, and resolution then
/// answers for it as though it were real. 3Delight's call log
/// cannot catch this, so the stream gate never would.
#[test]
fn connecting_an_uncreated_handle_is_an_error() {
    let mut scene = Scene::default();
    scene.create("real", "transform").unwrap();

    assert_eq!(
        scene.connect("ghost", None, "real", "objects"),
        Err(RecordError::UnknownHandle {
            handle: "ghost".to_string()
        })
    );
    assert_eq!(
        scene.connect("real", None, "ghost", "objects"),
        Err(RecordError::UnknownHandle {
            handle: "ghost".to_string()
        })
    );
    assert!(scene.edges.is_empty(), "nothing recorded");
}

/// ɴsɪ's `.root` and `.global` "don't need to be created", so they
/// are known without appearing in `nodes`.
#[test]
fn the_reserved_handles_need_no_create() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    assert!(scene.connect("xf", None, crate::ROOT, "objects").is_ok());
    assert!(scene.connect("xf", None, crate::GLOBAL, "objects").is_ok());
}

/// ɴsɪ puts `.all` in *four* positions, the source attribute
/// included. Classifying `Some(".all")` as a port name makes it
/// match nothing, so the call is a silent no-op.
#[test]
fn disconnect_all_matches_every_source_attribute() {
    let mut scene = Scene::default();
    scene.create("s1", "shader").unwrap();
    scene.create("s2", "shader").unwrap();
    scene
        .connect("s1", Some("outColor"), "s2", "inColor")
        .unwrap();
    scene
        .connect("s1", Some("outAlpha"), "s2", "inColor")
        .unwrap();

    scene
        .disconnect("s1", Some(crate::ALL), "s2", "inColor")
        .unwrap();

    assert!(scene.edges.is_empty(), "every source port matched");
}

/// ɴsɪ: "it is not possible to delete the root or the global node."
/// Deleting `.root` here would strip every membership edge in the
/// scene, quietly detaching everything.
#[test]
fn the_reserved_nodes_cannot_be_deleted() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.connect("xf", None, crate::ROOT, "objects").unwrap();

    assert_eq!(
        scene.delete(crate::ROOT),
        Err(RecordError::Reserved {
            handle: crate::ROOT.to_string()
        })
    );
    assert_eq!(scene.edges.len(), 1, "the scene is intact");
    assert!(scene.delete(crate::GLOBAL).is_err());
}

/// ɴsɪ: re-`create` "does nothing if all other parameters match the
/// call which created that node. Otherwise, it emits an error."
#[test]
fn recreating_with_a_different_type_is_an_error() {
    let mut scene = Scene::default();
    scene.create("x", "mesh").unwrap();

    assert_eq!(
        scene.create("x", "transform"),
        Err(RecordError::TypeMismatch {
            handle: "x".to_string(),
            existing: "mesh".to_string(),
            requested: "transform".to_string(),
        })
    );
    assert_eq!(scene.nodes["x"].node_type, "mesh", "type unchanged");
}

/// Re-creating with the same type is the no-op ɴsɪ describes, and
/// must not disturb the node's attributes.
#[test]
fn recreating_with_the_same_type_is_a_no_op() {
    let mut scene = Scene::default();
    scene.create("x", "mesh").unwrap();
    scene.set_attribute("x", vec![arg("fov", 45.0)]).unwrap();

    scene.create("x", "mesh").unwrap();

    assert_eq!(scene.nodes["x"].attrs.len(), 1, "attributes survive");
}

/// 3Delight answers a non-finite sample time with `E6026 invalid time`.
/// Storing one produced a `motion_times` a backend could not iterate:
/// a `NaN` compares equal to nothing, an `inf` sorts past every real
/// shutter time.
#[test]
fn a_non_finite_sample_time_is_refused() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            scene.set_attribute_at_time("xf", bad, vec![arg("t", 1.0)]),
            Err(RecordError::InvalidTime {
                handle: "xf".to_string()
            })
        );
    }
    assert!(scene.node("xf").unwrap().time_attrs.is_empty());
}

/// `-0.0` and `0.0` are **one** sample. The renderer reads a `-0` time
/// as `+0`, so keeping them apart handed a backend two matrices at
/// times that compare equal -- a zero-length motion segment where
/// 3Delight sees a single sample holding the later value.
#[test]
fn negative_zero_is_the_same_sample_time_as_zero() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene
        .set_attribute_at_time("xf", -0.0, vec![arg("t", 5.0)])
        .unwrap();
    scene
        .set_attribute_at_time("xf", 0.0, vec![arg("t", 9.0)])
        .unwrap();

    let samples = &scene.node("xf").unwrap().time_attrs;
    assert_eq!(samples.len(), 1, "one key, as the renderer has");
    assert!(!samples[0].0.is_sign_negative(), "normalised to +0");
    assert_eq!(samples[0].1["t"].data, OwnedData::F32(vec![9.0]));
}

/// ɴsɪ's reserved handles exist already, and 3Delight answers a
/// `create` on one with "already exists". Accepting it kept a node that
/// replay drops, so the scene changed on its own first round trip.
#[test]
fn the_reserved_handles_cannot_be_created() {
    let mut scene = Scene::default();
    for handle in [crate::ROOT, crate::GLOBAL] {
        assert_eq!(
            scene.create(handle, "transform"),
            Err(RecordError::Reserved {
                handle: handle.to_string()
            })
        );
        assert!(scene.node(handle).is_none(), "nothing recorded");
    }
}

/// `Node::effective` is what the resolver reads, so asking a node
/// directly gives the resolver's answer.
///
/// Reading `attrs` alone answers "not set" for an attribute set with
/// `SetAttributeAtTime`, which the renderer honours -- the silent wrong
/// answer this method exists to prevent a caller from reinventing.
#[test]
fn effective_reads_a_sampled_attribute() {
    let mut scene = Scene::default();
    scene.create("a", "attributes").unwrap();
    scene
        .set_attribute_at_time("a", 0.0, vec![arg("visibility", 0.0)])
        .unwrap();

    let node = scene.node("a").expect("created");
    assert!(
        node.attrs.get("visibility").is_none(),
        "it is not a static attribute",
    );
    assert!(
        node.effective("visibility").is_some(),
        "but it is the node's effective value",
    );
}
