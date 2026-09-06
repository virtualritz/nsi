//! Node and attribute tables.
//!
//! `IndexMap` throughout: replaying a scene in a different order than it
//! was recorded would make the `.nsi` stream diff against 3Delight
//! meaningless.

use crate::{ALL, Edge, EdgeKind, OwnedArg, RecordError, classify};
use core::cmp::Ordering;
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

/// One ɴsɪ node.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Node {
    /// The ɴsɪ node type this handle was created with.
    pub node_type: String,
    /// Attributes set with `set_attribute`, keyed by name.
    pub attrs: IndexMap<String, OwnedArg>,
    /// Every `set_attribute_at_time` call, per attribute, **in call
    /// order**.
    ///
    /// A call log rather than a timeline, because that is what ɴsɪ's
    /// rules are stated over. 3Delight applies the last *call*, and a
    /// table keyed by time cannot say which call that was: rendered,
    /// with `visibility` set only at times, `t=1 -> 0` then `t=0 -> 1`
    /// leaves the object **visible** while `t=1 -> 1` then `t=0 -> 0`
    /// hides it. The same two times, opposite answers.
    ///
    /// The order is equally what decides the reach of an unreadable
    /// sample: 3Delight rejects the argument at the call, so what
    /// survives is what was set *after* it, not what sits later on the
    /// timeline. And a same-time re-set is a call of its own here --
    /// keeping the superseded value is the difference between `good`
    /// replacing `good`, which sweeps, and `good` replacing an
    /// unreadable one, which does not.
    ///
    /// Read it through [`Node::effective`] or the resolver rather than
    /// walking it: they apply that rule. Setting an attribute
    /// statically clears its entry, as ɴsɪ says it should.
    ///
    /// One entry per call, so a caller that re-sets one time in a loop
    /// grows it. That is the record ɴsɪ's rules need, and for any
    /// scene an exporter writes -- each time set once -- it is the
    /// same values a table keyed by time would hold.
    pub samples: IndexMap<String, Vec<(f64, OwnedArg)>>,
}

impl Node {
    /// This node's effective value for an attribute.
    ///
    /// **Use this, not `attrs`.** `SetAttributeAtTime` on an attribute
    /// that is not motion data sets it for the whole shutter, exactly
    /// as `SetAttribute` would: rendered, an `attributes` node whose
    /// `visibility` is set only through `SetAttributeAtTime` hides the
    /// object, identically to the static form. Reading [`Node::attrs`]
    /// alone answers "not set" for an attribute the renderer honours,
    /// which is a silent wrong answer -- and was one here until it was
    /// rendered.
    ///
    /// Static first, then the **last call**, which is not the sample
    /// at the greatest time -- see [`Node::samples`] for the render
    /// that separates the two. The static value and the samples never
    /// coexist: `set_attribute` clears that name from the log and
    /// `set_attribute_at_time` clears the static value, which is ɴsɪ's
    /// own rule and 3Delight's behaviour.
    ///
    /// This is what the resolver reads, so a backend asking a node
    /// directly gets the same answer the resolver would.
    pub fn effective(&self, name: &str) -> Option<&OwnedArg> {
        if let Some(arg) = self.attrs.get(name) {
            return Some(arg);
        }
        self.samples.get(name)?.last().map(|(_, arg)| arg)
    }
}

/// The value standing at each time, once a same-time re-set has
/// replaced what it superseded, ascending by time.
///
/// One statement of it, read by [`Scene::attribute_samples`] and by
/// the resolver's typing rule -- which runs it over the calls that
/// survived an unreadable one rather than over all of them. Two copies
/// of a resolution rule have drifted apart in this crate four times.
pub(crate) fn latest_per_time(
    calls: &[(f64, OwnedArg)],
) -> Vec<(f64, &OwnedArg)> {
    // Sorted by time, and by **call order** within one time -- the
    // call index is part of the key rather than a stability the sort
    // happens to give. A reviewer switched this to `sort_unstable_by`
    // and nothing went red; no fixture had enough same-time calls to
    // make an unstable sort actually reorder, so the guarantee the
    // rule rested on was one no test could see. With the index in the
    // key the order is total and any correct sort gives this answer.
    let mut standing: Vec<(f64, usize, &OwnedArg)> = calls
        .iter()
        .enumerate()
        .map(|(index, (time, arg))| (*time, index, arg))
        .collect();
    standing.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    standing.dedup_by(|later, earlier| {
        // `dedup_by` keeps the *earlier* of a matching pair, and the
        // later call is the one that stands, so its value moves down
        // before the earlier one is dropped.
        if later.0.total_cmp(&earlier.0) == Ordering::Equal {
            earlier.2 = later.2;
            true
        } else {
            false
        }
    });

    standing
        .into_iter()
        .map(|(time, _, arg)| (time, arg))
        .collect()
}

