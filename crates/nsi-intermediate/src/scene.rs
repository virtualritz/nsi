//! Node and attribute tables.
//!
//! `IndexMap` throughout: replaying a scene in a different order than it
//! was recorded would make the `.nsi` stream diff against 3Delight
//! meaningless.

use crate::{ClassifyError, Edge, OwnedArg, classify};
use indexmap::IndexMap;

/// One ɴsɪ node.
#[derive(Debug, Clone, Default)]
pub struct Node {
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
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub nodes: IndexMap<String, Node>,
    pub edges: Vec<Edge>,
}

impl Scene {
    /// Create a node.
    ///
    /// Re-creating an existing handle updates its type, matching ɴsɪ's
    /// tolerance of a repeated `create` with identical parameters.
    pub fn create(&mut self, handle: &str, node_type: &str) {
        let node = self.nodes.entry(handle.to_string()).or_default();
        node.node_type = node_type.to_string();
    }

    /// Delete a node and every edge that touches it.
    ///
    /// `shift_remove` rather than `swap_remove`: insertion order is the
    /// replay order and must survive a delete.
    pub fn delete(&mut self, handle: &str) {
        self.nodes.shift_remove(handle);
        self.edges.retain(|e| e.from != handle && e.to != handle);
    }

    /// Set static attributes, overwriting by name.
    pub fn set_attribute(&mut self, handle: &str, args: Vec<OwnedArg>) {
        let node = self.nodes.entry(handle.to_string()).or_default();
        for arg in args {
            node.attrs.insert(arg.name.clone(), arg);
        }
    }

    /// Set attributes at one motion sample, keeping samples time-sorted.
    pub fn set_attribute_at_time(
        &mut self,
        handle: &str,
        time: f64,
        args: Vec<OwnedArg>,
    ) {
        let node = self.nodes.entry(handle.to_string()).or_default();

        let slot = match node.time_attrs.iter().position(|(t, _)| *t == time) {
            Some(index) => index,
            None => {
                // Insertion sort keeps samples ordered without needing a
                // total order on f64.
                let index = node
                    .time_attrs
                    .iter()
                    .position(|(t, _)| *t > time)
                    .unwrap_or(node.time_attrs.len());
                node.time_attrs.insert(index, (time, IndexMap::new()));
                index
            }
        };

        for arg in args {
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

    /// Classify and record a connection.
    ///
    /// An unmapped destination propagates rather than being recorded as
    /// a guess.
    pub fn connect(
        &mut self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), ClassifyError> {
        let kind = classify(from_attr, to_attr)?;
        self.edges.push(Edge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
        Ok(())
    }

    /// Remove a connection. Silent when absent, as ɴsɪ is.
    pub fn disconnect(
        &mut self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), ClassifyError> {
        let kind = classify(from_attr, to_attr)?;
        self.edges
            .retain(|e| !(e.from == from && e.to == to && e.kind == kind));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OwnedData;
    use nsi_trait::Type;

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
        scene.create("cam", "perspectivecamera");
        assert_eq!(scene.nodes["cam"].node_type, "perspectivecamera");
    }

    #[test]
    fn set_attribute_overwrites_by_name() {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
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
        scene.create("xf", "transform");
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
        scene.create("xf", "transform");
        scene.create("mesh", "mesh");
        scene
            .connect("mesh", None, "xf", "objects")
            .expect("known attribute");
        scene.delete("xf");
        assert!(!scene.nodes.contains_key("xf"));
        assert!(scene.edges.is_empty());
    }

    #[test]
    fn delete_attribute_removes_one_key() {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
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
            scene.create(handle, "transform");
        }
        let order: Vec<&str> = scene.nodes.keys().map(String::as_str).collect();
        assert_eq!(order, vec!["z", "a", "m"]);
    }

    /// An unmapped destination must propagate, not be swallowed.
    #[test]
    fn connect_rejects_an_unmapped_destination() {
        let mut scene = Scene::default();
        let err = scene.connect("a", None, "b", "nonsense").unwrap_err();
        assert_eq!(err.to_attr, "nonsense");
        assert!(scene.edges.is_empty());
    }
}
