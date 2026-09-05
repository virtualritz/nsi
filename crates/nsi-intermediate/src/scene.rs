//! Node and attribute tables.
//!
//! `IndexMap` throughout: replaying a scene in a different order than it
//! was recorded would make the `.nsi` stream diff against 3Delight
//! meaningless.

use crate::{ALL, Edge, EdgeKind, OwnedArg, RecordError, classify};
use core::cmp::Ordering;
use indexmap::IndexMap;

/// One ɴsɪ node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Node {
    /// The ɴsɪ node type this handle was created with.
    pub node_type: String,
    /// Attributes set with `set_attribute`, keyed by name.
    pub attrs: IndexMap<String, OwnedArg>,
    /// Attributes set with `set_attribute_at_time`, sorted by time.
    ///
    /// Motion samples are kept apart from static attributes because
    /// transform composition has to happen per sample: flattening an
    /// ɴsɪ transform chain into a single world matrix is a per-time
    /// operation, not a one-off.
    pub time_attrs: Vec<(f64, IndexMap<String, OwnedArg>)>,
}

/// The recorded scene graph.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    /// Nodes by handle, in creation order.
    pub nodes: IndexMap<String, Node>,
    /// Classified connections, in connection order.
    pub edges: Vec<Edge>,
}

impl Scene {
    /// The nodes, by handle, in creation order.
    pub fn nodes(&self) -> impl Iterator<Item = (&String, &Node)> {
        self.nodes.iter()
    }

    /// One node by handle.
    pub fn node(&self, handle: &str) -> Option<&Node> {
        self.nodes.get(handle)
    }

