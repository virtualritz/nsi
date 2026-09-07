//! Node and attribute tables.
//!
//! `IndexMap` throughout: replaying a scene in a different order than it
//! was recorded would make the `.nsi` stream diff against 3Delight
//! meaningless.

use crate::{
    ALL, Edge, EdgeKind, HashMap, HashSet, OwnedArgument, RecordError,
    classify,
    handle::{self, Handle},
};
use core::{cmp::Ordering, mem};
use indexmap::{IndexMap, IndexSet};
use std::sync::LazyLock;

/// One node's static attributes, by name.
pub(crate) type AttributeTable = IndexMap<Handle, OwnedArgument>;

/// Every `set_attribute_at_time` call for one node, per attribute, in
/// call order.
pub(crate) type SampleTable = IndexMap<Handle, Vec<(f64, OwnedArgument)>>;

/// One ɴsɪ node.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Node {
    /// The ɴsɪ node type this handle was created with. Read it with
    /// [`Node::node_type`].
    ///
    /// Stored rather than public so `ustr_handles` can intern it: a
    /// production scene says `"mesh"` a million times.
    pub(crate) node_type: Handle,
    /// Attributes set with `set_attribute`, keyed by name.
    ///
    /// Boxed and absent until one is set, for the same reason
    /// [`Node::samples`] is: a scene is mostly nodes that hold
    /// connections rather than values -- sets, groups, the transforms
    /// above an asset -- and an inline `IndexMap` cost every one of
    /// them a header it never filled.
    pub(crate) attributes: Option<Box<AttributeTable>>,
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
    ///
    /// Boxed and absent until something is set at a time: most nodes
    /// in a scene never move, and an inline `IndexMap` cost every one
    /// of them the header whether or not it held anything.
    pub(crate) samples: Option<Box<SampleTable>>,
}

/// An empty table, for a node that has no attributes.
static NO_ATTRIBUTES: LazyLock<AttributeTable> = LazyLock::new(IndexMap::new);

/// An empty table, for a node that has no samples.
static NO_SAMPLES: LazyLock<SampleTable> = LazyLock::new(IndexMap::new);

/// Two nodes are equal when they say the same thing, which an absent
/// table and an emptied one both do -- `delete_attribute` leaves the
/// second, and a scene compared against a replay of itself must not
/// hinge on which one it holds.
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.node_type == other.node_type
            && self.attribute_table() == other.attribute_table()
            && self.sample_table() == other.sample_table()
    }
}

impl Node {
    /// This node's static attributes, by name.
    pub fn attributes(&self) -> impl Iterator<Item = (&str, &OwnedArgument)> {
        self.attribute_table()
            .iter()
            .map(|(name, arg)| (name.as_str(), arg))
    }

    /// One static attribute by name.
    ///
    /// **Use [`Node::effective`] unless you mean the static one
    /// specifically**: an attribute set with `SetAttributeAtTime` is
    /// not here, and the renderer honours it.
    pub fn attribute(&self, name: &str) -> Option<&OwnedArgument> {
        handle::map_get(self.attribute_table(), name)
    }

    pub(crate) fn attribute_table(&self) -> &AttributeTable {
        self.attributes.as_deref().unwrap_or(&NO_ATTRIBUTES)
    }

    pub(crate) fn attribute_table_mut(&mut self) -> &mut AttributeTable {
        self.attributes.get_or_insert_with(Box::default)
    }

    /// This node's sampled attributes, by name, each in call order.
    pub fn samples(
        &self,
    ) -> impl Iterator<Item = (&str, &[(f64, OwnedArgument)])> {
        self.sample_table()
            .iter()
            .map(|(name, calls)| (name.as_str(), calls.as_slice()))
    }

    /// The calls that set one attribute at a time, in call order.
    pub fn sample_calls(&self, name: &str) -> Option<&[(f64, OwnedArgument)]> {
        handle::map_get(self.sample_table(), name).map(Vec::as_slice)
    }

