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
    assert_eq!(
        scene.attribute_times("xf", "t").unwrap(),
        vec![0.0, 1.0],
        "recorded later-time-first and reported in time order",
    );
    assert_eq!(
        scene.nodes["xf"].samples["t"]
            .iter()
            .map(|(time, _)| *time)
            .collect::<Vec<_>>(),
        vec![1.0, 0.0],
        "while the log keeps the order they were set in",
    );
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
    assert!(node.samples.get("t").is_none(), "every sample of it too");
    assert!(node.samples.contains_key("keep"));
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
    assert!(node.samples.get("t").is_none(), "its samples went with it");
    assert!(
        node.samples.contains_key("keep"),
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
    assert!(scene.node("xf").unwrap().samples.is_empty());
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

    let samples = scene.attribute_samples("xf", "t").unwrap();
    assert_eq!(samples.len(), 1, "one sample, as the renderer has");
    assert!(!samples[0].0.is_sign_negative(), "normalised to +0");
    assert_eq!(samples[0].1.data, OwnedData::F32(vec![9.0]));
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

/// The value is the last **call**'s, not the greatest time's.
///
/// Rendered, with `visibility` set only through `SetAttributeAtTime`:
/// `t=1 -> 0` then `t=0 -> 1` leaves the object **visible**, and
/// `t=1 -> 1` then `t=0 -> 0` hides it. The same two times, opposite
/// answers, and reading the greatest time gets both backwards -- which
/// is what this did until `Node::samples` became a call log.
#[test]
fn effective_takes_the_last_call_not_the_greatest_time() {
    for (first, second, expected) in
        [(0.0, 1.0, 1.0), (1.0, 0.0, 0.0), (2.0, 3.0, 3.0)]
    {
        let mut scene = Scene::default();
        scene.create("a", "attributes").unwrap();
        // The later time is set first, so the two orders disagree.
        scene
            .set_attribute_at_time("a", 1.0, vec![arg("visibility", first)])
            .unwrap();
        scene
            .set_attribute_at_time("a", 0.0, vec![arg("visibility", second)])
            .unwrap();

        let node = scene.node("a").expect("created");
        assert_eq!(
            node.effective("visibility").expect("set at a time").data,
            OwnedData::F32(vec![expected]),
            "the t=0 call came last",
        );
    }
}

/// Re-setting a time already recorded is another **call**: the log
/// keeps both, the later one is the latest definition, and the two
/// together are one sample.
#[test]
fn a_re_set_time_is_another_call_and_the_later_one_stands() {
    let mut scene = Scene::default();
    scene.create("a", "attributes").unwrap();
    for (time, value) in [(0.0, 1.0), (1.0, 2.0), (0.0, 3.0)] {
        scene
            .set_attribute_at_time("a", time, vec![arg("visibility", value)])
            .unwrap();
    }

    let node = scene.node("a").expect("created");
    assert_eq!(node.samples["visibility"].len(), 3, "three calls");
    assert_eq!(
        node.effective("visibility").expect("set at a time").data,
        OwnedData::F32(vec![3.0]),
        "the last of them",
    );
    assert_eq!(
        scene.attribute_times("a", "visibility").unwrap(),
        vec![0.0, 1.0],
        "but two samples",
    );
}

/// Every field of a [`Node`] is public, so a caller can build one no
/// recorder would. Reading it must not panic: a resolver that aborted
/// a render over a hand-built node would be a worse answer than an
/// attribute that goes quiet.
///
/// There is one table now rather than three, so the states a caller
/// can reach are far narrower than they were -- an empty call list is
/// what is left of them.
#[test]
fn a_hand_built_node_does_not_panic_the_readers() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene
        .nodes
        .get_mut("xf")
        .expect("created")
        .samples
        .insert("transformationmatrix".to_string(), Vec::new());

    let node = scene.node("xf").expect("created");
    assert!(node.effective("transformationmatrix").is_none());
    assert!(scene.motion_times("q").unwrap().is_empty());
    assert!(scene.world_transform("q").is_ok());
    assert!(
        scene
            .attribute_samples("xf", "transformationmatrix")
            .unwrap()
            .is_empty()
    );

    let mut out = Vec::new();
    crate::write_stream(&scene, &mut out).expect("writes");
    assert!(
        !String::from_utf8(out)
            .unwrap()
            .contains("SetAttributeAtTime"),
        "a sample that is not there is not invented",
    );
}

