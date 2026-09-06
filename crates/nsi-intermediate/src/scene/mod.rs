//! Node and attribute tables.
//!
//! `IndexMap` throughout: replaying a scene in a different order than it
//! was recorded would make the `.nsi` stream diff against 3Delight
//! meaningless.

use crate::{ALL, Edge, EdgeKind, OwnedArg, RecordError, classify};
use core::cmp::Ordering;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

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
#[non_exhaustive]
pub struct Scene {
    /// Nodes by handle, in creation order.
    nodes: IndexMap<String, Node>,
    /// Classified connections, in connection order.
    edges: Vec<Edge>,
    /// Edge positions keyed by source handle, and by destination.
    ///
    /// Resolution walks the graph per object, so without these every
    /// hop of every walk scanned every edge -- quadratic in the scene,
    /// which a production asset feels immediately. Rebuilt on removal
    /// (rare) and appended to on `connect` (common).
    by_from: HashMap<String, Vec<usize>>,
    by_to: HashMap<String, Vec<usize>>,
    /// Edge positions keyed by destination *and* destination attribute.
    ///
    /// `by_to` alone is not enough: a transform with twenty thousand
    /// children has twenty thousand incoming `objects` edges, and
    /// gathering attributes there would scan all of them once per
    /// child. Keying on the attribute too makes that lookup
    /// proportional to the matches rather than to the scene.
    by_to_attr: HashMap<(String, String), Vec<usize>>,
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