    pub(crate) fn sample_table(&self) -> &SampleTable {
        self.samples.as_deref().unwrap_or(&NO_SAMPLES)
    }

    pub(crate) fn sample_table_mut(&mut self) -> &mut SampleTable {
        self.samples.get_or_insert_with(Box::default)
    }

    /// The ɴsɪ node type this handle was created with.
    pub fn node_type(&self) -> &str {
        &self.node_type
    }

    /// This node's effective value for an attribute.
    ///
    /// **Use this, not [`Node::attributes`].** `SetAttributeAtTime` on an
    /// attribute
    /// that is not motion data sets it for the whole shutter, exactly
    /// as `SetAttribute` would: rendered, an `attributes` node whose
    /// `visibility` is set only through `SetAttributeAtTime` hides the
    /// object, identically to the static form. Reading [`Node::attributes`]
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
    pub fn effective(&self, name: &str) -> Option<&OwnedArgument> {
        if let Some(arg) = handle::map_get(self.attribute_table(), name) {
            return Some(arg);
        }
        handle::map_get(self.sample_table(), name)?
            .last()
            .map(|(_, arg)| arg)
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
    calls: &[(f64, OwnedArgument)],
) -> Vec<(f64, &OwnedArgument)> {
    // Sorted by time, and by **call order** within one time -- the
    // call index is part of the key rather than a stability the sort
    // happens to give. A reviewer switched this to `sort_unstable_by`
    // and nothing went red; no fixture had enough same-time calls to
    // make an unstable sort actually reorder, so the guarantee the
    // rule rested on was one no test could see. With the index in the
    // key the order is total and any correct sort gives this answer.
    let mut standing: Vec<(f64, usize, &OwnedArgument)> = calls
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
/// A **net** record, not a log of calls: re-setting one attribute
/// forty times is one entry, connecting and reconnecting one edge is
/// one entry, and a handle created and then deleted in the same
/// interval appears only in [`Changes::deleted`] -- the create is
/// undone by the delete, and a consumer that never saw the node has
/// nothing to undo. A consumer synchronising a live renderer wants the
/// set of things to look at again, and coalescing a call log into that
/// set would be its work rather than ours.
///
/// It carries **no values**. The scene holds the current one --
/// [`Node::effective`] answers it -- so an entry names an attribute
/// rather than copying a vertex buffer per edit.
///
/// This is the raw record, and it is deliberately in ɴsɪ's domain: a
/// `transform` and an `attributes` node have no counterpart in a
/// renderer's scene, and one geometry under two parents is several
/// objects there. [`Scene::affected`] turns "this transform moved"
/// into "these nodes may have moved", which is the question a backend
/// actually asks.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Changes {
    /// Handles created during this interval and not since deleted.
    ///
    /// Read it with [`Changes::created`]. Stored as handles so that
    /// `ustr_handles` interns them: a frame that touches ten thousand
    /// nodes copied ten thousand strings out of the scene otherwise,
    /// on the one path an interactive host walks every frame.
    pub(crate) created: IndexSet<Handle>,
    /// Handles deleted, with the node type they had.
    ///
    /// The type is kept because the handle is gone from the scene: a
    /// consumer that has to undo whatever it built for a node cannot
    /// ask what kind of node it was any more. Read it with
    /// [`Changes::deleted`].
    pub(crate) deleted: IndexMap<Handle, Handle>,
    /// `(handle, attribute)` pairs set, re-set or deleted. Read it
    /// with [`Changes::attributes`].
    pub(crate) attributes: IndexSet<(Handle, Handle)>,
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

impl Changes {
    /// The handles created and not since deleted.
    pub fn created(&self) -> impl Iterator<Item = &str> {
        self.created.iter().map(|handle| handle.as_str())
    }

    /// Whether one handle was created in this interval.
    pub fn was_created(&self, handle: &str) -> bool {
        handle::set_contains(&self.created, handle)
    }

    /// The handles deleted, each with the node type it had.
    pub fn deleted(&self) -> impl Iterator<Item = (&str, &str)> {
        self.deleted
            .iter()
            .map(|(handle, node_type)| (handle.as_str(), node_type.as_str()))
    }

    /// The node type a deleted handle had, if it was deleted here.
    pub fn deleted_type(&self, handle: &str) -> Option<&str> {
        handle::map_get(&self.deleted, handle)
            .map(|node_type| node_type.as_str())
    }

    /// The `(handle, attribute)` pairs set, re-set or deleted.
    pub fn attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(handle, name)| (handle.as_str(), name.as_str()))
    }