/// A static call clears the log, as ɴsɪ says it should.
#[test]
fn a_static_call_clears_the_call_order() {
    let mut scene = Scene::default();
    scene.create("a", "attributes").unwrap();
    scene
        .set_attribute_at_time("a", 1.0, vec![arg("visibility", 1.0)])
        .unwrap();
    scene
        .set_attribute("a", vec![arg("visibility", 2.0)])
        .unwrap();

    let node = scene.node("a").expect("created");
    assert!(node.samples.get("visibility").is_none());
    assert_eq!(
        node.effective("visibility").expect("static").data,
        OwnedData::F32(vec![2.0]),
    );
}

/// And `delete_attribute` forgets it too.
#[test]
fn delete_attribute_clears_the_log() {
    let mut scene = Scene::default();
    scene.create("a", "attributes").unwrap();
    scene
        .set_attribute_at_time("a", 1.0, vec![arg("visibility", 1.0)])
        .unwrap();
    scene.delete_attribute("a", "visibility");

    let node = scene.node("a").expect("created");
    assert!(node.samples.get("visibility").is_none());
    assert!(node.effective("visibility").is_none());
}

/// The record is **net**, not a log: forty sets of one attribute are
/// one entry, and a handle created and deleted in one interval leaves
/// no live node behind.
#[test]
fn the_record_is_net_not_a_log() {
    let mut scene = Scene::default();
    scene.create("a", "attributes").unwrap();
    for value in 0..40u8 {
        scene
            .set_attribute("a", vec![arg("visibility", f32::from(value))])
            .unwrap();
    }
    scene.create("gone", "mesh").unwrap();
    scene.delete("gone").unwrap();

    let changes = scene.take_changes();
    assert_eq!(changes.attributes.len(), 1, "one name, forty calls");
    assert!(
        changes
            .attributes
            .contains(&("a".to_string(), "visibility".to_string()))
    );
    assert!(changes.created.contains("gone"), "it was created");
    assert_eq!(
        changes.deleted.get("gone").map(String::as_str),
        Some("mesh"),
        "and deleted, with the type the handle no longer has",
    );

    assert_eq!(
        scene.take_changes(),
        Changes::default(),
        "taking clears, so the next synchronise starts empty",
    );
}

/// A repeated `connect` that only changes `"priority"` adds and
/// removes no edge, and changes which shader wins. A record keyed on
/// additions and removals would miss it entirely.
#[test]
fn a_connect_rearmed_in_place_is_recorded() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();
    scene.take_changes();

    scene
        .connect_with_args(
            "shader",
            None,
            "attr",
            "surfaceshader",
            vec![OwnedArg::new(
                "priority",
                Type::I32,
                1,
                0,
                OwnedData::I32(vec![10]),
            )],
        )
        .unwrap();

    let changes = scene.take_changes();
    assert!(changes.edges_added.is_empty(), "no edge appeared");
    assert!(changes.edges_removed.is_empty(), "none disappeared");
    assert_eq!(changes.edges_rearmed.len(), 1);
    assert_eq!(changes.edges_rearmed[0].from, "shader");
}

/// A `disconnect` naming `.all` is expanded into the edges it removed:
/// the pattern cannot be re-expanded once they are gone.
#[test]
fn a_wildcard_disconnect_records_what_it_removed() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    for handle in ["a", "b"] {
        scene.create(handle, "mesh").unwrap();
        scene.connect(handle, None, "xf", "objects").unwrap();
    }
    scene.take_changes();

    scene.disconnect(crate::ALL, None, "xf", "objects").unwrap();

    let changes = scene.take_changes();
    let removed: Vec<&str> = changes
        .edges_removed
        .iter()
        .map(|e| e.from.as_str())
        .collect();
    assert_eq!(removed, vec!["a", "b"], "both, by name, not the pattern");
}

/// A delete takes edges with it, and the record keeps them: working
/// out what was orphaned means walking down from a node the edge no
/// longer points at.
#[test]
fn a_delete_records_the_edges_it_took_with_it() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    scene.connect("q", None, "xf", "objects").unwrap();
    scene.take_changes();

    scene.delete("xf").unwrap();

    let changes = scene.take_changes();
    assert_eq!(changes.deleted.len(), 1);
    let pairs: Vec<(&str, &str)> = changes
        .edges_removed
        .iter()
        .map(|e| (e.from.as_str(), e.to.as_str()))
        .collect();
    assert!(pairs.contains(&("q", "xf")), "the child's edge, {pairs:?}");
    assert!(pairs.contains(&("xf", ".root")), "and its own");
}