/// What changed since the last [`Scene::take_changes`].
///
/// A **net** record, not a log of calls: a handle created and deleted
/// in one interval nets to nothing, and re-setting one attribute forty
/// times is one entry. A consumer synchronising a live renderer wants
/// the set of things to look at again, and coalescing a call log into
/// that set would be its work rather than ours.
///
/// It carries **no values**. The scene holds the current one --
/// [`Node::effective`] answers it -- so an entry names an attribute
/// rather than copying a vertex buffer per edit.
///
/// This is the raw record, and it is deliberately in ɴsɪ's domain: a
/// `transform` and an `attributes` node have no counterpart in a
/// renderer's scene, and one geometry under two parents is several
/// objects there. Turning "this transform moved" into "these placements
/// may have moved" is a walk *down* the graph, and it is the next thing
/// to build here rather than in every backend.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Changes {
    /// Handles created, and still present.
    pub created: IndexSet<String>,
    /// Handles deleted, with the node type they had.
    ///
    /// The type is kept because the handle is gone from the scene: a
    /// consumer that has to undo whatever it built for a node cannot
    /// ask what kind of node it was any more.
    pub deleted: IndexMap<String, String>,
    /// `(handle, attribute)` pairs set, re-set or deleted.
    pub attributes: IndexSet<(String, String)>,
    /// Connections made.
    pub edges_added: Vec<Edge>,
    /// Connections removed, **in full**.
    ///
    /// The endpoints and kind are kept because the graph no longer has
    /// them: working out what a severed `objects` edge orphaned means
    /// walking down from a node the edge no longer points at. A
    /// `disconnect` naming `.all` is expanded here into the edges it
    /// actually removed, since the pattern cannot be re-expanded once
    /// they are gone.
    pub edges_removed: Vec<Edge>,
    /// Connections whose arguments a repeated `connect` replaced in
    /// place.
    ///
    /// No edge appeared or disappeared, so a record keyed on additions
    /// and removals misses this entirely -- and ɴsɪ's `"priority"`
    /// rides on these arguments, which decides which of two shaders
    /// wins. This is the quietest way for a scene to change meaning.
    pub edges_rearmed: Vec<Edge>,
}

/// What a [`Changes`] batch may have moved.
///
/// **Candidates, not a minimal set.** An entry means "resolve this
/// again", not "this definitely differs": ɴsɪ's precedence rules can
/// make an edit invisible -- a re-armed `surfaceshader` at a lower
/// priority than a nearer one changes nothing -- and deciding that
/// here would mean remembering every previous answer this crate ever
/// gave, which is the consumer's own record to keep.
///
/// Keyed by **handle**, not by placement path. One geometry under two
/// parents is several objects to a renderer, and editing one parent
/// moves only one of them; a handle-level answer makes the consumer
/// re-resolve both. Correct, and coarse for a crowd -- path-precision
/// is an optimisation to ask for with a measurement.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Affected {
    /// Geometry, transforms, instancers, cameras and lights whose
    /// resolved answers may have moved.
    pub nodes: IndexSet<String>,
    /// Shader nodes whose own attributes or network changed.
    ///
    /// Kept apart because they map one-to-one onto a renderer's
    /// material parameters and cost no geometry work: only the *root*
    /// shader's identity reaches geometry, and that is a
    /// `surfaceshader` edge, which lands in `nodes`.
    pub shaders: IndexSet<String>,
    /// Whether the camera/screen/layer/driver chain changed, so the
    /// outputs need re-reading.
    pub outputs: bool,
    /// Whether something global changed -- an attribute on `.root` or
    /// `.global` -- and the whole scene is a candidate.
    ///
    /// When this is set, `nodes` is not filled: the answer is
    /// everything, and listing it would be a copy of the scene.
    pub everything: bool,
}