    /// Nothing changed since the last [`Scene::take_changes`].
    pub fn is_empty(&self) -> bool {
        self.created.is_empty()
            && self.deleted.is_empty()
            && self.attributes.is_empty()
            && self.edges_added.is_empty()
            && self.edges_removed.is_empty()
            && self.edges_rearmed.is_empty()
    }
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
/// # Why it borrows
///
/// Every handle in here already lives in the [`Scene`] or in the
/// [`Changes`] it was computed from, so owning them meant a `String`
/// clone per named node on a path an interactive host walks every
/// frame. Measured, one transform with *n* children, debug build,
/// best of ten: 1000 nodes 2.08 ms, 10 000 27.3 ms, 50 000 88.9 ms --
/// about 2 us a node, four to five allocations of it. Borrowing costs
/// a lifetime on the type and nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Affected<'a> {
    /// The **roots** of what may have moved: these nodes and
    /// everything below them on the `objects` chain.
    ///
    /// Roots rather than an enumeration, because enumerating is
    /// O(scene) for the edit that matters least -- moving one
    /// transform near the top of a production scene names every
    /// geometry under it, and a set-dressing scene has millions. The
    /// consumer walks its own objects anyway;
    /// [`Scene::descendants`] expands a root when it wants the list.
    pub roots: IndexSet<&'a str>,
    /// Shader nodes whose own attributes or network changed.
    ///
    /// Kept apart because they map one-to-one onto a renderer's
    /// material parameters and cost no geometry work: only the *root*
    /// shader's identity reaches geometry, and that is a
    /// `surfaceshader` edge, which lands in `nodes`.
    pub shaders: IndexSet<&'a str>,
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

/// Name a root of the affected set.
///
/// A root stands for itself and everything below it, so a handle that
/// is already named needs no second entry -- and a caller expanding
/// two overlapping roots gets the union either way.
fn insert_root<'a>(roots: &mut IndexSet<&'a str>, handle: &'a str) {
    roots.insert(handle);
}

/// Record an edge in one of [`Changes`]'s lists, at most once.
///
/// Keyed on `(from, to, kind)`, which is an edge's identity to ɴsɪ --
/// `args` carry the priority and are what a re-arm replaces. Without
/// this the lists were logs: a host that re-`connect`s the same edge
/// every frame grew the record by a full `Edge`, arguments included,
/// on every one of them, and the record is supposed to be net.
fn record_edge(list: &mut Vec<Edge>, edge: Edge) {
    match list.iter_mut().find(|recorded| {
        recorded.from == edge.from
            && recorded.to == edge.to
            && recorded.kind == edge.kind
    }) {
        Some(recorded) => *recorded = edge,
        None => list.push(edge),
    }
}

