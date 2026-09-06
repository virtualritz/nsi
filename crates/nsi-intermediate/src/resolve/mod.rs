//! Graph rewrites: turning ɴsɪ's scene-graph semantics into the flat
//! facts a renderer wants.
//!
//! Both target renderers need this and neither should re-derive it.
//! Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
//! resolves geometry to world space too. So the chain has to be
//! composed here, once.

use crate::{Edge, EdgeKind, Node, OwnedArg, OwnedData, Scene};
use core::{cmp::Ordering, fmt};
use indexmap::IndexMap;
use nsi_trait::Type;
use std::collections::HashSet;

/// A 4x4 identity, row-major.
#[rustfmt::skip]
pub const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

/// The ɴsɪ attribute holding a transform node's matrix.
const TRANSFORMATION_MATRIX: &str = "transformationmatrix";

/// An `instances` node's per-instance matrices.
const MATRICES: &str = "transformationmatrices";

/// Which prototype each instance draws.
const MODEL_INDICES: &str = "modelindices";

/// The instances an `instances` node skips.
const DISABLED: &str = "disabledinstances";

/// Why a scene could not be resolved into flat facts.
///
/// Every variant is a scene ɴsɪ permits but this crate refuses to guess
/// about. Returning a matrix or a binding anyway would be the silent
/// failure the crate exists to prevent: it renders, with the wrong
/// answer.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ResolveError {
    /// A node is connected to more than one parent.
    ///
    /// Usually through `objects`; a prototype connected to two
    /// `instances` nodes is the same problem.
    ///
    /// That is ɴsɪ's lightweight instancing: the node appears once per
    /// path, each with its own world transform. A single matrix cannot
    /// describe it, so resolving one is refused rather than silently
    /// answering for whichever parent was connected first.
    MultipleParents {
        /// The node with more than one parent.
        handle: String,
        /// Every parent, in connection order.
        parents: Vec<String>,
    },
    /// A node carries motion-sampled placement data where a single
    /// answer was asked for.
    ///
    /// Either a `transformationmatrix` on a node in the chain, or an
    /// `instances` node's `transformationmatrices`, `modelindices` or
    /// `disabledinstances`. Answering with the static value would hand
    /// a motion-blurred scene back its unblurred pose, and answering
    /// with an empty list -- which the instancer path used to do --
    /// reads as "nothing to draw".
    ///
    /// Ask at a time instead:
    /// [`Scene::world_transform_interpolated_at`],
    /// [`Scene::placements_at`] or
    /// [`Scene::instance_transforms_at`].
    MotionSampledTransform {
        /// The node whose transform is motion-sampled.
        handle: String,
    },
    /// The transform chain revisits a node. ɴsɪ does not forbid a cycle;
    /// no correct answer exists for one.
    Cycle {
        /// The node the walk arrived at twice.
        handle: String,
    },
    /// The node is not connected to `.root`, directly or through
    /// transforms.
    ///
    /// ɴsɪ: "A node can exist in an nsi context without being connected
    /// to the root node but in that case it won't affect the render in
    /// any way." It has no world transform and no gathered attributes,
    /// and answering identity would put unrendered geometry at the
    /// origin.
    Detached {
        /// The node that does not reach `.root`.
        handle: String,
    },
    /// No node with that handle exists.
    ///
    /// Asking about an attribute of a node that was never created is a
    /// caller mistake, and answering "not sampled" would read as a fact
    /// about the scene rather than about the question.
    UnknownHandle {
        /// The handle that names nothing.
        handle: String,
    },
    /// The node is an instancing prototype, so it has no single world
    /// transform.
    ///
    /// A prototype reaches the scene through an `instances` node, and
    /// ɴsɪ gives that node "a transformation matrix for each instance".
    /// Answering with the instancer's own transform would put every
    /// instance in the same, wrong place.
    Instanced {
        /// The `instances` node the prototype is connected to.
        instancer: String,
    },
    /// An `instances` node's `transformationmatrices` is not a whole
    /// number of 4x4 matrices.
    MalformedInstanceMatrices {
        /// The `instances` node.
        instances: String,
        /// How many values it carries.
        values: usize,
    },
    /// Two prototype connections share one `index`.
    ///
    /// ɴsɪ: connections "must have an integer index attribute if there
    /// are several, so the models effectively form an ordered list" --
    /// which a duplicate does not.
    DuplicateModelIndex {
        /// The `instances` node.
        instances: String,
        /// The index used twice.
        index: i32,
    },
    /// A `modelindices` entry matches no prototype connection's
    /// `index`.
    UnknownModelIndex {
        /// The `instances` node.
        instances: String,
        /// The index that matches nothing.
        model: i32,
    },
    /// [`Scene::relative_transform`] was given a node that is not on the
    /// chain.
    NotAnAncestor {
        /// The node whose chain was walked.
        handle: String,
        /// The node that is not on it.
        ancestor: String,
    },
    /// A node in the chain is motion-sampled, but has no sample at the
    /// requested time.
    ///
    /// From the **exact-hit** accessors only -- [`Scene::world_transform_at`],
    /// [`Scene::world_transform_samples`] and
    /// [`Scene::instance_transforms`] answer where a sample exists and
    /// refuse elsewhere, which is the right answer to "what did the
    /// caller record".
    ///
    /// For "where is it mid-shutter", ask
    /// [`Scene::world_transform_interpolated_at`],
    /// [`Scene::placements_at`] or
    /// [`Scene::instance_transforms_at`]. Those interpolate
    /// element-wise, which is the renderer's own model rather than a
    /// guess -- 3Delight's rotation blur fits component-wise far better
    /// than slerp -- and they hold the end sample outside the sampled
    /// range, as it does.
    ///
    /// The interpolating accessors still return this for a time that
    /// names no sample at all, such as a NaN. An earlier version of
    /// this said the crate never interpolates and that the
    /// decomposition was the backend's; that stopped being true when
    /// those accessors were added.
    MissingSampleAtTime {
        /// The node with no sample at that time.
        handle: String,
        /// The time that was asked for.
        time: f64,
        /// The times that node does have, ascending.
        available: Vec<f64>,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MultipleParents { handle, parents } => write!(
                f,
                "ɴsɪ node {handle:?} has {} parents ({}); that is \
                 instancing, which has one world transform per path, not \
                 one overall",
                parents.len(),
                parents.join(", ")
            ),
            Self::MotionSampledTransform { handle } => write!(
                f,
                "ɴsɪ node {handle:?} carries motion-sampled placement \
                 data; the static value would be the wrong answer, so \
                 ask at a time instead"
            ),
            Self::Cycle { handle } => write!(
                f,
                "ɴsɪ transform chain revisits node {handle:?}; a cyclic \
                 scene has no world transform"
            ),
            Self::UnknownHandle { handle } => {
                write!(f, "no ɴsɪ node is named {handle:?}")
            }
            Self::Detached { handle } => write!(
                f,
                "ɴsɪ node {handle:?} is not connected to {root:?}, so it \
                 is not in the scene and has no world transform",
                root = crate::ROOT
            ),
            Self::Instanced { instancer } => write!(
                f,
                "ɴsɪ node is an instancing prototype of {instancer:?}, \
                 which carries one transform per instance; there is no \
                 single world transform for it"
            ),
            Self::MalformedInstanceMatrices { instances, values } => {
                write!(
                    f,
                    "ɴsɪ node {instances:?} has {values} values in \
                     transformationmatrices, which is not a whole number \
                     of 4x4 matrices"
                )
            }
            Self::DuplicateModelIndex { instances, index } => write!(
                f,
                "ɴsɪ node {instances:?} has two sourcemodels connections \
                 at index {index}, so its models are not an ordered list"
            ),
            Self::UnknownModelIndex { instances, model } => write!(
                f,
                "ɴsɪ node {instances:?} selects model index {model}, which \
                 matches no sourcemodels connection"
            ),
            Self::NotAnAncestor { handle, ancestor } => write!(
                f,
                "ɴsɪ node {ancestor:?} is not on {handle:?}'s chain, so \
                 there is no transform of one relative to the other"
            ),
            Self::MissingSampleAtTime {
                handle,
                time,
                available,
            } => write!(
                f,
                "ɴsɪ node {handle:?} has no transform sample at time \
                 {time}; it has {available:?}, and this crate does not \
                 interpolate between them"
            ),
        }
    }
}

impl core::error::Error for ResolveError {}

/// What an ɴsɪ `attributes` node resolves to for one piece of geometry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Binding {
    /// Every `attributes` node gathered along the path, nearest the
    /// geometry first, then in connection order.
    ///
    /// The `priority` on a `geometryattributes` connection deliberately
    /// does **not** reorder this: 3Delight ignores it, whatever the
    /// specification says. A priority on a *shader* connection is
    /// honoured; see [`Binding::surface_shader`] and `research.md` D12.
    ///
    /// ɴsɪ gathers attributes along the whole path and considers
    /// *every* node on it -- "one attributes node can set object
    /// visibility and another can set the surface shader" -- so this is
    /// a list, not a winner. A backend looking for one attribute takes
    /// the first node in this list that defines it -- correct only while
    /// no node sets `ATTR.priority` and the attribute is not a
    /// visibility one. [`Scene::attribute_value`] applies those two
    /// rules and is the safe way to ask.
    ///
    /// When this [`Binding`] came from a [`Placement`], ask
    /// [`Scene::attribute_value_along`] with that placement's path
    /// instead: `attribute_value` takes a geometry, so it refuses the
    /// multi-parent node a placement exists for.
    ///
    /// The handles are returned rather than their contents because what
    /// lives on them -- visibility flags above all -- is encoded
    /// differently by each renderer, and inventing a common shape for it
    /// here would be guesswork.
    pub attributes: Vec<String>,
    /// The shader reached through `surfaceshader`, resolved across every
    /// gathered node by the same rule, using the priority of the
    /// `surfaceshader` connection itself.
    ///
    /// `None` when nothing on the path sets one.
    pub surface_shader: Option<String>,
    /// The `displacementshader`, resolved the same way.
    pub displacement_shader: Option<String>,
    /// The `volumeshader`, resolved the same way.
    pub volume_shader: Option<String>,
}