/// Pending changes are not part of what a scene *is*: a synchronised
/// scene still equals the identical unsynchronised one.
#[test]
fn changes_are_not_part_of_scene_equality() {
    let build = || {
        let mut scene = Scene::default();
        scene.create("a", "attributes").unwrap();
        scene
            .set_attribute("a", vec![arg("visibility", 1.0)])
            .unwrap();
        scene
    };
    let mut synchronised = build();
    synchronised.take_changes();

    assert_eq!(synchronised, build());
}

/// A transform edit dirties everything under it -- the inverse of the
/// chain walk resolution already does upward.
#[test]
fn a_moved_transform_dirties_its_subtree() {
    let mut scene = Scene::default();
    scene.create("outer", "transform").unwrap();
    scene.create("inner", "transform").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.create("elsewhere", "mesh").unwrap();
    scene.connect("outer", None, ".root", "objects").unwrap();
    scene.connect("inner", None, "outer", "objects").unwrap();
    scene.connect("q", None, "inner", "objects").unwrap();
    scene
        .connect("elsewhere", None, ".root", "objects")
        .unwrap();
    scene.take_changes();

    scene
        .set_attribute("outer", vec![arg("transformationmatrix", 1.0)])
        .unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.nodes.contains("q"), "two levels down");
    assert!(affected.nodes.contains("inner"));
    assert!(
        !affected.nodes.contains("elsewhere"),
        "and nothing on another branch",
    );
}

/// A shader edit reaches every geometry bound through the `attributes`
/// node that carries it -- including when the binding is onto a `set`,
/// and including a repeated `connect` that only changed `"priority"`.
#[test]
fn a_shader_edit_reaches_the_geometry_bound_through_it() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene.create("group", "set").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.create("other", "mesh").unwrap();
    scene.connect("q", None, ".root", "objects").unwrap();
    scene.connect("other", None, ".root", "objects").unwrap();
    scene.connect("q", None, "group", "members").unwrap();
    scene
        .connect("attr", None, "group", "geometryattributes")
        .unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();
    scene.take_changes();

    // Re-arming the connection changes which shader wins and adds no
    // edge; the geometry behind the set must still be named.
    scene
        .connect_with_args(
            "shader",
            None,
            "attr",
            "surfaceshader",
            vec![OwnedArg::new(
                "priority",
                Type::I32,
                1,
                0,
                OwnedData::I32(vec![10]),
            )],
        )
        .unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.nodes.contains("q"), "through the set's members");
    assert!(!affected.nodes.contains("other"), "not the whole scene");
    assert!(affected.shaders.contains("shader"));
}

/// A shader's *own* attribute is a material parameter and no geometry
/// work: it must not drag the geometry bound through it into the
/// affected set.
#[test]
fn a_shader_parameter_edit_is_not_geometry_work() {
    let mut scene = Scene::default();
    scene.create("attr", "attributes").unwrap();
    scene.create("shader", "shader").unwrap();
    scene.create("q", "mesh").unwrap();
    scene.connect("q", None, ".root", "objects").unwrap();
    scene
        .connect("attr", None, "q", "geometryattributes")
        .unwrap();
    scene
        .connect("shader", None, "attr", "surfaceshader")
        .unwrap();
    scene.take_changes();

    scene
        .set_attribute("shader", vec![arg("roughness", 0.5)])
        .unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.shaders.contains("shader"));
    assert!(
        affected.nodes.is_empty(),
        "a material parameter costs no geometry work: {:?}",
        affected.nodes,
    );
}

/// A prototype's ancestor reaches the instancer, which is **not**
/// below the transform that moved: it hangs off the other end of a
/// `sourcemodels` edge.
#[test]
fn a_prototypes_ancestor_reaches_the_instancer() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.create("proto", "mesh").unwrap();
    scene.create("inst", "instances").unwrap();
    scene.connect("inst", None, ".root", "objects").unwrap();
    scene.connect("proto", None, "xf", "objects").unwrap();
    scene
        .connect("proto", None, "inst", "sourcemodels")
        .unwrap();
    scene.take_changes();

    scene
        .set_attribute("xf", vec![arg("transformationmatrix", 1.0)])
        .unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.nodes.contains("proto"));
    assert!(
        affected.nodes.contains("inst"),
        "the instancer draws it, and is reached the other way round",
    );
}