/// The recorded scene graph.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Scene {
    /// Nodes by handle, in creation order.
    nodes: IndexMap<Handle, Node>,
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
    evaluations: Vec<Vec<OwnedArgument>>,
    /// Edge positions keyed by source handle, and by destination.
    ///
    /// Resolution walks the graph per object, so without these every
    /// hop of every walk scanned every edge -- quadratic in the scene,
    /// which a production asset feels immediately. Rebuilt on removal
    /// (rare) and appended to on `connect` (common).
    by_from: HashMap<Handle, Vec<usize>>,
    by_to: HashMap<Handle, Vec<usize>>,
    /// Edge positions keyed by destination *and* destination attribute.
    ///
    /// `by_to` alone is not enough: a transform with twenty thousand
    /// children has twenty thousand incoming `objects` edges, and
    /// gathering attributes there would scan all of them once per
    /// child. Keying on the attribute too makes that lookup
    /// proportional to the matches rather than to the scene.
    by_to_attr: HashMap<(Handle, Handle), Vec<usize>>,
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
    /// Pass it to [`Scene::affected`] for the nodes to re-resolve.
    /// This is the half that cannot be reconstructed after the fact --
    /// a delete takes its edges with it, and a re-armed connection
    /// leaves the graph looking exactly as it did.
    pub fn take_changes(&mut self) -> Changes {
        mem::take(&mut self.changes)
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
    pub fn affected<'a>(&'a self, changes: &'a Changes) -> Affected<'a> {
        let mut affected = Affected::default();

        for (handle, _) in &changes.attributes {
            if crate::is_reserved(handle) {
                affected.everything = true;
                continue;
            }
            match handle::map_get(&self.nodes, handle) {
                Some(node) if node.node_type == "shader" => {
                    affected.shaders.insert(handle.as_str());
                }
                Some(node) if node.node_type == "attributes" => {
                    self.through_bindings(handle, &mut affected.roots);
                }
                // Anything else is a scene node: it may be the thing
                // that moved, and it may be a transform with a subtree
                // under it. The descent answers both.
                _ => insert_root(&mut affected.roots, handle),
            }
            if self.is_output_node(handle) {
                affected.outputs = true;
            }
        }

        for handle in &changes.created {
            insert_root(&mut affected.roots, handle);
        }

        // A deleted handle is gone from the graph, so there is nothing
        // left to descend *from*: what it orphaned is reached through
        // the edges the delete took with it, which is why they are
        // recorded in full.
        for handle in changes.deleted.keys() {
            affected.roots.insert(handle.as_str());
        }

        for edge in changes
            .edges_added
            .iter()
            .chain(&changes.edges_removed)
            .chain(&changes.edges_rearmed)
        {
            // On `EdgeKind`, with no wildcard. Matching the
            // *destination attribute string* let a new variant land
            // silently in whatever arm the `_` was, and a
            // `ShaderNetwork` whose port happens to be named `objects`
            // took the child arm -- the resolver already carries a
            // comment about that hazard, and this had re-learned it.
            // Exhaustive here means the compiler asks the question.
            match &edge.kind {
                // A child, a set member: whatever hung below the
                // source now hangs somewhere else.
                EdgeKind::SceneMember | EdgeKind::SetMember => {
                    insert_root(&mut affected.roots, &edge.from);
                    if self.is_output_node(&edge.from)
                        || self.is_output_node(&edge.to)
                    {
                        affected.outputs = true;
                    }
                }
                // A prototype joining or leaving an instancer. The
                // node whose answer moves is the **instancer**, and it
                // is not below the prototype -- it is the other end of
                // this edge, which is exactly why a walk that only
                // descends from `edge.from` missed it: rendered, a
                // severed `sourcemodels` leaves the instancer drawing
                // a prototype the scene no longer has.
                EdgeKind::InstanceSource => {
                    insert_root(&mut affected.roots, &edge.from);
                    affected.roots.insert(edge.to.as_str());
                }
                // A container bound to something, or unbound from it.
                EdgeKind::AttributeBinding | EdgeKind::ShaderAttributes => {
                    insert_root(&mut affected.roots, &edge.to);
                    self.set_members(&edge.to, &mut affected.roots);
                }
                // A shader reaching an `attributes` node -- including a
                // repeated `connect` that only changed `"priority"`,
                // which is why re-armed edges are here.
                EdgeKind::SurfaceShader
                | EdgeKind::DisplacementShader
                | EdgeKind::VolumeShader => {
                    affected.shaders.insert(edge.from.as_str());
                    self.through_bindings(&edge.to, &mut affected.roots);
                }
                // The output chain, and the things hung off it. A
                // light set and a background layer belong to an
                // `outputlayer`, and a lens shader to a camera: none
                // of them is a material parameter, and filing them
                // under `shaders` told a backend honouring light sets
                // nothing at all.
                EdgeKind::Screen
                | EdgeKind::OutputLayer
                | EdgeKind::OutputDriver
                | EdgeKind::LightSet
                | EdgeKind::BackgroundLayer
                | EdgeKind::LensShader => {
                    affected.outputs = true;
                }
                // `.global`'s own membership: it applies to the whole
                // render, so the whole scene is the candidate.
                EdgeKind::ExclusiveShading => {
                    affected.everything = true;
                }
                // Bounds, subsurface sets and face sets attach to a
                // geometry or a set and change what it resolves to.
                EdgeKind::Bounds
                | EdgeKind::SubsurfaceSet
                | EdgeKind::FaceSet => {
                    insert_root(&mut affected.roots, &edge.to);
                    self.set_members(&edge.to, &mut affected.roots);
                }
                // A shader network edge is between shaders, and a
                // carried connection this crate does not resolve is
                // not known to reach geometry. Only ends that really
                // are shader nodes go in `shaders`.
                EdgeKind::ShaderNetwork { .. } | EdgeKind::Other { .. } => {
                    for handle in [&edge.from, &edge.to] {
                        if self.is_shader_node(handle) {
                            affected.shaders.insert(handle.as_str());
                        } else {
                            insert_root(&mut affected.roots, handle);
                        }
                    }
                }
            }
        }

        if affected.everything {
            affected.roots.clear();
        }

        affected
    }

    /// Everything at or below `handle` on the `objects` chain, plus any
    /// instancer that draws a prototype found there.
    ///
    /// This is what an [`Affected`] root stands for. Call it when the
    /// list is actually wanted -- a backend that is walking its own
    /// objects can compare against the roots instead and never build
    /// it.
    ///
    /// Iterative with an explicit stack: an ɴsɪ scene's depth is the
    /// caller's, not ours, and a recursive walk here would overflow on
    /// a deep chain. The `insert` doubles as the visited set, so a
    /// cycle terminates.
    pub fn descendants<'a>(&'a self, handle: &'a str) -> IndexSet<&'a str> {
        let mut out = IndexSet::new();
        self.descend(handle, &mut out);
        out
    }

    fn descend<'a>(&'a self, handle: &'a str, out: &mut IndexSet<&'a str>) {
        let mut stack = vec![handle];
        while let Some(node) = stack.pop() {
            if !out.insert(node) {
                continue;
            }
            for edge in self.edges_to_attribute(node, "objects") {
                stack.push(&edge.from);
            }
            // A prototype's mover moves every instancer drawing it, and
            // the instancer is not below the transform that moved --
            // it is reached the other way, through `sourcemodels`.
            for edge in self.edges_from(node) {
                if edge.kind.to_attribute() == "sourcemodels" {
                    stack.push(&edge.to);
                }
            }
        }
    }

    /// Everything an `attributes` node is bound to, and below that.
    fn through_bindings<'a>(
        &'a self,
        handle: &str,
        out: &mut IndexSet<&'a str>,
    ) {
        for edge in self.edges_from(handle) {
            if matches!(
                edge.kind.to_attribute(),
                "geometryattributes" | "shaderattributes"
            ) {
                insert_root(out, &edge.to);
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
    fn set_members<'a>(&'a self, handle: &str, out: &mut IndexSet<&'a str>) {
        for edge in self.edges_to_attribute(handle, "members") {
            insert_root(out, &edge.from);
        }
    }

    /// Whether this handle is a shader node.
    ///
    /// Asked before filing anything under [`Affected::shaders`], whose
    /// documented meaning is "maps one-to-one onto a renderer's
    /// material parameters". A `set` reached through a `lightset` edge
    /// is not that, and neither is a camera reached through a lens
    /// shader.
    fn is_shader_node(&self, handle: &str) -> bool {
        handle::map_get(&self.nodes, handle)
            .is_some_and(|node| node.node_type() == "shader")
    }

    /// Whether this handle is part of the camera/screen/layer/driver
    /// chain, whose answers come from [`Scene::render_outputs`] rather
    /// than from a geometry walk.
    fn is_output_node(&self, handle: &str) -> bool {
        handle::map_get(&self.nodes, handle).is_some_and(|node| {
            node.node_type().ends_with("camera")
                || matches!(
                    node.node_type(),
                    "screen" | "outputlayer" | "outputdriver"
                )
        })
    }

    /// The nodes, by handle, in creation order.
    pub fn nodes(&self) -> impl Iterator<Item = (&str, &Node)> {
        self.nodes
            .iter()
            .map(|(handle, node)| (handle.as_str(), node))
    }

    /// The recorded `Evaluate` calls, in call order.
    ///
    /// Each is the argument list as given. A backend that wants
    /// archives or procedurals has to execute them itself: this crate
    /// records the call and does not define an execution model for it.
    pub fn evaluations(&self) -> impl Iterator<Item = &[OwnedArgument]> {
        self.evaluations.iter().map(Vec::as_slice)
    }

    /// Record an `Evaluate`.
    ///
    /// Only the call, not where it fell among the nodes: replay emits
    /// every `Evaluate` first, nothing reads a position, and a node
    /// count kept here would be wrong the moment a `delete` shifted
    /// it.
    pub(crate) fn evaluate(&mut self, args: Vec<OwnedArgument>) {
        self.evaluations.push(args);
    }

    /// One node by handle.
    pub fn node(&self, handle: &str) -> Option<&Node> {
        handle::map_get(&self.nodes, handle)
    }

    /// A node together with the scene's own copy of its handle.
    ///
    /// Resolution returns borrowed handles that outlive the `&str` a
    /// caller passed in, so it needs the stored key, not the argument.
    pub(crate) fn node_entry(&self, handle: &str) -> Option<(&str, &Node)> {
        handle::map_entry(&self.nodes, handle)
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

    /// The connections into `handle` through `to_attribute`, in connection
    /// order.
    ///
    /// Indexed on both, so this is proportional to the matches rather
    /// than to the scene.
    pub fn edges_to_attribute<'a>(
        &'a self,
        handle: &str,
        to_attribute: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        // Without `ustr_handles` this allocates two `String`s to probe
        // the map, on what is the hot path's key: a `HashMap<(String,
        // String), _>` cannot be probed with a pair of `&str`.
        // Interned, the probe allocates nothing.
        handle::get_pair(&self.by_to_attr, handle, to_attribute)
            .into_iter()
            .flatten()
            .map(|position| &self.edges[*position])
    }

    fn indexed<'a>(
        &'a self,
        index: &'a HashMap<Handle, Vec<usize>>,
        handle: &str,
    ) -> impl Iterator<Item = &'a Edge> + use<'a> {
        crate::handle::get(index, handle)
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
                .entry(handle::handle(&edge.from))
                .or_default()
                .push(position);
            self.by_to
                .entry(handle::handle(&edge.to))
                .or_default()
                .push(position);
            self.by_to_attr
                .entry((
                    handle::handle(&edge.to),
                    handle::handle(edge.kind.to_attribute()),
                ))
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

        match handle::map_get(&self.nodes, handle) {
            Some(existing) if existing.node_type() != node_type => {
                Err(RecordError::TypeMismatch {
                    handle: handle.to_string(),
                    existing: existing.node_type().to_string(),
                    requested: node_type.to_string(),
                })
            }
            Some(_) => Ok(()),
            None => {
                self.nodes.insert(
                    crate::handle::handle(handle),
                    Node {
                        node_type: crate::handle::handle(node_type),
                        ..Node::default()
                    },
                );
                self.changes.created.insert(handle::handle(handle));
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
            if let Some(node) = handle::map_remove(&mut self.nodes, handle) {
                handle::set_remove(&mut self.changes.created, handle);
                self.changes
                    .deleted
                    .insert(handle::handle(handle), node.node_type);
            }
            for edge in self
                .edges
                .iter()
                .filter(|e| e.from == handle || e.to == handle)
            {
                record_edge(&mut self.changes.edges_removed, edge.clone());
            }
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
            || handle::map_get(&self.nodes, handle).is_some()
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
                .map(|edge| edge.from().to_string())
                .filter(|from| !doomed.contains(from.as_str()))
                .collect::<Vec<_>>();

            let mut grew = false;
            for candidate in candidates {
                let leads_elsewhere = self
                    .edges_from(&candidate)
                    .any(|edge| !doomed.contains(edge.to()));

                // The strength rule is about *this* node's connection to
                // anything being deleted, not only about how it was
                // first reached. Checking it at discovery alone let a
                // node be swept in through a second, weak path.
                let held = self.edges_from(&candidate).any(|edge| {
                    doomed.contains(edge.to()) && edge.strength() > 0
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
            if let Some(node) = handle::map_get(&self.nodes, handle) {
                let node_type = handle::copy(&node.node_type);
                handle::set_remove(&mut self.changes.created, handle);
                self.changes
                    .deleted
                    .insert(handle::handle(handle), node_type);
            }
        }
        for edge in self.edges.iter().filter(|edge| {
            doomed.contains(edge.from()) || doomed.contains(edge.to())
        }) {
            record_edge(&mut self.changes.edges_removed, edge.clone());
        }

        self.nodes
            .retain(|handle, _| !doomed.contains(handle.as_str()));
        self.edges.retain(|edge| {
            !doomed.contains(edge.from()) && !doomed.contains(edge.to())
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
        args: Vec<OwnedArgument>,
    ) -> Result<(), RecordError> {
        // Existence first, so a rejected call records nothing.
        self.node_mut(handle)?;
        let node_handle = handle::handle(handle);
        // `nodes` and `changes` are separate fields, so the node and
        // the journal are borrowed at once. Through `node_mut` they
        // are not, and carrying the names out of the loop in a `Vec`
        // to record them afterwards cost a clone of every one.
        // SAFETY: `node_mut` above returned `Ok`, which means it
        // either found this handle or created it, and nothing since
        // could have removed it.
        let node = self
            .nodes
            .get_mut(&node_handle)
            .expect("node_mut found or created it");
        for arg in args {
            let name = handle::handle(&arg.name);
            // Only when the node has samples: reaching for the table
            // through `sample_table_mut` would allocate one for every
            // node that has an attribute, which is most of a scene.
            if let Some(table) = node.samples.as_mut() {
                table.shift_remove(&name);
            }
            self.changes
                .attributes
                .insert((handle::copy(&node_handle), handle::copy(&name)));
            node.attribute_table_mut().insert(name, arg);
        }
        Ok(())
    }

    /// A node to mutate, or [`RecordError::UnknownHandle`].
    ///
    /// ɴsɪ's reserved handles are created on demand: they "don't need to
    /// be created using NSICreate", but they do carry attributes.
    fn node_mut(&mut self, handle: &str) -> Result<&mut Node, RecordError> {
        if crate::is_reserved(handle) {
            Ok(self.nodes.entry(crate::handle::handle(handle)).or_default())
        } else {
            handle::map_get_mut(&mut self.nodes, handle).ok_or_else(|| {
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
        args: Vec<OwnedArgument>,
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

        // Existence first, so a rejected call records nothing.
        self.node_mut(handle)?;
        let node_handle = handle::handle(handle);
        // See `set_attribute` for why the node is reached for by
        // field rather than through `node_mut`.
        // SAFETY: `node_mut` above returned `Ok`, which means it
        // either found this handle or created it, and nothing since
        // could have removed it.
        let node = self
            .nodes
            .get_mut(&node_handle)
            .expect("node_mut found or created it");
        for arg in args {
            let name = handle::handle(&arg.name);
            // ɴsɪ: setting at a time "replaces any value previously set
            // by NSISetAttribute", so the static value goes.
            if let Some(table) = node.attributes.as_mut() {
                table.shift_remove(&name);
            }
            self.changes
                .attributes
                .insert((handle::copy(&node_handle), handle::copy(&name)));

            // Appended, never merged: a re-set at a time already
            // recorded is another call, and what it superseded is part
            // of the record. `Node::samples` says why.
            node.sample_table_mut()
                .entry(name)
                .or_default()
                .push((time, arg));
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
            .insert((handle::handle(handle), handle::handle(name)));
        if let Some(node) = handle::map_get_mut(&mut self.nodes, handle) {
            if let Some(table) = node.attributes.as_mut() {
                table.shift_remove(&handle::handle(name));
            }
            if let Some(table) = node.samples.as_mut() {
                table.shift_remove(&handle::handle(name));
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
        from_attribute: Option<&str>,
        to: &str,
        to_attribute: &str,
    ) -> Result<(), RecordError> {
        self.connect_with_arguments(
            from,
            from_attribute,
            to,
            to_attribute,
            Vec::new(),
        )
    }

    /// Classify and record a connection carrying its ɴsɪ arguments.
    ///
    /// ɴsɪ: "It is not an error to create a connection which already
    /// exists." A repeat therefore updates the existing edge's arguments
    /// rather than adding a second one. Recording both would make the
    /// node look like it had two parents, which would fail resolution
    /// for it and everything beneath it.
    pub fn connect_with_arguments(
        &mut self,
        from: &str,
        from_attribute: Option<&str>,
        to: &str,
        to_attribute: &str,
        args: Vec<OwnedArgument>,
    ) -> Result<(), RecordError> {
        let kind = classify(from_attribute, to_attribute);

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

        // Through the index, not over every edge in the scene. A
        // repeated `connect` has to be detected before one is appended,
        // and scanning `edges` for it made recording **quadratic**:
        // measured, one transform with 1000 children took 67 ms to
        // build, 4000 took 749 ms and 16 000 took 17 s. An interactive
        // host connects per edit, so it paid that on every one.
        let existing = handle::get(&self.by_from, from).and_then(|positions| {
            positions.iter().copied().find(|&at| {
                let edge = &self.edges[at];
                edge.to == to && edge.kind == kind
            })
        });

        let mut rearmed = None;
        match existing.map(|at| &mut self.edges[at]) {
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
                    .entry(handle::handle(from))
                    .or_default()
                    .push(self.edges.len());
                self.by_to
                    .entry(handle::handle(to))
                    .or_default()
                    .push(self.edges.len());
                self.by_to_attr
                    .entry((
                        handle::handle(to),
                        handle::handle(kind.to_attribute()),
                    ))
                    .or_default()
                    .push(self.edges.len());
                let edge = Edge {
                    from: handle::handle(from),
                    to: handle::handle(to),
                    kind,
                    args,
                };
                record_edge(&mut self.changes.edges_added, edge.clone());
                self.edges.push(edge);
            }
        }

        if let Some(edge) = rearmed {
            record_edge(&mut self.changes.edges_rearmed, edge);
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
        from_attribute: Option<&str>,
        to: &str,
        to_attribute: &str,
    ) -> Result<(), RecordError> {
        // `.all` matches every class, so there is nothing to classify.
        let kind = if to_attribute == ALL {
            None
        } else {
            Some(classify(from_attribute, to_attribute))
        };

        let from_port = from_attribute.unwrap_or_default();
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
                // A named `to_attribute` fixes the class outright, unless the
                // source port is `.all` -- then only the destination
                // attribute is being matched.
                Some(kind) if !any_port => &edge.kind == kind,
                Some(kind) => edge.kind.to_attribute() == kind.to_attribute(),
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
        for edge in removed {
            record_edge(&mut self.changes.edges_removed, edge);
        }
        self.reindex();

        Ok(())
    }
}

#[cfg(test)]
mod tests;