/// The recorded scene graph.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Scene {
    /// Nodes by handle, in creation order.
    nodes: IndexMap<String, Node>,
    /// Classified connections, in connection order.
    edges: Vec<Edge>,
    /// `Evaluate` calls, in call order, each with the node count at the
    /// time so replay can put them back where they were.
    ///
    /// ɴsɪ's `Evaluate` "includes a block of interface calls from an
    /// external source" -- an archive, a Lua script or a compiled
    /// procedural. This crate does not execute one, so whatever it
    /// would have produced is absent from the scene; recording the call
    /// at least means a stream carrying one is not silently reduced to
    /// a scene missing its geometry, with no error and no trace.
    evaluations: Vec<Vec<OwnedArg>>,
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
    /// What has changed since the last [`Scene::take_changes`].
    ///
    /// Private, and not part of [`Scene`]'s equality: two scenes with
    /// the same nodes and edges are the same scene whether or not one
    /// of them has been synchronised since.
    changes: Changes,
}

/// Two scenes are equal when they describe the same thing.
///
/// Written out rather than derived because of what it must *exclude*:
/// the pending [`Changes`], which say what happened to a scene rather
/// than what it is -- a scene that has just been synchronised would
/// otherwise stop being equal to the identical scene that has not --
/// and the three edge indexes, which are a function of the edges they
/// index.
impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.nodes == other.nodes
            && self.edges == other.edges
            && self.evaluations == other.evaluations
    }
}

impl Scene {
    /// What has changed since this was last called, clearing the
    /// record.
    ///
    /// ɴsɪ's `NSIRenderControl "synchronize"` is where a host asks a
    /// renderer to catch up: the specification says "apply all the
    /// **buffered** calls to scene's state", so the interval between
    /// two synchronises is exactly one batch of edits. Observed on
    /// 3Delight: edits made without synchronising change nothing and
    /// report nothing; the synchronise then reports `Restarted`
    /// followed by `Synchronized`, once, for the whole batch.
    ///
    /// Taking clears, because the alternative is a record that grows
    /// for the life of an interactive session and a caller that must
    /// remember where it read up to.
    ///
    /// What a caller does with it today is walk down from the named
    /// handles itself, with [`Scene::edges_to_attr`] and the
    /// resolution accessors. That walk belongs here and is the next
    /// step; this is the record it needs, and the record is the half
    /// that cannot be reconstructed after the fact.
    pub fn take_changes(&mut self) -> Changes {
        core::mem::take(&mut self.changes)
    }

    /// What has changed, without clearing it.
    pub fn changes(&self) -> &Changes {
        &self.changes
    }

    /// The nodes a [`Changes`] batch may have moved.
    ///
    /// Every rule here is the **inverse** of a walk this crate already
    /// does upward. Resolution asks "what applies to this geometry" by
    /// climbing `objects` to `.root` and gathering containers on the
    /// way; a synchronise asks the same question backwards -- "given
    /// this node changed, whose answers depended on it" -- so the
    /// indexes that make the climb cheap (`by_to_attr`) make the
    /// descent cheap too, and no new index is needed.
    ///
    /// The rules, and what each inverts:
    ///
    /// - a `transform`'s attribute, or an `objects` edge: everything
    ///   below it, and any instancer drawing a prototype found there.
    ///   Inverts the chain walk. Cameras and lights hang off `objects`
    ///   like geometry, so they come along.
    /// - an `attributes` node's attribute, or a shader edge into one:
    ///   everything below each node it is bound to. Inverts the
    ///   container gather. A binding onto a `set` reaches the set's
    ///   members.
    /// - a shader's own attribute, or a shader-network edge: the
    ///   shader alone. Nothing about geometry changed.
    /// - `.root` or `.global`: everything.
    ///
    /// Over-approximate on purpose -- see [`Affected`].
    pub fn affected(&self, changes: &Changes) -> Affected {
        let mut affected = Affected::default();

        for (handle, _) in &changes.attributes {
            if crate::is_reserved(handle) {
                affected.everything = true;
                continue;
            }
            match self.nodes.get(handle) {
                Some(node) if node.node_type == "shader" => {
                    affected.shaders.insert(handle.clone());
                }
                Some(node) if node.node_type == "attributes" => {
                    self.through_bindings(handle, &mut affected.nodes);
                }
                // Anything else is a scene node: it may be the thing
                // that moved, and it may be a transform with a subtree
                // under it. The descent answers both.
                _ => self.descend(handle, &mut affected.nodes),
            }
            if self.is_output_node(handle) {
                affected.outputs = true;
            }
        }

        for handle in &changes.created {
            self.descend(handle, &mut affected.nodes);
        }

        // A deleted handle is gone from the graph, so there is nothing
        // left to descend *from*: what it orphaned is reached through
        // the edges the delete took with it, which is why they are
        // recorded in full.
        for handle in changes.deleted.keys() {
            affected.nodes.insert(handle.clone());
        }

        for edge in changes
            .edges_added
            .iter()
            .chain(&changes.edges_removed)
            .chain(&changes.edges_rearmed)
        {
            match edge.kind.to_attr() {
                // A child, a set member or a prototype: whatever hung
                // below the source now hangs somewhere else.
                "objects" | "members" | "sourcemodels" => {
                    self.descend(&edge.from, &mut affected.nodes);
                    if self.is_output_node(&edge.from)
                        || self.is_output_node(&edge.to)
                    {
                        affected.outputs = true;
                    }
                }
                // A container bound to something, or unbound from it.
                "geometryattributes" | "shaderattributes" => {
                    self.descend(&edge.to, &mut affected.nodes);
                    self.set_members(&edge.to, &mut affected.nodes);
                }
                // A shader reaching an `attributes` node -- including a
                // repeated `connect` that only changed `"priority"`,
                // which is why re-armed edges are here.
                "surfaceshader" | "displacementshader" | "volumeshader"
                | "lightset" | "exclusiveshading" => {
                    affected.shaders.insert(edge.from.clone());
                    self.through_bindings(&edge.to, &mut affected.nodes);
                }
                "screens" | "outputlayers" | "outputdrivers" => {
                    affected.outputs = true;
                }
                // A shader-network edge, or a class this crate carries
                // without resolving: both ends, and nothing below.
                _ => {
                    affected.shaders.insert(edge.from.clone());
                    affected.shaders.insert(edge.to.clone());
                }
            }
        }

        if affected.everything {
            affected.nodes.clear();
        }

        affected
    }