/// ɴsɪ's ray types: the suffixes a `visibility.<ray>` attribute takes.
///
/// Spelled out rather than accepting any suffix, because
/// `visibility.set.subsurface` is a *connection* to a `set` node and not
/// a per-ray visibility int; treating it as the more specific form of
/// `visibility` would rank a connection against a flag.
///
/// Public because a backend building a visibility mask needs exactly
/// this list, and a second copy of it would drift from this one.
pub const RAY_TYPES: [&str; 8] = [
    "camera",
    "diffuse",
    "hair",
    "reflection",
    "refraction",
    "shadow",
    "specular",
    "volume",
];

/// One placement of a geometry in the scene.
///
/// ɴsɪ's *lightweight* instancing: connecting a node to two transforms
/// draws it twice, once per path. Rendered, a quad under two transforms
/// translated `-2` and `+2` appears at both positions, and putting
/// `visibility 1` on one parent and `visibility 0` on the other draws
/// **one** copy -- so a path carries its own attributes as well as its
/// own transform, and a backend emitting one instance per placement
/// needs both together.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Placement {
    /// The nodes from the geometry to `.root`, geometry first.
    ///
    /// Two placements of one geometry differ here, and that is what
    /// makes them distinguishable: the handle alone does not.
    pub path: Vec<String>,
    /// The world transform along this path.
    pub transform: [f64; 16],
    /// What binds along this path, or `None` when nothing does.
    ///
    /// The same shape [`Scene::geometry_binding`] returns, resolved
    /// against this path rather than against "the" path.
    pub binding: Option<Binding>,
}

/// A node's effective value for an attribute, sampled or not.
///
/// `SetAttributeAtTime` on an attribute that is not motion data sets it
/// for the whole shutter, exactly as `SetAttribute` would. Rendered: an
/// `attributes` node whose `visibility` is set **only** through
/// `SetAttributeAtTime` hides the object, identically to the static
/// form, where a scene with nothing set renders it.
///
/// Reading `node.attrs` alone therefore answered "not set" for an
/// attribute the renderer honours -- a silent wrong answer, and the
/// same rule this crate already applied to an instancer's
/// `modelindices` and `disabledinstances`.
///
/// Static first, then the last sample naming it. The two never coexist:
/// `set_attribute` clears that name from every sample and
/// `set_attribute_at_time` clears the static value, so there is nothing
/// to arbitrate.
fn effective_attr<'a>(node: &'a Node, name: &str) -> Option<&'a OwnedArg> {
    node.attrs.get(name).or_else(|| {
        node.time_attrs
            .iter()
            .rev()
            .find_map(|(_, attrs)| attrs.get(name))
    })
}

/// What a node's samples of one attribute amount to.
#[derive(Clone, Copy)]
enum Sampled<'a> {
    /// Not sampled; the static value applies.
    No,
    /// Sampled, but the last sample *naming* it cannot be read as the
    /// attribute's type -- so the attribute is unset, at every time.
    Unset,
    /// The surviving tail of the node's `time_attrs`.
    ///
    /// Never empty of the attribute: `keep_from` is one past the last
    /// *unreadable* naming sample, and the last naming sample is
    /// readable whenever this variant is built, so it is always in the
    /// tail. Three call sites carried an `is_empty` branch for a case
    /// that cannot arise; they are gone.
    ///
    /// A slice rather than a collected `Vec`: this is read per node per
    /// query on the sampled paths, so collecting here allocated once
    /// per node per time.
    ///
    /// The remaining cost is **one hash lookup per sample per pass**,
    /// not the two passes as such: the rule needs the last naming
    /// sample before it can decide anything, and each pass pays an
    /// `IndexMap<String, _>` probe for every sample. Streaming it in a
    /// single pass was measured and costs the same -- it trades the
    /// second lookup for a push. See `contracts/resolution.md`; an
    /// earlier version of this comment blamed the pass count, which the
    /// measurement does not support.
    Yes(&'a [(f64, IndexMap<String, OwnedArg>)]),
}

impl<'a> Sampled<'a> {
    /// The surviving samples of `name`, in time order.
    fn samples(
        self,
        name: &'a str,
    ) -> impl Iterator<Item = (f64, &'a OwnedArg)> {
        let tail = match self {
            Self::Yes(tail) => tail,
            Self::No | Self::Unset => &[][..],
        };
        tail.iter()
            .filter_map(move |(time, attrs)| Some((*time, attrs.get(name)?)))
    }
}

/// Apply ɴsɪ's typing rule to a node's samples of `name`.
///
/// An unreadable sample **discards every sample before it**. 3Delight
/// warns `E6007` and unsets the attribute at that call; only samples
/// set afterwards rebuild it. So if the last sample naming the
/// attribute is unreadable it is unset entirely, and otherwise the
/// answer is the run of samples following the last unreadable one.
///
/// Rendered, and this is the case that settles it: a good
/// `doublematrix` at `t=0`, a `float` at `t=1`, a good one at `t=2`
/// draws a **static** object at the `t=2` matrix -- one lit band. The
/// control without the `float` sweeps across four. Dropping the
/// unreadable sample and keeping the two good ones, which this rule
/// first said, produces that sweep: a motion blur the renderer does not
/// draw.
///
/// Stated once because it was stated six times and only one was right.
/// The correction matters more than the sharing: deduplicating a rule
/// makes every site agree, and they agreed on the wrong answer until
/// this scene was rendered.
fn sampled_attr<'a>(
    node: &'a Node,
    name: &str,
    readable: impl Fn(&OwnedArg) -> bool,
) -> Sampled<'a> {
    // One pass for the last sample naming the attribute, and for the
    // point after the last unreadable one.
    let mut last_named = None;
    let mut keep_from = 0;
    for (index, (_, attrs)) in node.time_attrs.iter().enumerate() {
        if let Some(arg) = attrs.get(name) {
            last_named = Some(index);
            if !readable(arg) {
                keep_from = index + 1;
            }
        }
    }

    let Some(last_named) = last_named else {
        return Sampled::No;
    };

    // The last naming sample is itself unreadable.
    if keep_from > last_named {
        return Sampled::Unset;
    }

    // Everything before `keep_from` is discarded; `keep_from - 1` is
    // the last unreadable sample, when there is one. The sample *at*
    // `keep_from` is the first survivor and is readable whenever it
    // names the attribute -- an earlier comment here claimed nothing at
    // or before it could be readable, which was false of the boundary
    // and of every discarded sample. Handed back as a slice so a caller
    // that only needs times or existence allocates nothing.
    Sampled::Yes(&node.time_attrs[keep_from..])
}

/// Where a time falls among a set of samples.
enum Located<'a, T> {
    /// Use this sample as it stands: an exact hit, or a held end.
    At(&'a T),
    /// Interpolate, `alpha` of the way from the first to the second.
    Between(&'a T, &'a T, f64),
}

/// Locate `time` among samples ordered by time.
///
/// The one statement of ɴsɪ's sampling rule -- exact hit, hold the end
/// outside the range, otherwise interpolate between the bracketing
/// pair. Both the transform path and the instancer path read it here.
///
/// It exists because they did not. The instancer's copy dropped the
/// exact-hit branch, so asking at an interior sample time -- shutter
/// centre, the single most ordinary query -- errored on a scene the
/// renderer draws. That was the fourth time a copied resolution rule
/// drifted in this crate, and the commit before it removed three.
///
/// `None` when there are no samples, or when `time` names nothing: a
/// NaN compares false against everything and brackets no pair.
fn locate_sample<T>(samples: &[(f64, T)], time: f64) -> Option<Located<'_, T>> {
    // `-0.0` names the sample at `0.0`: the recorder folds the two when
    // storing, and the renderer reads `-0` as `+0`. Without this,
    // `total_cmp` misses the exact hit, the `<=`/`>=` ends do not clamp
    // an *interior* `0.0`, and the strict windows bracket nothing -- so
    // querying at `-0.0` errored on a sample that exists.
    let time = time + 0.0;

    let first = samples.first()?;
    let last = samples.last()?;

    if let Some((_, value)) = samples
        .iter()
        .find(|(t, _)| t.total_cmp(&time) == Ordering::Equal)
    {
        return Some(Located::At(value));
    }

    if time <= first.0 {
        return Some(Located::At(&first.1));
    }
    if time >= last.0 {
        return Some(Located::At(&last.1));
    }

    samples
        .windows(2)
        .find(|pair| pair[0].0 < time && time < pair[1].0)
        .map(|pair| {
            let alpha = (time - pair[0].0) / (pair[1].0 - pair[0].0);
            Located::Between(&pair[0].1, &pair[1].1, alpha)
        })
}

/// The winning definition of one attribute, gathered along a path.
///
/// Returned by [`Scene::attribute_value`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct AttributeValue<'a> {
    /// The node the winning definition sits on.
    ///
    /// Usually an `attributes` node. For a shader attribute it can be
    /// the geometry itself, which ɴsɪ ranks above every container.
    pub node: &'a str,
    /// The definition itself.
    ///
    /// [`OwnedArg::name`] is the attribute that actually won, which is
    /// not always the one asked for: in [`Scene::attribute_value`] a
    /// `visibility.<ray>` query falls back to the less specific
    /// `visibility`, and this is how a backend tells the two apart.
    /// [`Scene::shader_attribute_value`] performs no such fallback, so
    /// there the name always matches the query.
    pub arg: &'a OwnedArg,
    /// The `ATTR.priority` that selected it; `0` when none is set.
    pub priority: i32,
}

/// Read an `ATTR.priority`.
///
/// ɴsɪ declares it `int`, and 3Delight reads nothing else -- **not even
/// `int64`**. Rendered: `visibility 1` with `"visibility.priority"` as
/// an `int64` 10 loses to a nearer `visibility 0`, and loses to a rival
/// `int` priority of 5. The `int64` is echoed back by `renderdl -cat`,
/// so it is parsed and then ignored. `int64` *is* accepted for the
/// `visibility` value itself, so the rejection is specific to the
/// priority.
///
/// Reading one here would rank a node the renderer does not.
fn priority_value(arg: &OwnedArg) -> Option<i32> {
    match &arg.data {
        OwnedData::I32(values) => values.first().copied(),
        _ => None,
    }
}

/// One renderable output: a camera paired with a screen, and the AOVs
/// written from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderOutput {
    /// The camera the screen is connected to.
    pub camera: String,
    /// The screen, which is what carries resolution and oversampling.
    pub screen: String,
    /// AOVs in connection order; may be empty.
    pub layers: Vec<OutputLayer>,
}