    /// How many nodes the scene holds.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the scene holds no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The connections *out of* `handle`, in connection order.
    ///
    /// Indexed, so this does not scan the scene.
    pub fn edges_from<'a>(
        &'a self,
        handle: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        self.indexed(&self.by_from, handle)
    }

    /// The connections *into* `handle`, in connection order.
    ///
    /// Indexed, so this does not scan the scene.
    pub fn edges_to<'a>(
        &'a self,
        handle: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        self.indexed(&self.by_to, handle)
    }

    /// The connections into `handle` through `to_attr`, in connection
    /// order.
    ///
    /// Indexed on both, so this is proportional to the matches rather
    /// than to the scene.
    pub fn edges_to_attr<'a>(
        &'a self,
        handle: &str,
        to_attr: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        // `HashMap<(String, String), _>` cannot be probed with a pair of
        // `&str` without allocating, and this is the hot path's key.
        self.by_to_attr
            .get(&(handle.to_string(), to_attr.to_string()))
            .into_iter()
            .flatten()
            .map(|position| &self.edges[*position])
    }

    fn indexed<'a>(
        &'a self,
        index: &'a HashMap<String, Vec<usize>>,
        handle: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        index
            .get(handle)
            .into_iter()
            .flatten()
            .map(|position| &self.edges[*position])
    }

    /// Rebuild both edge indexes.
    ///
    /// Called after a removal, which shifts every later position.
    fn reindex(&mut self) {
        self.by_from.clear();
        self.by_to.clear();
        self.by_to_attr.clear();
        for (position, edge) in self.edges.iter().enumerate() {
            self.by_from
                .entry(edge.from.clone())
                .or_default()
                .push(position);
            self.by_to
                .entry(edge.to.clone())
                .or_default()
                .push(position);
            self.by_to_attr
                .entry((edge.to.clone(), edge.kind.to_attr().to_string()))
                .or_default()
                .push(position);
        }
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
        // ɴsɪ's reserved handles exist already. 3Delight answers a
        // `create` on one with "already exists"; accepting it here kept
        // a node that replay then drops, so the scene changed on its
        // own first round trip.
        if crate::is_reserved(handle) {
            return Err(RecordError::Reserved {
                handle: handle.to_string(),
            });
        }

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
            self.reindex();
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

    /// Delete a node and, recursively, the nodes that only fed it.
    ///
    /// ɴsɪ: "nodes which connect to the specified node are recursively
    /// removed, unless they meet one of the following conditions: they
    /// also have connections which do not eventually lead to the
    /// specified node; their connection to the deleted node was created
    /// with a strength greater than 0." That is what makes deleting a
    /// whole shader network one call.
    ///
    /// # Errors
    ///
    /// [`RecordError::Reserved`] for `.root` or `.global`.
    pub fn delete_recursive(
        &mut self,
        handle: &str,
    ) -> Result<(), RecordError> {
        if crate::is_reserved(handle) {
            return Err(RecordError::Reserved {
                handle: handle.to_string(),
            });
        }

        let mut doomed = HashSet::new();
        doomed.insert(handle.to_string());

        // Grow the set until it stops growing: a node joins when every
        // connection it makes leads into the set, and none of those is
        // strong enough to block.
        loop {
            let candidates = doomed
                .iter()
                .flat_map(|node| self.edges_to(node))
                .filter(|edge| edge.strength() <= 0)
                .map(|edge| edge.from.clone())
                .filter(|from| !doomed.contains(from))
                .collect::<Vec<_>>();

            let mut grew = false;
            for candidate in candidates {
                let leads_elsewhere = self
                    .edges_from(&candidate)
                    .any(|edge| !doomed.contains(&edge.to));

                // The strength rule is about *this* node's connection to
                // anything being deleted, not only about how it was
                // first reached. Checking it at discovery alone let a
                // node be swept in through a second, weak path.
                let held = self.edges_from(&candidate).any(|edge| {
                    doomed.contains(&edge.to) && edge.strength() > 0
                });

                if !leads_elsewhere && !held && doomed.insert(candidate) {
                    grew = true;
                }
            }

            if !grew {
                break;
            }
        }

        self.nodes.retain(|handle, _| !doomed.contains(handle));
        self.edges.retain(|edge| {
            !doomed.contains(&edge.from) && !doomed.contains(&edge.to)
        });
        self.reindex();

        Ok(())
    }

    /// Set static attributes, overwriting by name.
    ///
    /// ɴsɪ: "Setting an attribute using this function replaces any value
    /// previously set by `NSISetAttribute` or `NSISetAttributeAtTime`."
    /// So this also clears every motion sample of the same name --
    /// otherwise a node set static after being sampled would still look
    /// motion-blurred to the resolver.
    ///
    /// # Errors
    ///
    /// [`RecordError::UnknownHandle`] when the node does not exist.
    /// 3Delight answers the same call with "unknown node handle". A
    /// fabricated node is worse than a rejected one: it joins the scene,
    /// satisfies later `connect` calls, and replays as a `Create` the
    /// renderer never wrote.
    pub fn set_attribute(
        &mut self,
        handle: &str,
        args: Vec<OwnedArg>,
    ) -> Result<(), RecordError> {
        let node = self.node_mut(handle)?;
        for arg in args {
            for (_, sample) in &mut node.time_attrs {
                sample.shift_remove(&arg.name);
            }
            node.attrs.insert(arg.name.clone(), arg);
        }
        node.time_attrs.retain(|(_, sample)| !sample.is_empty());
        Ok(())
    }

    /// A node to mutate, or [`RecordError::UnknownHandle`].
    ///
    /// ɴsɪ's reserved handles are created on demand: they "don't need to
    /// be created using NSICreate", but they do carry attributes.
    fn node_mut(&mut self, handle: &str) -> Result<&mut Node, RecordError> {
        if crate::is_reserved(handle) {
            Ok(self.nodes.entry(handle.to_string()).or_default())
        } else {
            self.nodes.get_mut(handle).ok_or_else(|| {
                RecordError::UnknownHandle {
                    handle: handle.to_string(),
                }
            })
        }
    }

    /// Set attributes at one motion sample, keeping samples time-sorted.
    ///
    /// # Errors
    ///
    /// [`RecordError::UnknownHandle`] when the node does not exist.
    pub fn set_attribute_at_time(
        &mut self,
        handle: &str,
        time: f64,
        args: Vec<OwnedArg>,
    ) -> Result<(), RecordError> {
        // 3Delight answers a non-finite time with `E6026 invalid time`.
        if !time.is_finite() {
            return Err(RecordError::InvalidTime {
                handle: handle.to_string(),
            });
        }

        // `-0.0` and `0.0` are one sample to the renderer, which reads a
        // `-0` time as `+0`. Keeping them apart handed a backend two
        // matrices at times that compare equal -- a zero-length motion
        // segment.
        let time = time + 0.0;

        let node = self.node_mut(handle)?;

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

        Ok(())
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
        let kind = classify(from_attr, to_attr);

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
            None => {
                self.by_from
                    .entry(from.to_string())
                    .or_default()
                    .push(self.edges.len());
                self.by_to
                    .entry(to.to_string())
                    .or_default()
                    .push(self.edges.len());
                self.by_to_attr
                    .entry((to.to_string(), kind.to_attr().to_string()))
                    .or_default()
                    .push(self.edges.len());
                self.edges.push(Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    kind,
                    args,
                });
            }
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
            Some(classify(from_attr, to_attr))
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
        self.reindex();

        Ok(())
    }
}

#[cfg(test)]
mod tests;