/// An attribute on `.root` or `.global` is a candidate set of
/// "everything", and saying so beats listing the scene.
#[test]
fn a_global_edit_dirties_everything() {
    let mut scene = Scene::default();
    scene.create("q", "mesh").unwrap();
    scene.connect("q", None, ".root", "objects").unwrap();
    scene.take_changes();

    scene
        .set_attribute(crate::GLOBAL, vec![arg("quality.shadingsamples", 8.0)])
        .unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.everything);
    assert!(affected.nodes.is_empty(), "not a copy of the scene");
}

/// A `disconnect` naming `.all` severs several children at once, and
/// each one's subtree is a candidate.
#[test]
fn a_wildcard_disconnect_dirties_every_child_it_severed() {
    let mut scene = Scene::default();
    scene.create("xf", "transform").unwrap();
    scene.connect("xf", None, ".root", "objects").unwrap();
    for handle in ["a", "b"] {
        scene.create(handle, "mesh").unwrap();
        scene.connect(handle, None, "xf", "objects").unwrap();
    }
    scene.take_changes();

    scene.disconnect(crate::ALL, None, "xf", "objects").unwrap();

    let changes = scene.take_changes();
    let affected = scene.affected(&changes);
    assert!(affected.nodes.contains("a"));
    assert!(affected.nodes.contains("b"));
}