    /// Everything at or below `handle` on the `objects` chain, plus any
    /// instancer that draws a prototype found there.
    ///
    /// Iterative with an explicit stack: an ɴsɪ scene's depth is the
    /// caller's, not ours, and a recursive walk here would overflow on
    /// a deep chain. The `insert` doubles as the visited set, so a
    /// cycle terminates.
    fn descend(&self, handle: &str, out: &mut IndexSet<String>) {
        let mut stack = vec![handle.to_string()];
        while let Some(node) = stack.pop() {
            if !out.insert(node.clone()) {
                continue;
            }
            for edge in self.edges_to_attr(&node, "objects") {
                stack.push(edge.from.clone());
            }
            // A prototype's mover moves every instancer drawing it, and
            // the instancer is not below the transform that moved --
            // it is reached the other way, through `sourcemodels`.
            for edge in self.edges_from(&node) {
                if edge.kind.to_attr() == "sourcemodels" {
                    stack.push(edge.to.clone());
                }
            }
        }
    }

    /// Everything an `attributes` node is bound to, and below that.
    fn through_bindings(&self, handle: &str, out: &mut IndexSet<String>) {
        for edge in self.edges_from(handle) {
            if matches!(
                edge.kind.to_attr(),
                "geometryattributes" | "shaderattributes"
            ) {
                self.descend(&edge.to, out);
                self.set_members(&edge.to, out);
            }
        }
    }

    /// If `handle` is a `set`, everything its members carry.
    ///
    /// One hop, not a recursion: a `set` inside a `set` contributes
    /// nothing to what a geometry inherits -- rendered, and pinned by
    /// `a_nested_sets_attributes_are_not_inherited` -- so descending
    /// through nested sets would name nodes the renderer never reaches.
    fn set_members(&self, handle: &str, out: &mut IndexSet<String>) {
        for edge in self.edges_to_attr(handle, "members") {
            self.descend(&edge.from, out);
        }
    }

    /// Whether this handle is part of the camera/screen/layer/driver
    /// chain, whose answers come from [`Scene::render_outputs`] rather
    /// than from a geometry walk.
    fn is_output_node(&self, handle: &str) -> bool {
        self.nodes.get(handle).is_some_and(|node| {
            node.node_type.ends_with("camera")
                || matches!(
                    node.node_type.as_str(),
                    "screen" | "outputlayer" | "outputdriver"
                )
        })
    }

    /// The nodes, by handle, in creation order.
    pub fn nodes(&self) -> impl Iterator<Item = (&String, &Node)> {
        self.nodes.iter()
    }

    /// The recorded `Evaluate` calls, in call order.
    ///
    /// Each is the argument list as given. A backend that wants
    /// archives or procedurals has to execute them itself: this crate
    /// records the call and does not define an execution model for it.
    pub fn evaluations(&self) -> impl Iterator<Item = &[OwnedArg]> {
        self.evaluations.iter().map(Vec::as_slice)
    }