/// One entry of a chain walk, and how it reached its parent.
struct ChainLink {
    handle: String,
    /// The step to the parent was a `sourcemodels` connection.
    via_instancer: bool,
}

/// One instance an `instances` node places.
///
/// ɴsɪ gives an `instances` node "a transformation matrix for each
/// instance" and an optional `modelindices` selecting which prototype
/// each uses. This pairs the two, so a backend building a MoonRay
/// `InstanceGeometry` or a Mitsuba `shapegroup` reference does not have
/// to.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Instance {
    /// Which prototype this instance draws, as a position in
    /// [`Scene::instance_sources`].
    ///
    /// ɴsɪ matches `modelindices` against "the index attribute of the
    /// model connection", not against connection order, so this is the
    /// resolved position rather than the raw value.
    pub source: usize,
    /// This instance's transform, in the `instances` node's space.
    pub transform: [f64; 16],
}

/// One AOV and the drivers it is written to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputLayer {
    /// The `outputlayer` node's handle.
    pub handle: String,
    /// A layer may fan out to several drivers -- a file and a display,
    /// say -- so this is a list, in connection order.
    pub drivers: Vec<String>,
}

/// Row-major 4x4 product, `a` then `b`.
///
/// ɴsɪ uses the RenderMan row-vector convention: a point is a row and
/// transforms multiply on the right, so `p * a * b` applies `a` first.
/// Composing a child with its parent is therefore `mul(child, parent)`,
/// not the other way round.
fn mul(a: [f64; 16], b: [f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

impl Scene {
    /// The chain from `handle` up to and including `.root`, nearest
    /// first.
    ///
    /// `.root` *is* an entry. ɴsɪ gathers attributes "until the scene
    /// root is reached", and describes the root as "much like a
    /// transform node" with its own `objects` and `geometryattributes`,
    /// so a scene-wide `attributes` node bound to it must be found.
    ///
    /// Every walk up the `objects` hierarchy goes through here, so the
    /// scenes with no single answer -- more than one parent, a cycle, a
    /// node that never reaches the root -- are rejected in one place
    /// rather than each caller re-deriving them.
    fn chain(&self, handle: &str) -> Result<Vec<String>, ResolveError> {
        Ok(self
            .linked_chain(handle, true)?
            .into_iter()
            .map(|link| link.handle)
            .collect())
    }

    /// The chain, refusing to pass through an `instances` node.
    ///
    /// Attribute gathering continues through one; transform composition
    /// cannot, because an `instances` node holds one matrix per
    /// instance rather than one for the prototype.
    fn transform_chain(
        &self,
        handle: &str,
    ) -> Result<Vec<String>, ResolveError> {
        Ok(self
            .linked_chain(handle, false)?
            .into_iter()
            .map(|link| link.handle)
            .collect())
    }

    /// The chain, recording for each entry whether the step to its
    /// parent was an `instances` connection rather than a transform.
    ///
    /// [`Scene::relative_transform`] needs that: an `instances` node's
    /// own matrix is not the prototype's, so a composition may not cross
    /// one even when the walk may.
    fn linked_chain(
        &self,
        handle: &str,
        through_instances: bool,
    ) -> Result<Vec<ChainLink>, ResolveError> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = handle.to_string();

        loop {
            if !seen.insert(current.clone()) {
                return Err(ResolveError::Cycle { handle: current });
            }

            if current == crate::ROOT {
                chain.push(ChainLink {
                    handle: current,
                    via_instancer: false,
                });
                break;
            }

            // ɴsɪ gathers "through all the transform nodes it is
            // connected to", so a direct placement is the path. An
            // `instances` connection is only the way up when there is no
            // direct one -- a prototype may be both, and that is not
            // ambiguous.
            let mut scene_parents = self
                .edges_from(&current)
                .filter(|edge| edge.kind == EdgeKind::SceneMember);

            let first = match scene_parents.next() {
                Some(edge) => {
                    if let Some(second) = scene_parents.next() {
                        // ɴsɪ's lightweight instancing: one world
                        // transform per path, so no single answer.
                        let parents = [edge, second]
                            .into_iter()
                            .chain(scene_parents)
                            .map(|edge| edge.to.clone())
                            .collect();
                        return Err(ResolveError::MultipleParents {
                            handle: current,
                            parents,
                        });
                    }
                    edge
                }
                None => {
                    // Reached only through an `instances` node, if at
                    // all.
                    let mut instancers = self
                        .edges_from(&current)
                        .filter(|edge| edge.kind == EdgeKind::InstanceSource);

                    let Some(instancer) = instancers.next() else {
                        return Err(ResolveError::Detached { handle: current });
                    };

                    if !through_instances {
                        // An `instances` node holds one matrix per
                        // instance, not one for the prototype.
                        return Err(ResolveError::Instanced {
                            instancer: instancer.to.clone(),
                        });
                    }

                    if let Some(second) = instancers.next() {
                        let parents = [instancer, second]
                            .into_iter()
                            .chain(instancers)
                            .map(|edge| edge.to.clone())
                            .collect();
                        return Err(ResolveError::MultipleParents {
                            handle: current,
                            parents,
                        });
                    }

                    instancer
                }
            };

            let parent = first.to.clone();
            chain.push(ChainLink {
                handle: current,
                via_instancer: first.kind == EdgeKind::InstanceSource,
            });
            current = parent;
        }

        Ok(chain)
    }

    /// Compose the transform chain applying to `handle`, from the node
    /// itself up to `.root`.
    ///
    /// Includes `handle`'s own matrix when it carries one, so asking
    /// about a transform and asking about its child differ by exactly
    /// that node's matrix.
    ///
    /// Non-`f64` matrices are ignored: ɴsɪ's `transformationmatrix` is
    /// documented as `doublematrix`, and silently reinterpreting an
    /// `f32` one would be worse than skipping it.
    ///
    /// # Errors
    ///
    /// Every variant of [`ResolveError`]. Each is a scene with no single
    /// correct world transform, and each would otherwise be answered
    /// with a plausible wrong matrix.
    pub fn world_transform(
        &self,
        handle: &str,
    ) -> Result<[f64; 16], ResolveError> {
        let chain = self.transform_chain(handle)?;
        self.compose_along(&chain)
    }

    /// Every time at which a transform in `handle`'s chain is sampled,
    /// ascending, deduplicated.
    ///
    /// Empty for a wholly static chain, which is the check a backend
    /// makes to decide between [`Scene::world_transform`] and
    /// [`Scene::world_transform_samples`].
    ///
    /// **Transforms only.** An `instances` node whose
    /// `transformationmatrices` are sampled reports no motion times
    /// here, while [`Scene::instance_transforms`] refuses it and says to
    /// ask at a time -- leaving a backend that used this as its
    /// static-or-sampled check with no time to ask at. Use
    /// [`Scene::attribute_times`] with `"transformationmatrices"` for an
    /// instancer, and [`Scene::attribute_times`] with `"P"` for
    /// deforming geometry.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MultipleParents`] or [`ResolveError::Cycle`].
    pub fn motion_times(&self, handle: &str) -> Result<Vec<f64>, ResolveError> {
        let chain = self.transform_chain(handle)?;

        // The typing rule again: an unreadable sample is not a motion
        // time. Reporting one made `world_transform_samples` iterate a
        // time that `world_transform_at` then refused -- the same scene
        // answered differently depending on which accessor asked.
        let mut times = chain
            .iter()
            .filter_map(|node| self.node(node))
            .flat_map(|node| {
                sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
                    matrix_of(arg).is_some()
                })
                .samples(TRANSFORMATION_MATRIX)
            })
            .map(|(time, _)| time)
            .collect::<Vec<_>>();

        // `total_cmp` throughout, matching how the samples were keyed.
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| a.total_cmp(b) == Ordering::Equal);

        Ok(times)
    }

    /// The node for `handle`, or `None` for a reserved handle nothing
    /// has been set on yet.
    ///
    /// `.root` and `.global` exist whether or not they were created, so
    /// they have no samples rather than being unknown. Without this the
    /// answer flipped between `Err` and `Ok` depending on whether some
    /// unrelated attribute had been set on `.root` first, and the error
    /// text claimed no node was named `.root`.
    fn existing_node(
        &self,
        handle: &str,
    ) -> Result<Option<&Node>, ResolveError> {
        match self.node(handle) {
            Some(node) => Ok(Some(node)),
            None if crate::is_reserved(handle) => Ok(None),
            None => Err(ResolveError::UnknownHandle {
                handle: handle.to_string(),
            }),
        }
    }

    /// Every time at which `name` is sampled on `handle`, ascending.
    ///
    /// Empty when the attribute is static or absent, which is the check
    /// a backend makes before asking for
    /// [`Scene::attribute_samples`] -- the same shape as
    /// [`Scene::motion_times`] and [`Scene::world_transform_samples`].
    ///
    /// This is what makes deforming geometry resolvable: a mesh whose
    /// `P` is sampled under a *static* transform has no motion times at
    /// all, so [`Scene::motion_times`] answers "static" for something
    /// that plainly moves. Ask this for `"P"`, and take the union with
    /// [`Scene::motion_times`] when a backend needs every time the
    /// object changes -- ɴsɪ does not require the two to agree.
    ///
    /// # Every recorded time, including unreadable ones
    ///
    /// [`Scene::motion_times`] drops a sample whose type it cannot
    /// read, because 3Delight unsets such an attribute and it knows
    /// `transformationmatrix` is a `doublematrix`. This cannot: `name`
    /// is any attribute, and the crate does not carry ɴsɪ's type for
    /// each one, so "unreadable" has no meaning here. It reports what
    /// was recorded and [`Scene::attribute_samples`] hands over the
    /// arguments for a caller that knows the type to judge.
    ///
    /// So the two disagree by design on a scene with a wrong-typed
    /// transform sample, and that is the one case worth knowing about.
    ///
    /// # Errors
    ///
    /// [`ResolveError::UnknownHandle`] if no such node exists.
    pub fn attribute_times(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<Vec<f64>, ResolveError> {
        let Some(node) = self.existing_node(handle)? else {
            return Ok(Vec::new());
        };

        // `time_attrs` is kept in `total_cmp` order as it is recorded,
        // and each time appears once, so this needs no sort or dedup.
        Ok(node
            .time_attrs
            .iter()
            .filter(|(_, attrs)| attrs.contains_key(name))
            .map(|(time, _)| *time)
            .collect())
    }

    /// The recorded samples of `name` on `handle`, ascending by time.
    ///
    /// Empty on the same terms as [`Scene::attribute_times`]. The
    /// static value, if any, is *not* included: a sampled attribute and
    /// a static one are separate recordings, and mixing them would
    /// invent a sample at a time the caller never set.
    ///
    /// # Errors
    ///
    /// [`ResolveError::UnknownHandle`] if no such node exists.
    pub fn attribute_samples(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<Vec<(f64, &OwnedArg)>, ResolveError> {
        let Some(node) = self.existing_node(handle)? else {
            return Ok(Vec::new());
        };

        Ok(node
            .time_attrs
            .iter()
            .filter_map(|(time, attrs)| attrs.get(name).map(|arg| (*time, arg)))
            .collect())
    }

    /// The world transform at `time`, interpolating linearly between
    /// the samples that bracket it.
    ///
    /// [`Scene::world_transform_at`] answers only at a recorded sample,
    /// which is right for asking "what did the caller say". A backend
    /// rendering motion blur needs a transform at an arbitrary shutter
    /// time instead, and this is that.
    ///
    /// # Why component-wise is not a guess
    ///
    /// Blur moves *points*. Linearly interpolating a transformed point
    /// gives `(1-a)·M₀p + a·M₁p`, which is `((1-a)·M₀ + a·M₁)·p` -- so
    /// interpolating the matrix element by element is identical to
    /// interpolating the moving point, for every point. It is not an
    /// approximation of some better decomposition; it is what a
    /// renderer blurring geometry already does. Long rotations look
    /// wrong under it because they look wrong under blur, not because
    /// this differs from the renderer.
    ///
    /// Each node on the chain is interpolated from **its own** samples
    /// and the results composed, because each node is animated
    /// separately. That is not the same as interpolating the composed
    /// world matrices, and it is the accurate model of a hierarchy in
    /// motion.
    ///
    /// # Outside the sampled range, the end sample is held
    ///
    /// Not extrapolated, and not refused -- because that is what
    /// 3Delight does, and a backend that differed would render a
    /// different picture. Rendered with samples at `t=0` and `t=1` and
    /// the shutter open over `[-1, 2]`: there is **zero** alpha beyond
    /// the two sampled positions, where extrapolation would sweep half
    /// again as far each way, and a peak at each end 2.7 times the swept
    /// middle, where a third of the shutter is held.
    ///
    /// An earlier version of this refused such a time, on the reasoning
    /// that clamping "answers for a moment the caller never described".
    /// The caller did describe it: they opened the shutter there.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MissingSampleAtTime`] when `time` is not a
    /// number, which names no sample and brackets no pair.
    ///
    /// Also [`ResolveError::MultipleParents`] or [`ResolveError::Cycle`]
    /// from walking the chain.
    pub fn world_transform_interpolated_at(
        &self,
        handle: &str,
        time: f64,
    ) -> Result<[f64; 16], ResolveError> {
        let chain = self.transform_chain(handle)?;
        self.interpolate_along(&chain, time)
    }

    /// Compose an already-walked path with each node interpolated at
    /// `time`.
    ///
    /// The interpolating twin of [`Scene::compose_along`], and shared
    /// the same way: `placements_at` had a verbatim copy of this fold,
    /// which is the drift that copy was introduced to prevent. Nothing
    /// constrained it -- reversing the multiplication and reversing the
    /// path both left the suite green, because every fixture that
    /// reached it used translations, which commute.
    fn interpolate_along(
        &self,
        path: &[String],
        time: f64,
    ) -> Result<[f64; 16], ResolveError> {
        path.iter().try_fold(IDENTITY, |matrix, node| {
            Ok(match self.local_transform_interpolated_at(node, time)? {
                Some(local) => mul(matrix, local),
                None => matrix,
            })
        })
    }

    /// Compose the transform chain applying to `handle` at `time`.
    ///
    /// A node with no motion samples is constant, so its static matrix
    /// applies at every time. A node that *is* sampled contributes the
    /// sample at exactly `time`, and having none there is an error
    /// rather than an interpolation; see
    /// [`ResolveError::MissingSampleAtTime`].
    ///
    /// A wholly static chain resolves at any time, and agrees with
    /// [`Scene::world_transform`].
    ///
    /// # Errors
    ///
    /// Every variant of [`ResolveError`].
    pub fn world_transform_at(
        &self,
        handle: &str,
        time: f64,
    ) -> Result<[f64; 16], ResolveError> {
        let chain = self.transform_chain(handle)?;

        chain.iter().try_fold(IDENTITY, |matrix, node| {
            Ok(match self.local_transform_at(node, time)? {
                Some(local) => mul(matrix, local),
                None => matrix,
            })
        })
    }

    /// The world transform of `handle` at each of its
    /// [`Scene::motion_times`].
    ///
    /// This is the shape a renderer wants for motion blur: the sample
    /// times, and the composed matrix at each. Empty for a static chain.
    ///
    /// # Errors
    ///
    /// Every variant of [`ResolveError`]. `MissingSampleAtTime` means
    /// the chain mixes nodes sampled at different times, which has no
    /// answer without interpolation.
    pub fn world_transform_samples(
        &self,
        handle: &str,
    ) -> Result<Vec<(f64, [f64; 16])>, ResolveError> {
        self.motion_times(handle)?
            .into_iter()
            .map(|time| Ok((time, self.world_transform_at(handle, time)?)))
            .collect()
    }

    /// Whether any motion sample on `handle` sets a transform.
    ///
    /// Read before composing, because [`Scene::world_transform`] reads
    /// static attributes only and a motion-sampled node has none.
    fn has_motion_transform(&self, handle: &str) -> bool {
        // The typing rule applies here too: a node whose samples are
        // all unreadable, or whose last one is, has no motion -- and
        // reporting motion for it made `world_transform` refuse a node
        // the other accessors resolve to identity.
        self.node(handle).is_some_and(|node| {
            // No allocation: this runs per node per fold.
            sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
                matrix_of(arg).is_some()
            })
            .samples(TRANSFORMATION_MATRIX)
            .next()
            .is_some()
        })
    }

    /// Gather the `attributes` nodes gathered along a geometry's path,
    /// and the surface shader they resolve to.
    ///
    /// ɴsɪ routes material through an intermediate node --
    /// `shader -> attributes -> geometry` -- that no target renderer
    /// has. Mitsuba wants a `bsdf` on the shape; MoonRay wants a `Layer`
    /// entry. Both need the same walk, so it happens here once.
    ///
    /// # Gathering
    ///
    /// ɴsɪ gathers attribute values "along the path starting from the
    /// geometric primitive, through all the transform nodes it is
    /// connected to, until the scene root is reached", and *every*
    /// `attributes` node on that path is considered: "one attributes
    /// node can set object visibility and another can set the surface
    /// shader". So this returns all of them, nearest the geometry
    /// first -- ɴsɪ selects "the definition that is the closest to the
    /// geometric primitive" -- and a backend takes the first that
    /// defines the attribute it wants.
    ///
    /// **That is not the whole rule.** ɴsɪ ranks a definition by
    /// `ATTR.priority`, set on an `attributes` node beside the attribute
    /// it applies to; and at equal priority a more specific
    /// `visibility.<ray>` beats `visibility`. This method applies
    /// neither -- it orders nodes, not the attributes on them. Ask
    /// [`Scene::attribute_value`] for one attribute's value and both
    /// rules are applied.
    ///
    /// Returns `Ok(None)` for geometry with nothing bound anywhere on
    /// its path.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MultipleParents`], [`ResolveError::Cycle`] or
    /// [`ResolveError::Detached`], from walking the path. A
    /// motion-sampled transform does not affect which attributes bind,
    /// so it is not an error here.
    pub fn geometry_binding(
        &self,
        geometry: &str,
    ) -> Result<Option<Binding>, ResolveError> {
        let gathered = self.gathered_attributes(geometry)?;

        if gathered.is_empty() {
            Ok(None)
        } else {
            // A shader connection carries its own priority, "used in
            // the same way as for regular attributes".
            let shader = |kind: &EdgeKind| self.shader_on(&gathered, kind);

            Ok(Some(Binding {
                surface_shader: shader(&EdgeKind::SurfaceShader),
                displacement_shader: shader(&EdgeKind::DisplacementShader),
                volume_shader: shader(&EdgeKind::VolumeShader),
                attributes: gathered
                    .iter()
                    .map(|(_, _, edge)| edge.from.clone())
                    .collect(),
            }))
        }
    }

    /// The value of one attribute, gathered along a geometry's path by
    /// ɴsɪ's full precedence rule.
    ///
    /// [`Binding::attributes`] hands back the `attributes` nodes and
    /// lets a backend take the first that defines what it wants. That is
    /// right only while no node sets `ATTR.priority` and the attribute
    /// is not a visibility one. This applies both rules and returns the
    /// answer itself.
    ///
    /// # Scope
    ///
    /// This resolves `attributes` nodes reached through
    /// `geometryattributes`. ɴsɪ has a *second* container with a
    /// *different* rule: a `shaderattributes` node is gathered along the
    /// same path, but "priority is given to nodes attached closest to
    /// the geometric primitive, with the highest priority given to
    /// attributes set directly on the geometric primitive", with no
    /// `ATTR.priority` in it at all. Ask
    /// [`Scene::shader_attribute_value`] for those; this one returns
    /// `None` for them.
    ///
    /// # Precedence
    ///
    /// ɴsɪ: "When an attribute is defined multiple times along this
    /// path, the definition with the highest priority is selected. In
    /// case of conflicting priorities, the definition that is the
    /// closest to the geometric primitive [...] is selected." A
    /// definition's priority is the `ATTR.priority` int sitting beside
    /// it on the same `attributes` node, `0` when absent.
    ///
    /// For a `visibility.<ray>` query the default `visibility` is a
    /// candidate too: "When visibility is set both per ray type and with
    /// this default visibility, the attribute with the highest priority
    /// is used. If their priority is the same, the more specific
    /// attribute (i.e. per ray type) is used."
    ///
    /// Candidates therefore rank by priority, then specificity, then the
    /// [`Binding::attributes`] order.
    ///
    /// # Assumptions
    ///
    /// Two orderings the specification does not settle. Both are
    /// recorded in `contracts/resolution.md` rather than left implicit:
    ///
    /// - Specificity is compared *before* proximity, so a distant
    ///   `visibility.camera` beats a nearer plain `visibility` at equal
    ///   priority. ɴsɪ gives the specificity rule without saying whether
    ///   it outranks proximity. Confirmed against 3Delight.
    /// - A priority that is not an `int` is ignored, leaving `0`. That
    ///   includes `int64`, which 3Delight also ignores here.
    ///
    /// # Known divergence
    ///
    /// A node that sets `ATTR.priority` but **not** `ATTR` is a
    /// definition to 3Delight -- of `ATTR` at its default value, with
    /// that priority. Rendered: an `attributes` node carrying only
    /// `visibility.priority` makes the geometry visible even though a
    /// farther node sets `visibility 0`, and it does so at priority `0`
    /// too; a node with no attributes at all does not. This function
    /// skips such a node, because it has no recorded value to return
    /// and this crate does not carry ɴsɪ's per-attribute defaults.
    ///
    /// A backend that cares can look for `<name>.priority` among
    /// [`Binding::attributes`]. Tracked as an `Open` row in
    /// `contracts/resolution.md`.
    ///
    /// # Errors
    ///
    /// As [`Scene::geometry_binding`]: walking the path can fail.
    pub fn attribute_value(
        &self,
        geometry: &str,
        name: &str,
    ) -> Result<Option<AttributeValue<'_>>, ResolveError> {
        // As above: the geometry form is the path form over its chain.
        let chain = self.chain(geometry)?;
        Ok(self.attribute_value_along(&chain, name))
    }

    /// The same, along one [`Placement`]'s path.
    ///
    /// A geometry with more than one parent has no single path, so
    /// [`Scene::attribute_value`] refuses it -- and then the rules it
    /// applies could not be applied to an instanced object at all.
    /// This takes the path from a placement instead.
    ///
    /// Infallible: the path has already been walked, so there is
    /// nothing left to refuse.
    pub fn attribute_value_along(
        &self,
        path: &[String],
        name: &str,
    ) -> Option<AttributeValue<'_>> {
        let gathered = self.gathered_along(path, &EdgeKind::AttributeBinding);
        self.resolve_attribute(&gathered, name)
    }

    /// The same, for the `shaderattributes` container.
    ///
    /// Proximity only, as [`Scene::shader_attribute_value`] explains,
    /// and the path's first node -- the geometry -- still outranks every
    /// container. This is the body
    /// [`Scene::shader_attribute_value`] runs over a geometry's own
    /// chain, not a second copy of the rule.
    pub fn shader_attribute_value_along(
        &self,
        path: &[String],
        name: &str,
    ) -> Option<AttributeValue<'_>> {
        if let Some(geometry) = path.first()
            && let Some((handle, node)) = self.node_entry(geometry)
            && let Some(arg) = effective_attr(node, name)
        {
            return Some(AttributeValue {
                node: handle,
                arg,
                priority: 0,
            });
        }

        for (_, _, edge) in
            self.gathered_along(path, &EdgeKind::ShaderAttributes)
        {
            let Some(node) = self.node(&edge.from) else {
                continue;
            };
            if let Some(arg) = effective_attr(node, name) {
                return Some(AttributeValue {
                    node: &edge.from,
                    arg,
                    priority: 0,
                });
            }
        }
        None
    }

    /// ɴsɪ's attribute precedence over an already-gathered list.
    fn resolve_attribute<'a>(
        &'a self,
        gathered: &[(usize, usize, &'a Edge)],
        name: &str,
    ) -> Option<AttributeValue<'a>> {
        // A per-ray visibility query also matches the default.
        let fallback = name
            .strip_prefix("visibility.")
            .filter(|ray| RAY_TYPES.contains(ray))
            .map(|_| "visibility");

        let mut candidates = Vec::new();
        for (rank, (_, _, edge)) in gathered.iter().enumerate() {
            let Some(node) = self.node(&edge.from) else {
                continue;
            };

            let keys = core::iter::once((1u8, name))
                .chain(fallback.map(|name| (0u8, name)));

            for (specificity, key) in keys {
                let Some(arg) = effective_attr(node, key) else {
                    continue;
                };
                let priority = effective_attr(node, &format!("{key}.priority"))
                    .and_then(priority_value)
                    .unwrap_or(0);

                candidates.push((
                    priority,
                    specificity,
                    rank,
                    AttributeValue {
                        node: &edge.from,
                        arg,
                        priority,
                    },
                ));
            }
        }

        // Highest priority, then the more specific attribute, then the
        // gathered order.
        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2))
        });

        candidates.into_iter().next().map(|(_, _, _, value)| value)
    }

    /// The sources of *shader* attributes for a geometry, in ɴsɪ's
    /// precedence order.
    ///
    /// **The first entry is the geometry itself**, which is a source in
    /// its own right and outranks every container: ɴsɪ gives "the
    /// highest priority ... to attributes set directly on the geometric
    /// primitive". It is listed unconditionally, so a caller walking
    /// this in order and taking the first handle that defines what it
    /// wants gets the same answer as
    /// [`Scene::shader_attribute_value`]. Every later entry is an
    /// `attributes` node.
    ///
    /// After it come ɴsɪ's second attribute container, nearest the
    /// geometry first. The same `attributes` node type reaches it,
    /// through the `shaderattributes` connection rather than
    /// `geometryattributes`, gathered "along the path starting from the
    /// geometric primitive, through all the transform nodes it is
    /// connected to, until the scene root is reached".
    ///
    /// # Errors
    ///
    /// As [`Scene::geometry_binding`]: walking the path can fail.
    pub fn shader_attributes(
        &self,
        geometry: &str,
    ) -> Result<Vec<String>, ResolveError> {
        let mut sources = Vec::new();
        if let Some((handle, _)) = self.node_entry(geometry) {
            sources.push(handle.clone());
        }
        sources.extend(
            self.gathered_containers(geometry, &EdgeKind::ShaderAttributes)?
                .into_iter()
                .map(|(_, _, edge)| edge.from.clone()),
        );
        Ok(sources)
    }

    /// The value of one shader attribute, gathered along a geometry's
    /// path.
    ///
    /// # Precedence
    ///
    /// **Not** the rule [`Scene::attribute_value`] applies. ɴsɪ gives
    /// this container its own, and it is simpler: "Priority is given to
    /// nodes attached closest to the geometric primitive, with the
    /// highest priority given to attributes set directly on the
    /// geometric primitive." There is no `ATTR.priority` here and no
    /// per-ray fallback -- nearest wins, then connection order. Reusing
    /// the `geometryattributes` rule would invent a priority the
    /// specification does not give this node.
    ///
    /// [`AttributeValue::priority`] is therefore always `0`.
    ///
    /// ɴsɪ adds that attributes on such a node "may only have a single
    /// value"; this does not enforce that, and returns what was
    /// recorded.
    ///
    /// # Scope
    ///
    /// ɴsɪ allows these on "geometric primitives, transform nodes or
    /// **set nodes**". All three are walked; see `gathered_containers`
    /// for where a set ranks.
    ///
    /// # Errors
    ///
    /// As [`Scene::geometry_binding`]: walking the path can fail.
    pub fn shader_attribute_value(
        &self,
        geometry: &str,
        name: &str,
    ) -> Result<Option<AttributeValue<'_>>, ResolveError> {
        // The path form *is* this rule; asking about a geometry is
        // asking along its own chain. Kept as one body because a copy
        // of a resolution rule has drifted three times in this crate --
        // `compose_along` from `world_transform`'s fold, `placements_at`
        // from the interpolating one, and these two were next.
        let chain = self.chain(geometry)?;
        Ok(self.shader_attribute_value_along(&chain, name))
    }

    /// Every way `geometry` is placed in the scene.
    ///
    /// [`Scene::world_transform`] and [`Scene::geometry_binding`]
    /// answer for *the* path and refuse a node with more than one
    /// parent, because there is no single answer. This enumerates them
    /// instead: one [`Placement`] per path, in the order the parents
    /// were connected, each with the transform and the binding resolved
    /// along that path.
    ///
    /// A singly-placed geometry yields exactly one placement, agreeing
    /// with those two methods, so a backend can use this alone.
    ///
    /// # Errors
    ///
    /// [`ResolveError::Cycle`] for a path that revisits a node, and
    /// [`ResolveError::Detached`] for a geometry that reaches no root
    /// at all. A node reached only through an `instances` node is
    /// *skipped* rather than refused -- an `instances` node carries one
    /// matrix per instance, which [`Scene::instance_transforms`]
    /// answers; see [`ResolveError::Instanced`].
    pub fn placements(
        &self,
        geometry: &str,
    ) -> Result<Vec<Placement>, ResolveError> {
        self.placements_with(geometry, |path| self.compose_along(path))
    }

    /// The shared body of [`Scene::placements`] and
    /// [`Scene::placements_at`], differing only in how a path composes.
    fn placements_with(
        &self,
        geometry: &str,
        compose: impl Fn(&[String]) -> Result<[f64; 16], ResolveError>,
    ) -> Result<Vec<Placement>, ResolveError> {
        let mut paths = Vec::new();
        self.walk_placements(geometry, &mut paths)?;

        if paths.is_empty() {
            // A prototype reached only through an `instances` node has
            // no *direct* placement, and 3Delight draws it -- the
            // instancer supplies its matrices. Calling that `Detached`
            // said "not in the scene" about something that renders, and
            // contradicted both this function's own documentation and
            // `an_instancing_prototype_is_not_detached`. An empty list
            // is the honest answer: no direct placements, ask
            // `instance_transforms` for the instancer's.
            let instanced = self
                .edges_from(geometry)
                .any(|edge| edge.kind == EdgeKind::InstanceSource);

            return if instanced {
                Ok(Vec::new())
            } else {
                Err(ResolveError::Detached {
                    handle: geometry.to_string(),
                })
            };
        }

        self.build_placements(paths, compose)
    }

    /// Resolve each walked path into a [`Placement`].
    fn build_placements(
        &self,
        paths: Vec<Vec<String>>,
        compose: impl Fn(&[String]) -> Result<[f64; 16], ResolveError>,
    ) -> Result<Vec<Placement>, ResolveError> {
        paths
            .into_iter()
            .map(|path| {
                let transform = compose(&path)?;
                let gathered =
                    self.gathered_along(&path, &EdgeKind::AttributeBinding);
                let binding = if gathered.is_empty() {
                    None
                } else {
                    let shader =
                        |kind: &EdgeKind| self.shader_on(&gathered, kind);
                    Some(Binding {
                        surface_shader: shader(&EdgeKind::SurfaceShader),
                        displacement_shader: shader(
                            &EdgeKind::DisplacementShader,
                        ),
                        volume_shader: shader(&EdgeKind::VolumeShader),
                        attributes: gathered
                            .iter()
                            .map(|(_, _, edge)| edge.from.clone())
                            .collect(),
                    })
                };
                Ok(Placement {
                    path,
                    transform,
                    binding,
                })
            })
            .collect()
    }

    /// Every placement of `geometry`, with each transform interpolated
    /// at `time`.
    ///
    /// [`Scene::placements`] refuses a motion-sampled node and
    /// [`Scene::world_transform_interpolated_at`] refuses a node with
    /// more than one parent, so a geometry with several *moving*
    /// parents had no answer from either. This is that answer.
    ///
    /// A geometry animated by an `instances` node instead -- a crowd or
    /// a particle system, where the *instancer's*
    /// `transformationmatrices` are sampled -- is a different question,
    /// and [`Scene::instance_transforms_at`] answers it. This one
    /// returns an empty list for such a prototype, because it has no
    /// direct placement.
    ///
    /// The end sample is held outside the sampled range, exactly as
    /// [`Scene::world_transform_interpolated_at`] describes.
    ///
    /// # Errors
    ///
    /// As [`Scene::placements`], less
    /// [`ResolveError::MotionSampledTransform`] -- the case this exists
    /// to answer -- plus [`ResolveError::MissingSampleAtTime`] for a
    /// time that names no sample, such as a NaN.
    pub fn placements_at(
        &self,
        geometry: &str,
        time: f64,
    ) -> Result<Vec<Placement>, ResolveError> {
        self.placements_with(geometry, |path| {
            self.interpolate_along(path, time)
        })
    }

    /// Depth-first over every parent, collecting root-ward paths.
    ///
    /// An explicit stack, not recursion: measured, the recursive form
    /// aborted the process on a chain 40 000 deep on the main thread
    /// and 10 000 deep on a spawned one -- and a stack overflow kills
    /// the process rather than returning an error a backend could
    /// handle. `chain` is iterative for the same reason.
    fn walk_placements(
        &self,
        geometry: &str,
        out: &mut Vec<Vec<String>>,
    ) -> Result<(), ResolveError> {
        // (node, how many of its parents have been taken)
        let mut stack: Vec<(String, usize)> = vec![(geometry.to_string(), 0)];
        // The handles on the stack, for the cycle check. A set rather
        // than scanning the stack, which was quadratic in the depth:
        // 523 ms against 14 ms on a 20 000-node chain.
        let mut on_path: HashSet<String> = HashSet::new();
        on_path.insert(geometry.to_string());

        while let Some((current, taken)) = stack.last().cloned() {
            if taken == 0 && current == crate::ROOT {
                out.push(
                    stack.iter().map(|(handle, _)| handle.clone()).collect(),
                );
                stack.pop();
                on_path.remove(&current);
                continue;
            }

            // Only direct placements branch. An `instances` connection
            // is not a path in this sense: the instancer holds a matrix
            // per instance rather than one for the prototype.
            let parent = self
                .edges_from(&current)
                .filter(|edge| edge.kind == EdgeKind::SceneMember)
                .nth(taken)
                .map(|edge| edge.to.clone());

            let Some(parent) = parent else {
                stack.pop();
                on_path.remove(&current);
                continue;
            };

            stack.last_mut().expect("just read").1 += 1;

            if !on_path.insert(parent.clone()) {
                return Err(ResolveError::Cycle { handle: parent });
            }
            stack.push((parent, 0));
        }

        Ok(())
    }

    /// Compose the transforms along an already-walked path.
    ///
    /// The one composition [`Scene::world_transform`] and
    /// [`Scene::placements`] both use, so the two cannot disagree about
    /// multiplication order or about refusing a sampled node -- it was
    /// a copy of this fold, and a reversed `mul` in the copy went
    /// unnoticed because every placement fixture used translations,
    /// which commute.
    fn compose_along(
        &self,
        path: &[String],
    ) -> Result<[f64; 16], ResolveError> {
        path.iter().try_fold(IDENTITY, |matrix, node| {
            if self.has_motion_transform(node) {
                Err(ResolveError::MotionSampledTransform {
                    handle: node.clone(),
                })
            } else {
                Ok(match self.local_transform(node) {
                    Some(local) => mul(matrix, local),
                    None => matrix,
                })
            }
        })
    }

    /// The effective value of an integer instancer attribute that was
    /// set with `SetAttributeAtTime`.
    ///
    /// `None` when it was not.
    ///
    /// # Time is not part of the answer
    ///
    /// `modelindices` names a prototype and `disabledinstances` names
    /// instances to skip. 3Delight does not sample either: the last one
    /// **defined** applies for the whole shutter, exactly as an
    /// overwriting `SetAttribute` would. Rendered -- two instances,
    /// `disabledinstances [1]` at `t=0` then `[0]` at `t=1` -- it draws
    /// instance **1**, which is the `t=1` value applying throughout, and
    /// mirroring the values mirrors the result. Moving the shutter to
    /// `[0, 0.25]`, `[2, 3]` or `[-3, -2]` changes nothing.
    ///
    /// An earlier version held the *earlier* sample of the bracketing
    /// pair -- a step -- and documented that as a choice, because the
    /// probe behind it used the same values at both times and could not
    /// tell a step from a blend. It was the wrong choice, and it
    /// returned the discarded sample in every case that discriminates.
    ///
    /// # The one divergence
    ///
    /// 3Delight keys on the last sample *defined*; this keys on the last
    /// by *time*, because `time_attrs` is sorted by time and definition
    /// order is not recorded. A stream setting `t=1` before `t=0` --
    /// which 3Delight itself never writes -- resolves to the other one.
    /// Recorded in `contracts/resolution.md`.
    fn instance_ints(&self, node: &Node, name: &str) -> Option<Vec<i32>> {
        // The shared typing rule, not a sixth hand-rolled copy of it:
        // the last sample that *names* the attribute wins, and a type
        // that cannot be read unsets it. Rendered, a good `int` at
        // `t=0` followed by an `int64` draws *both* instances.
        match sampled_attr(node, name, |arg| {
            matches!(arg.data, OwnedData::I32(_))
        }) {
            Sampled::No => None,
            Sampled::Unset => Some(Vec::new()),
            found @ Sampled::Yes(_) => {
                found.samples(name).last().map(|(_, arg)| match &arg.data {
                    OwnedData::I32(values) => values.to_vec(),
                    _ => Vec::new(),
                })
            }
        }
    }

    /// The instancer's matrices at `time`, when they are sampled.
    ///
    /// `None` when the node has no sampled matrices, in which case the
    /// static ones apply.
    fn instance_matrices_at(
        &self,
        node: &Node,
        instances: &str,
        time: Option<f64>,
    ) -> Result<Option<Vec<f64>>, ResolveError> {
        // The same typing rule: a wrong-typed last sample unsets the
        // matrices, and 3Delight then draws **nothing** rather than the
        // discarded earlier set.
        let mut samples: Vec<(f64, &[f64])> =
            match sampled_attr(node, MATRICES, |arg| {
                matches!(arg.data, OwnedData::F64(_))
            }) {
                // `No` means "use the static value"; `Unset` means
                // "there is none". Indistinguishable today, because
                // `set_attribute` clears the samples of that name and
                // `set_attribute_at_time` clears the static one, so a
                // node never holds both -- swapping these two leaves
                // the suite green, and no reachable scene separates
                // them. Kept apart because they say different things,
                // and the arm that would go wrong if that rule changed
                // is the one that draws instances that should not be
                // there.
                Sampled::No => return Ok(None),
                Sampled::Unset => return Ok(Some(Vec::new())),
                found @ Sampled::Yes(_) => found
                    .samples(MATRICES)
                    .filter_map(|(t, arg)| match arg.data {
                        OwnedData::F64(ref values) => {
                            Some((t, values.as_slice()))
                        }
                        _ => None,
                    })
                    .collect(),
            };

        // A sample that changes the instance *count* describes a
        // different set, not a moved one, and 3Delight refuses the
        // change rather than the instancer: `E6023 ... incompatible
        // with its definition at previously defined time steps and will
        // be ignored`, after which it renders the first sample's set,
        // static. Rendered with 2 matrices at t=0 and 3 at t=1: two
        // sharp bands at the t=0 positions, no third instance and no
        // blur. Dropping the mismatched samples here reproduces that.
        let width = samples[0].1.len();
        samples.retain(|(_, values)| values.len() == width);

        let Some(time) = time else {
            return Err(ResolveError::MotionSampledTransform {
                handle: instances.to_string(),
            });
        };

        match locate_sample(&samples, time) {
            Some(Located::At(values)) => Ok(Some(values.to_vec())),
            Some(Located::Between(from, to, alpha)) => {
                // Samples of a differing length were dropped above, so
                // the two here agree by construction.
                Ok(Some(
                    from.iter()
                        .zip(*to)
                        .map(|(a, b)| a * (1.0 - alpha) + b * alpha)
                        .collect(),
                ))
            }
            None => Err(ResolveError::MissingSampleAtTime {
                handle: instances.to_string(),
                time,
                available: samples.iter().map(|(t, _)| *t).collect(),
            }),
        }
    }

    /// Every `attributes` node on a geometry's path, in ɴsɪ's
    /// precedence order.
    ///
    /// `(depth, connection order, edge)`, nearest the geometry first.
    /// [`Scene::geometry_binding`] and [`Scene::attribute_value`] share
    /// it so the two cannot disagree about which node outranks which.
    ///
    /// # The `priority` on a `geometryattributes` connection is ignored
    ///
    /// ɴsɪ documents `connect`'s `priority` as "when connecting
    /// attributes nodes, indicates in which order the nodes should be
    /// considered when evaluating the value of an attribute", and this
    /// function used to sort by it. **3Delight does not implement
    /// that.** Rendering `mesh -> xf -> .root`, `visibility 0` on the
    /// geometry's own `attributes` node and `visibility 1` on a node
    /// connected to the transform with `"priority" 10`, 3Delight leaves
    /// the object invisible: proximity wins and the connection priority
    /// does nothing. Moving the same `10` onto the far node as
    /// `visibility.priority` *does* flip it. Six probe scenes agree.
    ///
    /// The prose the renderer follows is the other sentence:
    /// "Connections **(for shaders, essentially)** can also be assigned
    /// priorities". A priority on a *shader* connection is honoured, and
    /// `shader_on` still applies it; one on the `geometryattributes`
    /// connection is not.
    fn gathered_attributes(
        &self,
        geometry: &str,
    ) -> Result<Vec<(usize, usize, &Edge)>, ResolveError> {
        self.gathered_containers(geometry, &EdgeKind::AttributeBinding)
    }

    /// The same walk for any container class, nearest the geometry
    /// first.
    ///
    /// # Where a `set` ranks
    ///
    /// ɴsɪ describes gathering as running "from the geometric
    /// primitive, through all the transform nodes it is connected to,
    /// until the scene root is reached", and names `set` nodes only as
    /// a place a `shaderattributes` node may hang. 3Delight honours a
    /// container on a set for **both** classes, and the rule is per node
    /// on that path rather than for the geometry alone: each node
    /// contributes its own containers, then those on the sets it is
    /// *directly* a member of. Rendered, each direction mirrored so no
    /// answer is an artefact of which value happened to be `0`:
    ///
    /// - a node's own container beats one on a set it belongs to;
    /// - a set of the geometry beats a set of its transform;
    /// - a set of a transform beats that transform's parent, and beats
    ///   a container on `.root`;
    /// - with two memberships the first connection wins;
    /// - a set nested inside another set contributes **nothing** --
    ///   only direct membership counts;
    /// - a set holding two nodes of the chain is one source, at its
    ///   nearest occurrence.
    ///
    /// `ATTR.priority` still outranks all of it, as everywhere else.
    fn gathered_containers(
        &self,
        geometry: &str,
        kind: &EdgeKind,
    ) -> Result<Vec<(usize, usize, &Edge)>, ResolveError> {
        let chain = self.chain(geometry)?;
        Ok(self.gathered_along(&chain, kind))
    }

    /// The same, along a path already walked.
    ///
    /// Split out because a geometry with more than one parent has more
    /// than one path, and the containers on each are **different** --
    /// rendered, two parents carrying `visibility 1` and `visibility 0`
    /// draw one copy, not two or none.
    fn gathered_along(
        &self,
        chain: &[String],
        kind: &EdgeKind,
    ) -> Vec<(usize, usize, &Edge)> {
        // The geometry, then the sets it belongs to directly, then the
        // transforms above it. With no set memberships this is `chain`
        // and the walk is unchanged.
        let mut sources: Vec<&String> = Vec::with_capacity(chain.len() + 1);
        for node in chain {
            sources.push(node);
            sources.extend(
                self.edges_from(node.as_str())
                    .filter(|edge| edge.kind == EdgeKind::SetMember)
                    .map(|edge| &edge.to),
            );
        }

        // One set can hold several nodes on the chain. It is a single
        // source at its nearest occurrence -- rendered, a set holding
        // both the mesh and its transform ranks where the mesh does.
        let mut seen = HashSet::new();
        sources.retain(|handle| seen.insert(handle.as_str()));

        let mut gathered: Vec<(usize, usize, &Edge)> = sources
            .into_iter()
            .enumerate()
            .flat_map(|(depth, node)| {
                self.edges_to_attr(node.as_str(), kind.to_attr())
                    // A shader-network edge's `to_attr` is its *port*
                    // name, so it shares this bucket with the named
                    // class. Without the filter a port called
                    // `geometryattributes` resolved as a binding.
                    .filter(move |edge| edge.kind == *kind)
                    .enumerate()
                    .map(move |(order, edge)| (depth, order, edge))
            })
            .collect::<Vec<_>>();

        // Nearest the geometry, then connection order. `chain` already
        // runs geometry-first, so this states the invariant rather than
        // establishing it.
        gathered.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        gathered
    }

    /// The shader of one kind reached from any gathered `attributes`
    /// node, by ɴsɪ's precedence.
    ///
    /// Ranked by the *connection's* priority first -- ɴsɪ calls that
    /// "useful for overriding a shader from higher in the scene graph"
    /// -- then by the gathered order, so this agrees with
    /// [`Binding::attributes`] rather than disagreeing with it.
    fn shader_on(
        &self,
        gathered: &[(usize, usize, &Edge)],
        kind: &EdgeKind,
    ) -> Option<String> {
        gathered
            .iter()
            .enumerate()
            .flat_map(|(rank, (_, _, edge))| {
                self.edges_to_attr(&edge.from, kind.to_attr())
                    .filter(move |shader| shader.kind == *kind)
                    .map(move |shader| (shader.priority(), rank, shader))
            })
            // Highest priority, then earliest in the gathered order.
            .min_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)))
            .map(|(_, _, shader)| shader.from.clone())
    }

    /// Resolve ɴsɪ's output chain into what a renderer actually needs.
    ///
    /// ɴsɪ spreads this over four nodes and three connection classes —
    /// `outputdriver -> outputlayer -> screen -> camera` — where both
    /// targets want it collapsed: Mitsuba into a `Sensor` with a `Film`,
    /// MoonRay into `RenderOutput`s. The walk is the same either way.
    ///
    /// One entry per screen, since a screen is what pairs a camera with
    /// a resolution. A screen with no layers still yields an entry: the
    /// camera and resolution are meaningful on their own.
    ///
    /// Layers and drivers come back in connection order, which is
    /// insertion order in `edges`, so AOV order is the order the
    /// consumer declared.
    pub fn render_outputs(&self) -> Vec<RenderOutput> {
        self.edges()
            .filter(|edge| edge.kind == EdgeKind::Screen)
            .map(|screen_edge| {
                let screen = &screen_edge.from;
                let layers = self
                    .edges_to_attr(screen, EdgeKind::OutputLayer.to_attr())
                    .filter(|edge| edge.kind == EdgeKind::OutputLayer)
                    .map(|layer_edge| OutputLayer {
                        handle: layer_edge.from.clone(),
                        drivers: self
                            .edges_to_attr(
                                &layer_edge.from,
                                EdgeKind::OutputDriver.to_attr(),
                            )
                            .filter(|edge| edge.kind == EdgeKind::OutputDriver)
                            .map(|edge| edge.from.clone())
                            .collect(),
                    })
                    .collect();

                RenderOutput {
                    camera: screen_edge.to.clone(),
                    screen: screen.clone(),
                    layers,
                }
            })
            .collect()
    }

    /// The prototypes an `instances` node draws from, in connection
    /// order.
    ///
    /// Mitsuba turns these into a `shapegroup` referenced by `instance`
    /// shapes; MoonRay into a `GeometrySet`. Both start from this list.
    /// ɴsɪ: connections "must have an integer index attribute if there
    /// are several, so the models effectively form an ordered list", and
    /// an `instances` node's `modelindices` "is matched to the index
    /// attribute of the model connection". So the list is ordered by
    /// that index, not by connection order -- a backend indexes into it.
    /// Connections without one share index `0` and keep their relative
    /// connection order.
    pub fn instance_sources(&self, instances: &str) -> Vec<String> {
        self.sorted_instance_sources(instances)
            .into_iter()
            .map(|(_, from)| from)
            .collect()
    }

    /// Compose the transform chain from `handle` up to, but excluding,
    /// `ancestor`.
    ///
    /// The transform an instancing prototype needs: its subtree cannot
    /// be resolved to world space, because the `instances` node placing
    /// it holds one matrix per instance, but it *can* be resolved
    /// relative to the prototype root that the instance transform is
    /// then applied to.
    ///
    /// `relative_transform(h, ROOT)` is [`Scene::world_transform`].
    ///
    /// # Errors
    ///
    /// [`ResolveError::NotAnAncestor`] when `ancestor` is not on
    /// `handle`'s chain, plus the usual walk errors.
    pub fn relative_transform(
        &self,
        handle: &str,
        ancestor: &str,
    ) -> Result<[f64; 16], ResolveError> {
        let chain = self.linked_chain(handle, true)?;

        let depth = chain
            .iter()
            .position(|link| link.handle == ancestor)
            .ok_or_else(|| ResolveError::NotAnAncestor {
                handle: handle.to_string(),
                ancestor: ancestor.to_string(),
            })?;

        // Composing across an `instances` node would fold in the
        // instancer's own matrix and leave out the per-instance one --
        // a plausible wrong answer, which is the thing this crate is
        // for refusing. Stopping *at* the instancer is the supported
        // case and is what a backend asks for.
        // Only a *crossing* is wrong. Stopping at the instancer means
        // its matrix is never composed, which is exactly the query a
        // backend makes to place a prototype's subtree.
        if let Some(crossed) = chain[..depth]
            .iter()
            .enumerate()
            .position(|(hop, link)| link.via_instancer && hop + 1 < depth)
        {
            return Err(ResolveError::Instanced {
                instancer: chain[crossed + 1].handle.clone(),
            });
        }

        chain[..depth].iter().try_fold(IDENTITY, |matrix, link| {
            let node = &link.handle;
            if self.has_motion_transform(node) {
                Err(ResolveError::MotionSampledTransform {
                    handle: node.clone(),
                })
            } else {
                Ok(match self.local_transform(node) {
                    Some(local) => mul(matrix, local),
                    None => matrix,
                })
            }
        })
    }

    /// The instances an `instances` node places.
    ///
    /// Reads ɴsɪ's `transformationmatrices` for the per-instance
    /// transforms, `modelindices` for which prototype each draws, and
    /// `disabledinstances` for the ones to skip. An instance whose model
    /// index is negative is omitted too: ɴsɪ says "a negative value will
    /// cause an instance to not be rendered".
    ///
    /// Empty when the node carries no `transformationmatrices` at all.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MalformedInstanceMatrices`] when the matrix
    /// buffer is not a whole number of 4x4s, and
    /// [`ResolveError::UnknownModelIndex`] when a `modelindices` entry
    /// matches no prototype connection. Both were silent drops, which is
    /// the failure mode this crate exists to refuse.
    ///
    /// [`ResolveError::MotionSampledTransform`] when the matrices are
    /// *sampled* rather than static: this read only the static
    /// attributes, so a moving instancer -- which 3Delight renders --
    /// came back as an empty list, indistinguishable from "no
    /// instances". Ask [`Scene::instance_transforms_at`] for those.
    pub fn instance_transforms(
        &self,
        instances: &str,
    ) -> Result<Vec<Instance>, ResolveError> {
        self.instances_with(instances, None)
    }

    /// The same, with the instance matrices interpolated at `time`.
    ///
    /// ɴsɪ animates a whole instancer by sampling
    /// `transformationmatrices`, which is how a crowd or a particle
    /// system moves. Interpolated element-wise between the bracketing
    /// samples and held outside them, exactly as
    /// [`Scene::world_transform_interpolated_at`] describes.
    ///
    /// # Errors
    ///
    /// As [`Scene::instance_transforms`], less
    /// [`ResolveError::MotionSampledTransform`]; plus
    /// [`ResolveError::MissingSampleAtTime`] for a time that names no
    /// sample, such as a NaN.
    pub fn instance_transforms_at(
        &self,
        instances: &str,
        time: f64,
    ) -> Result<Vec<Instance>, ResolveError> {
        self.instances_with(instances, Some(time))
    }

    /// The shared body: `time` is `None` for the static reading.
    fn instances_with(
        &self,
        instances: &str,
        time: Option<f64>,
    ) -> Result<Vec<Instance>, ResolveError> {
        let Some(node) = self.node(instances) else {
            return Ok(Vec::new());
        };

        let sampled = self.instance_matrices_at(node, instances, time)?;
        let matrices: &[f64] = match (&sampled, node.attrs.get(MATRICES)) {
            (Some(values), _) => values,
            (None, Some(arg)) => match &arg.data {
                OwnedData::F64(values) => values.as_slice(),
                _ => &[],
            },
            (None, None) => &[],
        };

        // `modelindices` and `disabledinstances` can be sampled too,
        // and 3Delight honours them: an instancer whose
        // `disabledinstances` is set only through `SetAttributeAtTime`
        // renders the same one instance as the static form, and a
        // sampled `modelindices` selects the same prototype. Reading
        // only `attrs` reported every instance as enabled and drawn from
        // source 0 -- the same silent-empty class as the matrices, one
        // level down.
        let sampled_models = self.instance_ints(node, MODEL_INDICES);
        let model_indices: &[i32] =
            match (&sampled_models, node.attrs.get(MODEL_INDICES)) {
                (Some(values), _) => values,
                (None, Some(arg)) => match &arg.data {
                    OwnedData::I32(values) => values.as_slice(),
                    _ => &[],
                },
                (None, None) => &[],
            };

        let sampled_disabled = self.instance_ints(node, DISABLED);
        let disabled: &[i32] =
            match (&sampled_disabled, node.attrs.get(DISABLED)) {
                (Some(values), _) => values,
                (None, Some(arg)) => match &arg.data {
                    OwnedData::I32(values) => values.as_slice(),
                    _ => &[],
                },
                (None, None) => &[],
            };

        // `modelindices` names the connection's `index` attribute, so
        // the value has to be looked up rather than used as a position.
        if !matrices.len().is_multiple_of(16) {
            return Err(ResolveError::MalformedInstanceMatrices {
                instances: instances.to_string(),
                values: matrices.len(),
            });
        }

        let sources = self.sorted_instance_sources(instances);

        if let Some(pair) = sources
            .windows(2)
            .find(|pair| pair[0].0 == pair[1].0 && sources.len() > 1)
        {
            return Err(ResolveError::DuplicateModelIndex {
                instances: instances.to_string(),
                index: pair[0].0,
            });
        }

        matrices
            .as_chunks::<16>()
            .0
            .iter()
            .enumerate()
            .filter(|(instance, _)| {
                !disabled.contains(&(i32::try_from(*instance).unwrap_or(-1)))
            })
            .filter_map(|(instance, values)| {
                let model =
                    model_indices.get(instance).copied().unwrap_or_default();
                // ɴsɪ: "a negative value will cause an instance to not
                // be rendered".
                if model < 0 {
                    return None;
                }

                Some(
                    sources
                        .iter()
                        .position(|(index, _)| *index == model)
                        .ok_or_else(|| ResolveError::UnknownModelIndex {
                            instances: instances.to_string(),
                            model,
                        })
                        .map(|source| Instance {
                            source,
                            transform: *values,
                        }),
                )
            })
            .collect()
    }

    /// The prototypes of an `instances` node with their `index`
    /// arguments, ordered as [`Scene::instance_sources`] returns them.
    fn sorted_instance_sources(&self, instances: &str) -> Vec<(i32, String)> {
        let mut sources = self
            .edges_to_attr(instances, EdgeKind::InstanceSource.to_attr())
            .filter(|edge| edge.kind == EdgeKind::InstanceSource)
            .enumerate()
            .map(|(order, edge)| (edge.index(), order, edge.from.clone()))
            .collect::<Vec<_>>();
        sources.sort_by_key(|(index, order, _)| (*index, *order));
        sources
            .into_iter()
            .map(|(index, _, from)| (index, from))
            .collect()
    }

    /// This node's own matrix, if it carries one.
    ///
    /// The node *type* is never consulted, matching classification: ɴsɪ
    /// permits attributes the node type would not imply, and a
    /// `transformationmatrix` on a non-transform node is composed like
    /// any other. See `contracts/resolution.md`.
    fn local_transform(&self, handle: &str) -> Option<[f64; 16]> {
        let node = self.node(handle)?;
        matrix_of(node.attrs.get(TRANSFORMATION_MATRIX)?)
    }

    /// This node's matrix at `time`, interpolated.
    ///
    /// Linear between the bracketing samples, and held at the nearest
    /// sample outside the sampled range -- which is what 3Delight does;
    /// see [`Scene::world_transform_interpolated_at`].
    fn local_transform_interpolated_at(
        &self,
        handle: &str,
        time: f64,
    ) -> Result<Option<[f64; 16]>, ResolveError> {
        let Some(node) = self.node(handle) else {
            return Ok(None);
        };

        // A wrong-typed *last* sample unsets the attribute, so the
        // static value applies -- rendered, 3Delight draws the node at
        // identity rather than at the discarded earlier sample.
        let found = sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
            matrix_of(arg).is_some()
        });
        if matches!(found, Sampled::No | Sampled::Unset) {
            return Ok(self.local_transform(handle));
        }
        let sampled: Vec<(f64, [f64; 16])> = found
            .samples(TRANSFORMATION_MATRIX)
            .filter_map(|(t, arg)| matrix_of(arg).map(|m| (t, m)))
            .collect();

        // Outside the sampled range the end sample is held, because
        // that is what 3Delight does. Rendered: samples at t=0 and t=1
        // with the shutter open over [-1, 2] leaves **zero** alpha
        // beyond the two sampled positions -- an extrapolating renderer
        // would sweep half again as far each way -- with a peak at each
        // end, 2.7x the swept middle, where a third of the shutter is
        // held. `locate_sample` states that rule once.
        match locate_sample(&sampled, time) {
            Some(Located::At(matrix)) => Ok(Some(*matrix)),
            Some(Located::Between(from, to, alpha)) => {
                let mut out = [0.0f64; 16];
                for (index, slot) in out.iter_mut().enumerate() {
                    *slot = from[index] * (1.0 - alpha) + to[index] * alpha;
                }
                Ok(Some(out))
            }
            None => Err(ResolveError::MissingSampleAtTime {
                handle: handle.to_string(),
                time,
                available: sampled.iter().map(|(t, _)| *t).collect(),
            }),
        }
    }

    fn local_transform_at(
        &self,
        handle: &str,
        time: f64,
    ) -> Result<Option<[f64; 16]>, ResolveError> {
        // `-0.0` names the sample at `0.0` here as well. This scan is a
        // third statement of the exact-hit lookup -- it cannot use
        // `locate_sample`, which clamps and interpolates where this must
        // refuse -- so folding in one place left the two accessors
        // disagreeing: `world_transform_interpolated_at` answered while
        // `world_transform_at` named a sample the recorder had folded.
        let time = time + 0.0;
        let Some(node) = self.node(handle) else {
            return Ok(None);
        };

        // Same typing rule as the interpolating twin: a wrong-typed
        // last sample unsets the attribute at *every* time, rather than
        // leaving this to error at one time and answer the discarded
        // sample at another. That inconsistency had one scene giving
        // three different answers across the three accessors.
        let found = sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
            matrix_of(arg).is_some()
        });
        if matches!(found, Sampled::No | Sampled::Unset) {
            return Ok(self.local_transform(handle));
        }

        match found
            .samples(TRANSFORMATION_MATRIX)
            .find(|(t, _)| t.total_cmp(&time) == Ordering::Equal)
        {
            Some((_, arg)) => Ok(matrix_of(arg)),
            None => Err(ResolveError::MissingSampleAtTime {
                handle: handle.to_string(),
                time,
                available: found
                    .samples(TRANSFORMATION_MATRIX)
                    .map(|(t, _)| t)
                    .collect(),
            }),
        }
    }
}

/// A `transformationmatrix` argument as a row-major 4x4.
///
/// Non-`f64` matrices yield `None`: ɴsɪ documents the attribute as
/// `doublematrix`, and silently reinterpreting an `f32` one would be
/// worse than skipping it.
fn matrix_of(arg: &OwnedArg) -> Option<[f64; 16]> {
    // The declared type, not just the payload: sixteen `double`s are
    // not a `doublematrix`. Rendered, `"transformationmatrix" "double"
    // 16 [...]` is `E6007` and the node draws at identity, while this
    // read it as a matrix -- and since six sites now share this
    // predicate, that one leniency defined the rule everywhere.
    if arg.type_tag != Type::MatrixF64 {
        return None;
    }

    match &arg.data {
        OwnedData::F64(values) if values.len() == 16 => {
            let mut matrix = [0.0; 16];
            matrix.copy_from_slice(values);
            Some(matrix)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
