//! Graph rewrites: turning ɴsɪ's scene-graph semantics into the flat
//! facts a renderer wants.
//!
//! Both target renderers need this and neither should re-derive it.
//! Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
//! resolves geometry to world space too. So the chain has to be
//! composed here, once.

use crate::{Edge, EdgeKind, Node, OwnedArg, OwnedData, Scene};
use core::{cmp::Ordering, fmt};
use std::collections::HashSet;

mod attributes;
mod chain;
mod instances;
mod motion;
mod outputs;

// The types are here; the walks are in the five modules above. Split
// at 2530 lines against a stated 300-500, along the seams the code
// already had: a chain walk, a time rule, an attribute rule, the
// instancer, and the output chain. `mod.rs` keeps what all five need
// so none of them has to reach sideways.
pub use instances::InstanceIter;
pub use motion::Sampled;

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
    /// The attribute that actually won, which is not always the one
    /// asked for: in [`Scene::attribute_value`] a `visibility.<ray>`
    /// query falls back to the less specific `visibility`, and this is
    /// how a backend tells the two apart.
    /// [`Scene::shader_attribute_value`] performs no such fallback, so
    /// there it always matches the query.
    pub name: &'a str,
    /// The definition itself, or `None` when the winning node carries
    /// only `<name>.priority`.
    ///
    /// 3Delight reads a lone `ATTR.priority` as a definition of `ATTR`
    /// **at its ɴsɪ default**, and it wins on the strength of that
    /// priority; [`Scene::attribute_value`] has the rendered evidence.
    /// This crate does not carry ɴsɪ's per-attribute defaults, so it
    /// names the winner and leaves the value to the backend: `None`
    /// means *the default of [`AttributeValue::name`]*, never
    /// *undefined*.
    pub arg: Option<&'a OwnedArg>,
    /// The `ATTR.priority` that selected it; `0` when none is set.
    pub priority: i32,
}

/// One renderable output: a camera paired with a screen, and the AOVs
/// written from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RenderOutput {
    /// The camera the screen is connected to.
    pub camera: String,
    /// The screen, which is what carries resolution and oversampling.
    pub screen: String,
    /// AOVs in connection order; may be empty.
    pub layers: Vec<OutputLayer>,
}

/// One instance, borrowing the matrix the scene already holds.
///
/// [`Scene::instance_transforms`] copies: for a set-dressing
/// instancer with a million entries that is 136 MB of `Instance`
/// against the 128 MB `doublematrix` array it was copied from.
/// [`Scene::instances`] hands out these instead, which is the shape a
/// renderer's own instancer wants -- a prototype index and a matrix it
/// can read in place.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct InstanceRef<'a> {
    /// Which prototype this instance draws, as a position in
    /// [`Scene::instance_sources`].
    pub source: usize,
    /// This instance's transform, in the `instances` node's space.
    pub transform: &'a [f64; 16],
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
#[non_exhaustive]
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

#[cfg(test)]
mod tests;