/// **The gate for the whole feature: `changed ⊆ affected`.**
///
/// The defect this crate can have here is an *under*-approximation --
/// a node whose resolved answer moved and which the candidate set does
/// not name. A backend acting on that synchronises everything except
/// the thing that changed, and renders the old state with no error and
/// no warning. No hand-written case can be trusted to find it, because
/// the rule that goes missing is the one nobody thought of.
///
/// So: a fixture carrying every feature the walks touch -- nested
/// transforms, an instancer with a prototype under its own transform,
/// a `set` with members, containers bound at two levels with a
/// priority, a shader network -- then a scripted run of edits, and a
/// brute-force comparison of every resolved answer before and after.
/// Over-approximating is allowed; missing one is not.
///
/// What it cannot see: an over-approximation. Dropping the
/// `sourcemodels` hop leaves this green, because moving a prototype's
/// parent transform does not change what the instancer draws in this
/// fixture -- naming the instancer there is a conservative choice, and
/// `a_prototypes_ancestor_reaches_the_instancer` is what pins it.
///
/// What it structurally cannot catch, established by mutating the walk
/// against it rather than assumed:
///
/// - **Over-approximation.** Naming a node that did not move breaks no
///   `changed ⊆ affected`, so filing a shader attribute under `nodes`
///   instead of `shaders` survives here;
///   `a_shader_parameter_edit_is_not_geometry_work` is what pins that.
/// - **A bare `create`.** A node with no edges resolves to the same
///   answers it did before it existed -- `world_transform` says
///   `Detached` for an unknown handle as readily as for an unconnected
///   one -- so dropping the walk over `Changes::created` survives. It
///   is over-approximation either way.
/// - **Dropping an `EdgeKind` arm.** That is now a compile error rather
///   than a surviving mutation: the match is exhaustive over the enum
///   instead of over `to_attr()` strings, so a new edge class stops the
///   build at the place a decision is owed.
///
/// Widened three times, each because a mutation survived it: the
/// script's `transformationmatrix` was an `f32`, which `matrix_of`
/// refuses, so "move a transform" moved nothing; and its priorities
/// were `f32` too, which 3Delight does not read as priorities, so
/// precedence was never exercised. A gate that cannot fail proves only
/// its own fixture.
#[test]
fn every_changed_answer_is_named_in_the_affected_set() {
    // Answers a backend would act on, as text so they compare.
    fn answers(scene: &Scene, handle: &str) -> String {
        format!(
            "{:?}|{:?}|{:?}|{:?}",
            scene.world_transform(handle),
            scene.geometry_binding(handle),
            scene.attribute_value(handle, "visibility"),
            scene.placements(handle).map(|placements| placements
                .iter()
                .map(|placement| (placement.path.clone(), placement.transform))
                .collect::<Vec<_>>()),
        ) + &format!(
            "|{:?}|{:?}|{:?}",
            scene.instance_transforms(handle),
            scene.attribute_value(handle, "visibility.camera"),
            scene.instance_sources(handle),
        )
    }

    fn fixture() -> Scene {
        let mut scene = Scene::default();
        for (handle, node_type) in [
            ("outer", "transform"),
            ("inner", "transform"),
            ("protoxf", "transform"),
            ("q", "mesh"),
            ("r", "mesh"),
            ("proto", "mesh"),
            ("inst", "instances"),
            ("group", "set"),
            ("near", "attributes"),
            ("far", "attributes"),
            ("surface", "shader"),
            ("surface2", "shader"),
            ("texture", "shader"),
            ("cam", "perspectivecamera"),
            ("screen", "screen"),
            ("layer", "outputlayer"),
            ("driver", "outputdriver"),
        ] {
            scene.create(handle, node_type).unwrap();
        }
        for (from, to) in [
            ("outer", ".root"),
            ("inner", "outer"),
            ("q", "inner"),
            ("r", "outer"),
            ("protoxf", ".root"),
            ("proto", "protoxf"),
            ("inst", ".root"),
        ] {
            scene.connect(from, None, to, "objects").unwrap();
        }
        scene
            .connect("proto", None, "inst", "sourcemodels")
            .unwrap();
        // Matrices, or `instance_transforms` is `Ok([])` before and
        // after every edit and the instancer is invisible to the gate.
        scene
            .set_attribute(
                "inst",
                vec![OwnedArg::new(
                    "transformationmatrices",
                    Type::MatrixF64,
                    1,
                    0,
                    OwnedData::F64(
                        [
                            [
                                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                                0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0,
                            ],
                            [
                                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                                0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0,
                            ],
                        ]
                        .concat(),
                    ),
                )],
            )
            .unwrap();
        scene.connect("cam", None, ".root", "objects").unwrap();
        scene.connect("screen", None, "cam", "screens").unwrap();
        scene
            .connect("layer", None, "screen", "outputlayers")
            .unwrap();
        scene
            .connect("driver", None, "layer", "outputdrivers")
            .unwrap();
        scene.connect("q", None, "group", "members").unwrap();
        scene
            .connect("near", None, "q", "geometryattributes")
            .unwrap();
        scene
            .connect("far", None, "group", "geometryattributes")
            .unwrap();
        scene
            .connect("surface", None, "near", "surfaceshader")
            .unwrap();
        // A rival on the same node: which of the two wins is decided
        // by the connection's arguments, so a re-arm that adds and
        // removes nothing changes the answer.
        scene
            .connect("surface2", None, "near", "surfaceshader")
            .unwrap();
        scene
            .connect("texture", Some("out"), "surface", "colour")
            .unwrap();
        scene
            .set_attribute(
                "outer",
                vec![OwnedArg::new(
                    "transformationmatrix",
                    Type::MatrixF64,
                    1,
                    0,
                    OwnedData::F64(vec![
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                        0.0, 1.0, 0.0, 0.0, 1.0,
                    ]),
                )],
            )
            .unwrap();
        scene
            .set_attribute("near", vec![arg("visibility", 1.0)])
            .unwrap();
        scene
            .set_attribute(
                "far",
                vec![
                    arg("visibility", 0.0),
                    OwnedArg::new(
                        "visibility.priority",
                        Type::I32,
                        1,
                        0,
                        OwnedData::I32(vec![10]),
                    ),
                ],
            )
            .unwrap();
        scene
    }

    let handles = [
        "outer", "inner", "protoxf", "q", "r", "proto", "inst", "group",
        "near", "far", "surface", "surface2", "texture", "cam", "screen",
        "layer", "driver",
        // The node a `create` edit makes. Without it in this list no
        // created node was ever checked, and dropping the walk over
        // `Changes::created` left the gate green.
        "fresh",
    ];

    // One edit is applied by `edit`, so the runs below can be
    // exhaustive over (handle x operation) rather than trusting a
    // random draw to have tried the one that matters.
    // A `doublematrix`, declared. An `f32` here is `E6007` to
    // 3Delight and `None` to `matrix_of`, so a script that used one
    // would be editing nothing at all -- which is how this fixture
    // first failed to notice a truncated descent.
    fn matrix(x: f64) -> OwnedArg {
        OwnedArg::new(
            "transformationmatrix",
            Type::MatrixF64,
            1,
            0,
            OwnedData::F64(vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x,
                0.0, 0.0, 1.0,
            ]),
        )
    }

    fn edit(scene: &mut Scene, handle: &str, op: usize) {
        match op {
            0 => {
                let _ = scene.set_attribute(handle, vec![matrix(2.0)]);
            }
            1 => {
                let _ =
                    scene.set_attribute(handle, vec![arg("visibility", 0.0)]);
            }
            2 => {
                let _ =
                    scene.set_attribute_at_time(handle, 0.5, vec![matrix(3.0)]);
            }
            3 => scene.delete_attribute(handle, "visibility"),
            4 => {
                let _ = scene.delete(handle);
            }
            5 => {
                let _ = scene.disconnect(crate::ALL, None, handle, "objects");
            }
            6 => {
                let _ = scene.connect(handle, None, ".root", "objects");
            }
            7 => {
                // Re-arm: no edge added, none removed, and which of the
                // two rival shaders wins changes.
                let _ = scene.connect_with_args(
                    // Keyed on the *handle*, not on `op`: `op` is 7
                    // throughout this arm, so `op % 2` was a constant
                    // and only one of the two rivals was ever re-armed.
                    if handle.len().is_multiple_of(2) {
                        "surface"
                    } else {
                        "surface2"
                    },
                    None,
                    "near",
                    "surfaceshader",
                    vec![OwnedArg::new(
                        "priority",
                        Type::I32,
                        1,
                        0,
                        OwnedData::I32(vec![7]),
                    )],
                );
            }
            9 => {
                // Created and left unconnected, on purpose: connecting
                // it would record an edge, the edge arm would name it,
                // and the walk over `Changes::created` could be deleted
                // with the gate still green.
                let _ = scene.create("fresh", "mesh");
            }
            10 => {
                let _ =
                    scene.disconnect(crate::ALL, None, handle, "sourcemodels");
            }
            _ => {
                // An `int`, and exactly one: anything else is not a
                // priority to 3Delight, so an `f32` here would rank
                // nothing and precedence would never be exercised.
                let _ = scene.set_attribute(
                    handle,
                    vec![OwnedArg::new(
                        "visibility.priority",
                        Type::I32,
                        1,
                        0,
                        OwnedData::I32(vec![20]),
                    )],
                );
            }
        }
    }

    let check =
        |scene: &Scene, before: &Scene, affected: &Affected, what: &str| {
            if affected.everything {
                return;
            }
            // `render_outputs` is one answer for the whole scene, not
            // one per handle, so it is held against the flag that
            // exists for it. Folding it into every handle's answer made
            // deleting a camera look like every node in the scene
            // changing -- which is how this assertion first failed.
            assert!(
                format!("{:?}", before.render_outputs())
                    == format!("{:?}", scene.render_outputs())
                    || affected.outputs,
                "{what}: the outputs changed and `outputs` is false",
            );

            for handle in handles {
                let was = answers(before, handle);
                let now = answers(scene, handle);
                // `nodes` and `shaders` are separate answers -- one
                // costs geometry work and the other does not -- so a
                // geometry answer that moved must be in `nodes`.
                // Accepting either left the distinction the API sells
                // unverified.
                assert!(
                    was == now || affected.nodes.contains(handle),
                    "{what}: `{handle}` answers differently and is not in the \
                 affected set\n  before: {was}\n   after: {now}",
                );
            }
        };

    // Every single edit, on every handle.
    for handle in handles {
        for op in 0..11 {
            let mut scene = fixture();
            let before = scene.clone();
            scene.take_changes();
            edit(&mut scene, handle, op);
            let changes = scene.take_changes();
            let affected = scene.affected(&changes);
            check(&scene, &before, &affected, &format!("{handle}/{op}"));
        }
    }

    // And pairs, for the interactions a single edit cannot show.
    for seed in 0..256u64 {
        let mut scene = fixture();
        let before = scene.clone();
        scene.take_changes();

        let mut state =
            seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        for _ in 0..2 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let pick = (state >> 33) as usize;
            edit(&mut scene, handles[pick % handles.len()], pick % 11);
        }

        let changes = scene.take_changes();
        let affected = scene.affected(&changes);
        check(&scene, &before, &affected, &format!("seed {seed}"));
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