    /// The classified connections, in connection order.
    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter()
    }

    /// Create a node.
    ///
    /// # Errors
    ///
    /// [`RecordError::TypeMismatch`] when the handle exists with a
    /// different type. ɴsɪ: "the function does nothing if all other
    /// parameters match the call which created that node. Otherwise, it
    /// emits an error." Re-creating with the *same* type is a no-op, as
    /// ɴsɪ says.
    pub fn create(
        &mut self,
        handle: &str,
        node_type: &str,
    ) -> Result<(), RecordError> {
        match self.nodes.get(handle) {
            Some(existing) if existing.node_type != node_type => {
                Err(RecordError::TypeMismatch {
                    handle: handle.to_string(),
                    existing: existing.node_type.clone(),
                    requested: node_type.to_string(),
                })
            }
            Some(_) => Ok(()),
            None => {
                self.nodes.insert(
                    handle.to_string(),
                    Node {
                        node_type: node_type.to_string(),
                        ..Node::default()
                    },
                );
                Ok(())
            }
        }
    }

    /// Delete a node and every edge that touches it.
    ///
    /// `shift_remove` rather than `swap_remove`: insertion order is the
    /// replay order and must survive a delete.
    ///
    /// # Errors
    ///
    /// [`RecordError::Reserved`] for `.root` or `.global`. ɴsɪ: "it is
    /// not possible to delete the root or the global node." Deleting
    /// `.root` here would strip every membership edge in the scene.
    pub fn delete(&mut self, handle: &str) -> Result<(), RecordError> {
        if crate::is_reserved(handle) {
            Err(RecordError::Reserved {
                handle: handle.to_string(),
            })
        } else {
            self.nodes.shift_remove(handle);
            self.edges.retain(|e| e.from != handle && e.to != handle);
            Ok(())
        }
    }

    /// Whether `handle` names something a connection may refer to.
    ///
    /// ɴsɪ's `.root` and `.global` are reserved and "don't need to be
    /// created", so they count as known without appearing in `nodes`.
    fn is_known(&self, handle: &str) -> bool {
        handle == crate::ROOT
            || handle == crate::GLOBAL
            || self.nodes.contains_key(handle)
    }

    /// Set static attributes, overwriting by name.
    ///
    /// ɴsɪ: "Setting an attribute using this function replaces any value
    /// previously set by `NSISetAttribute` or `NSISetAttributeAtTime`."
    /// So this also clears every motion sample of the same name --
    /// otherwise a node set static after being sampled would still look
    /// motion-blurred to the resolver.
    pub fn set_attribute(&mut self, handle: &str, args: Vec<OwnedArg>) {
        let node = self.nodes.entry(handle.to_string()).or_default();
        for arg in args {
            for (_, sample) in &mut node.time_attrs {
                sample.shift_remove(&arg.name);
            }
            node.attrs.insert(arg.name.clone(), arg);
        }
        node.time_attrs.retain(|(_, sample)| !sample.is_empty());
    }

    /// Set attributes at one motion sample, keeping samples time-sorted.
    pub fn set_attribute_at_time(
        &mut self,
        handle: &str,
        time: f64,
        args: Vec<OwnedArg>,
    ) {
        let node = self.nodes.entry(handle.to_string()).or_default();

        // `total_cmp`, not `==`. A sample time arrives from a caller
        // and ɴsɪ does not validate it: under `==` a NaN time never
        // matches itself, so every repeat appends another sample and the
        // vector grows without bound, and `-0.0 == 0.0` silently merges
        // two distinct keys. A total order has neither failure.
        let slot = match node
            .time_attrs
            .iter()
            .position(|(t, _)| t.total_cmp(&time) == Ordering::Equal)
        {
            Some(index) => index,
            None => {
                // Insertion sort keeps the samples in `total_cmp` order.
                let index = node
                    .time_attrs
                    .iter()
                    .position(|(t, _)| t.total_cmp(&time) == Ordering::Greater)
                    .unwrap_or(node.time_attrs.len());
                node.time_attrs.insert(index, (time, IndexMap::new()));
                index
            }
        };

        for arg in args {
            // ɴsɪ: setting at a time "replaces any value previously set
            // by NSISetAttribute", so the static value goes.
            node.attrs.shift_remove(&arg.name);
            node.time_attrs[slot].1.insert(arg.name.clone(), arg);
        }
    }

    /// Remove one attribute by name, from static and every time sample.
    /// Silent when absent, as ɴsɪ is.
    pub fn delete_attribute(&mut self, handle: &str, name: &str) {
        if let Some(node) = self.nodes.get_mut(handle) {
            node.attrs.shift_remove(name);
            for (_, attrs) in &mut node.time_attrs {
                attrs.shift_remove(name);
            }
        }
    }

    /// Classify and record a connection at the default priority.
    ///
    /// An unmapped destination propagates rather than being recorded as
    /// a guess.
    pub fn connect(
        &mut self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), RecordError> {
        self.connect_with_args(from, from_attr, to, to_attr, Vec::new())
    }

    /// Classify and record a connection carrying its ɴsɪ arguments.
    ///
    /// ɴsɪ: "It is not an error to create a connection which already
    /// exists." A repeat therefore updates the existing edge's arguments
    /// rather than adding a second one. Recording both would make the
    /// node look like it had two parents, which would fail resolution
    /// for it and everything beneath it.
    pub fn connect_with_args(
        &mut self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Vec<OwnedArg>,
    ) -> Result<(), RecordError> {
        let kind = classify(from_attr, to_attr)?;

        // ɴsɪ: "the nodes on which the connection is performed must
        // exist." `.root` and `.global` are reserved and need no
        // `create`.
        for handle in [from, to] {
            if !self.is_known(handle) {
                return Err(RecordError::UnknownHandle {
                    handle: handle.to_string(),
                });
            }
        }

        match self.edges.iter_mut().find(|edge| {
            edge.from == from && edge.to == to && edge.kind == kind
        }) {
            Some(existing) => existing.args = args,
            None => self.edges.push(Edge {
                from: from.to_string(),
                to: to.to_string(),
                kind,
                args,
            }),
        }

        Ok(())
    }

    /// Remove a connection. Silent when absent, as ɴsɪ is.
    ///
    /// ɴsɪ: "the handle for either node, as well as any or all of the
    /// attributes, may be the special value `.all`. This will remove all
    /// connections which match the other parameters." So each of the
    /// four positions matches everything when it is [`ALL`].
    ///
    /// The connection *arguments* are not part of an edge's identity:
    /// ɴsɪ's disconnect names four things and priority is not one.
    pub fn disconnect(
        &mut self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), RecordError> {
        // `.all` matches every class, so there is nothing to classify.
        let kind = if to_attr == ALL {
            None
        } else {
            Some(classify(from_attr, to_attr)?)
        };

        let from_port = from_attr.unwrap_or_default();
        let any_port = from_port == ALL;

        self.edges.retain(|edge| {
            let port_matches = any_port
                || match &edge.kind {
                    EdgeKind::ShaderNetwork {
                        from_port: port, ..
                    } => port == from_port,
                    _ => from_port.is_empty(),
                };

            let attr_matches = match &kind {
                // A named `to_attr` fixes the class outright, unless the
                // source port is `.all` -- then only the destination
                // attribute is being matched.
                Some(kind) if !any_port => &edge.kind == kind,
                Some(kind) => edge.kind.to_attr() == kind.to_attr(),
                None => true,
            };

            let matches = (from == ALL || edge.from == from)
                && (to == ALL || edge.to == to)
                && port_matches
                && attr_matches;

            !matches
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClassifyError, OwnedData};
    use nsi_trait::Type;

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
        scene.set_attribute("cam", vec![arg("fov", 45.0)]);
        scene.set_attribute("cam", vec![arg("fov", 60.0)]);
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
        scene.set_attribute_at_time("xf", 1.0, vec![arg("t", 1.0)]);
        scene.set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)]);
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
        scene.set_attribute("cam", vec![arg("fov", 45.0), arg("fs", 1.0)]);
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
    fn connect_rejects_an_unmapped_destination() {
        let mut scene = Scene::default();
        scene.create("a", "transform").unwrap();
        scene.create("b", "transform").unwrap();
        let err = scene.connect("a", None, "b", "nonsense").unwrap_err();
        assert_eq!(
            err,
            RecordError::Classify(ClassifyError {
                to_attr: "nonsense".to_string()
            })
        );
        assert!(scene.edges.is_empty());
    }

    /// `delete_attribute` walks the motion samples too. Only the static
    /// path was proven before, and the two are separate tables.
    #[test]
    fn delete_attribute_removes_from_every_time_sample() {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.set_attribute("xf", vec![arg("t", 9.0)]);
        scene.set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)]);
        scene.set_attribute_at_time(
            "xf",
            1.0,
            vec![arg("t", 1.0), arg("keep", 2.0)],
        );

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
    /// unmapped destination cannot be a silent no-op.
    #[test]
    fn disconnect_rejects_an_unmapped_destination() {
        let mut scene = Scene::default();
        let err = scene.disconnect("a", None, "b", "nonsense").unwrap_err();
        assert_eq!(
            err,
            RecordError::Classify(ClassifyError {
                to_attr: "nonsense".to_string()
            })
        );
    }

    /// `priority` is not part of an edge's identity: ɴsɪ's `disconnect`
    /// names four things and priority is not one of them.
    #[test]
    fn disconnect_ignores_priority() {
        let mut scene = Scene::default();
        scene.create("attr", "attributes").unwrap();
        scene.create("m", "mesh").unwrap();
        scene
            .connect_with_args(
                "attr",
                None,
                "m",
                "geometryattributes",
                vec![priority(5)],
            )
            .unwrap();
        scene
            .disconnect("attr", None, "m", "geometryattributes")
            .unwrap();
        assert!(scene.edges.is_empty());
    }

    /// ɴsɪ: `NSISetAttribute` "replaces any value previously set by
    /// NSISetAttribute or NSISetAttributeAtTime". Leaving the samples
    /// behind makes a node that was set static after being sampled look
    /// motion-blurred to the resolver forever.
    #[test]
    fn a_static_set_clears_the_motion_samples_of_that_name() {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)]);
        scene.set_attribute_at_time(
            "xf",
            1.0,
            vec![arg("t", 1.0), arg("keep", 9.0)],
        );

        scene.set_attribute("xf", vec![arg("t", 5.0)]);

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
        scene.set_attribute("xf", vec![arg("t", 5.0), arg("keep", 9.0)]);

        scene.set_attribute_at_time("xf", 0.0, vec![arg("t", 0.0)]);

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
            .connect_with_args(
                "grp",
                None,
                ".root",
                "objects",
                vec![priority(4)],
            )
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
        scene.set_attribute("x", vec![arg("fov", 45.0)]);

        scene.create("x", "mesh").unwrap();

        assert_eq!(scene.nodes["x"].attrs.len(), 1, "attributes survive");
    }

    /// A sample time is caller data and ɴsɪ does not validate it. Under
    /// `==` a NaN never matches itself, so every repeat would append
    /// another sample and the vector would grow without bound.
    #[test]
    fn a_nan_sample_time_matches_itself() {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.set_attribute_at_time("xf", f64::NAN, vec![arg("t", 0.0)]);
        scene.set_attribute_at_time("xf", f64::NAN, vec![arg("t", 1.0)]);

        let samples = &scene.nodes["xf"].time_attrs;
        assert_eq!(samples.len(), 1, "one key, not one per call");
        assert_eq!(samples[0].1["t"].data, OwnedData::F32(vec![1.0]));
    }

    /// `-0.0` and `0.0` are distinct sample keys under a total order,
    /// where `==` would merge them.
    #[test]
    fn negative_zero_is_a_distinct_sample_time() {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.set_attribute_at_time("xf", 0.0, vec![arg("t", 1.0)]);
        scene.set_attribute_at_time("xf", -0.0, vec![arg("t", 2.0)]);

        let times: Vec<f64> = scene.nodes["xf"]
            .time_attrs
            .iter()
            .map(|(t, _)| *t)
            .collect();
        assert_eq!(times.len(), 2);
        assert!(times[0].is_sign_negative(), "-0.0 sorts first");
    }
}