    /// Record an `Evaluate`.
    ///
    /// Only the call, not where it fell among the nodes: replay emits
    /// every `Evaluate` first, nothing reads a position, and a node
    /// count kept here would be wrong the moment a `delete` shifted
    /// it.
    pub(crate) fn evaluate(&mut self, args: Vec<OwnedArg>) {
        self.evaluations.push(args);
    }

    /// One node by handle.
    pub fn node(&self, handle: &str) -> Option<&Node> {
        self.nodes.get(handle)
    }

    /// A node together with the scene's own copy of its handle.
    ///
    /// Resolution returns borrowed handles that outlive the `&str` a
    /// caller passed in, so it needs the stored key, not the argument.
    pub(crate) fn node_entry(&self, handle: &str) -> Option<(&String, &Node)> {
        self.nodes.get_key_value(handle)
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
                self.changes.created.insert(handle.to_string());
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
            // Recorded *before* the removal: afterwards the type is
            // gone and the edges that named this handle cannot be
            // found, and a consumer working out what the delete
            // orphaned needs both.
            if let Some(node) = self.nodes.shift_remove(handle) {
                self.changes
                    .deleted
                    .insert(handle.to_string(), node.node_type);
            }
            self.changes.edges_removed.extend(
                self.edges
                    .iter()
                    .filter(|e| e.from == handle || e.to == handle)
                    .cloned(),
            );
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

        for handle in &doomed {
            if let Some(node) = self.nodes.get(handle) {
                self.changes
                    .deleted
                    .insert(handle.clone(), node.node_type.clone());
            }
        }
        self.changes.edges_removed.extend(
            self.edges
                .iter()
                .filter(|edge| {
                    doomed.contains(&edge.from) || doomed.contains(&edge.to)
                })
                .cloned(),
        );

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
        let mut touched = Vec::with_capacity(args.len());
        for arg in args {
            node.samples.shift_remove(&arg.name);
            touched.push(arg.name.clone());
            node.attrs.insert(arg.name.clone(), arg);
        }
        for name in touched {
            self.changes.attributes.insert((handle.to_string(), name));
        }
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

        let mut touched = Vec::with_capacity(args.len());
        for arg in args {
            // ɴsɪ: setting at a time "replaces any value previously set
            // by NSISetAttribute", so the static value goes.
            node.attrs.shift_remove(&arg.name);
            touched.push(arg.name.clone());

            // Appended, never merged: a re-set at a time already
            // recorded is another call, and what it superseded is part
            // of the record. `Node::samples` says why.
            node.samples
                .entry(arg.name.clone())
                .or_default()
                .push((time, arg));
        }

        for name in touched {
            self.changes.attributes.insert((handle.to_string(), name));
        }

        Ok(())
    }

    /// Remove one attribute by name, from static and every time sample.
    /// Silent when absent, as ɴsɪ is.
    pub fn delete_attribute(&mut self, handle: &str, name: &str) {
        // Recorded whether or not it was set: ɴsɪ is silent about
        // deleting an absent attribute, and a consumer asking "what
        // should I look at again" is not harmed by one extra name.
        self.changes
            .attributes
            .insert((handle.to_string(), name.to_string()));
        if let Some(node) = self.nodes.get_mut(handle) {
            node.attrs.shift_remove(name);
            node.samples.shift_remove(name);
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

        let mut rearmed = None;
        match self.edges.iter_mut().find(|edge| {
            edge.from == from && edge.to == to && edge.kind == kind
        }) {
            Some(existing) => {
                // No edge appears or disappears here, and ɴsɪ's
                // `"priority"` rides on these arguments -- so a record
                // keyed on additions and removals would miss a scene
                // changing which shader wins.
                existing.args = args;
                rearmed = Some(existing.clone());
            }
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
                let edge = Edge {
                    from: from.to_string(),
                    to: to.to_string(),
                    kind,
                    args,
                };
                self.changes.edges_added.push(edge.clone());
                self.edges.push(edge);
            }
        }

        if let Some(edge) = rearmed {
            self.changes.edges_rearmed.push(edge);
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

        let mut removed = Vec::new();
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

            if matches {
                // Kept in full, because a `.all` pattern cannot be
                // re-expanded once the edges it named are gone.
                removed.push(edge.clone());
            }

            !matches
        });
        self.changes.edges_removed.extend(removed);
        self.reindex();

        Ok(())
    }
}

#[cfg(test)]
mod tests;
