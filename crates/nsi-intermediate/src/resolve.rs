//! Graph rewrites: turning ɴsɪ's scene-graph semantics into the flat
//! facts a renderer wants.
//!
//! Both target renderers need this and neither should re-derive it.
//! Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
//! resolves geometry to world space too. So the chain has to be
//! composed here, once.

use crate::{Edge, EdgeKind, OwnedArg, OwnedData, Scene};
use core::{cmp::Ordering, fmt};
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

/// Why a scene could not be resolved into flat facts.
///
/// Every variant is a scene ɴsɪ permits but this crate refuses to guess
/// about. Returning a matrix or a binding anyway would be the silent
/// failure the crate exists to prevent: it renders, with the wrong
/// answer.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ResolveError {
    /// A node is connected to more than one parent through `objects`.
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
    /// A node in the chain carries a motion-sampled
    /// `transformationmatrix`.
    ///
    /// Composing per sample is not implemented, and answering with the
    /// static transform would hand a motion-blurred scene back its
    /// unblurred pose.
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
    /// A node in the chain is motion-sampled, but has no sample at the
    /// requested time.
    ///
    /// This crate does not interpolate between samples. Element-wise
    /// interpolation of a matrix is wrong for anything containing a
    /// rotation, and choosing a decomposition here would bake one
    /// renderer's answer into every backend. Ask at a time in
    /// [`Scene::motion_times`], or interpolate in the backend, where the
    /// right decomposition is known.
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
                "ɴsɪ node {handle:?} has a motion-sampled \
                 transformationmatrix; per-sample composition is not \
                 implemented and the static transform would be the wrong \
                 answer"
            ),
            Self::Cycle { handle } => write!(
                f,
                "ɴsɪ transform chain revisits node {handle:?}; a cyclic \
                 scene has no world transform"
            ),
            Self::Detached { handle } => write!(
                f,
                "ɴsɪ node {handle:?} is not connected to {root:?}, so it \
                 is not in the scene and has no world transform",
                root = crate::ROOT
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
    /// Every `attributes` node gathered along the path, in ɴsɪ's
    /// precedence order: highest connection priority first, then
    /// nearest the geometry.
    ///
    /// ɴsɪ gathers attributes along the whole path and considers
    /// *every* node on it -- "one attributes node can set object
    /// visibility and another can set the surface shader" -- so this is
    /// a list, not a winner. A backend looking for one attribute takes
    /// the first node in this list that defines it.
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
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = handle.to_string();

        loop {
            if !seen.insert(current.clone()) {
                return Err(ResolveError::Cycle { handle: current });
            }

            let mut parents = self.edges.iter().filter(|edge| {
                edge.from == current && edge.kind == EdgeKind::SceneMember
            });

            if current == crate::ROOT {
                chain.push(current);
                break;
            }

            let Some(first) = parents.next().or_else(|| {
                // An instancing prototype is connected to an
                // `instances` node through `sourcemodels`, never to
                // `.root` directly. Its attributes are still gathered
                // through that node, so the walk continues there rather
                // than calling the prototype detached.
                self.edges.iter().find(|edge| {
                    edge.from == current
                        && edge.kind == EdgeKind::InstanceSource
                })
            }) else {
                // No parent at all, and we never reached the root.
                return Err(ResolveError::Detached { handle: current });
            };

            // ɴsɪ's lightweight instancing. Refuse rather than answer
            // for whichever parent happened to be connected first.
            let rest = parents.map(|edge| edge.to.clone()).collect::<Vec<_>>();
            if !rest.is_empty() {
                let parents =
                    core::iter::once(first.to.clone()).chain(rest).collect();
                return Err(ResolveError::MultipleParents {
                    handle: current,
                    parents,
                });
            }

            let parent = first.to.clone();
            chain.push(current);
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
        let chain = self.chain(handle)?;

        chain.iter().try_fold(IDENTITY, |matrix, node| {
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

    /// Every time at which a transform in `handle`'s chain is sampled,
    /// ascending, deduplicated.
    ///
    /// Empty for a wholly static chain, which is the check a backend
    /// makes to decide between [`Scene::world_transform`] and
    /// [`Scene::world_transform_samples`].
    ///
    /// # Errors
    ///
    /// [`ResolveError::MultipleParents`] or [`ResolveError::Cycle`].
    pub fn motion_times(&self, handle: &str) -> Result<Vec<f64>, ResolveError> {
        let chain = self.chain(handle)?;

        let mut times = chain
            .iter()
            .filter_map(|node| self.nodes.get(node))
            .flat_map(|node| node.time_attrs.iter())
            .filter(|(_, attrs)| attrs.contains_key(TRANSFORMATION_MATRIX))
            .map(|(time, _)| *time)
            .collect::<Vec<_>>();

        // `total_cmp` throughout, matching how the samples were keyed.
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| a.total_cmp(b) == Ordering::Equal);

        Ok(times)
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
        let chain = self.chain(handle)?;

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
        self.nodes.get(handle).is_some_and(|node| {
            node.time_attrs
                .iter()
                .any(|(_, attrs)| attrs.contains_key(TRANSFORMATION_MATRIX))
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
    /// shader". So this returns all of them, ordered by ɴsɪ's own rule
    /// -- "the definition with the highest priority is selected. In case
    /// of conflicting priorities, the definition that is the closest to
    /// the geometric primitive" -- and a backend takes the first that
    /// defines the attribute it wants.
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
        let chain = self.chain(geometry)?;

        // (priority, depth, connection order, handle) for every
        // `attributes` node anywhere on the path.
        let mut gathered = chain
            .iter()
            .enumerate()
            .flat_map(|(depth, node)| {
                self.edges
                    .iter()
                    .enumerate()
                    .filter(move |(_, edge)| {
                        edge.to == *node
                            && edge.kind == EdgeKind::AttributeBinding
                    })
                    .map(move |(order, edge)| {
                        (edge.priority(), depth, order, edge)
                    })
            })
            .collect::<Vec<_>>();

        // Highest priority first, then nearest the geometry, then
        // connection order.
        gathered.sort_by(|a, b| {
            b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))
        });

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
                    .map(|(_, _, _, edge)| edge.from.clone())
                    .collect(),
            }))
        }
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
        gathered: &[(i32, usize, usize, &Edge)],
        kind: &EdgeKind,
    ) -> Option<String> {
        gathered
            .iter()
            .enumerate()
            .flat_map(|(rank, (_, _, _, edge))| {
                self.edges
                    .iter()
                    .filter(move |shader| {
                        shader.to == edge.from && shader.kind == *kind
                    })
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
        self.edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Screen)
            .map(|screen_edge| {
                let screen = &screen_edge.from;
                let layers = self
                    .edges
                    .iter()
                    .filter(|edge| {
                        edge.to == *screen && edge.kind == EdgeKind::OutputLayer
                    })
                    .map(|layer_edge| OutputLayer {
                        handle: layer_edge.from.clone(),
                        drivers: self
                            .edges
                            .iter()
                            .filter(|edge| {
                                edge.to == layer_edge.from
                                    && edge.kind == EdgeKind::OutputDriver
                            })
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
    pub fn instance_sources(&self, instances: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|edge| {
                edge.to == instances && edge.kind == EdgeKind::InstanceSource
            })
            .map(|edge| edge.from.clone())
            .collect()
    }

    /// This node's own matrix, if it carries one.
    ///
    /// The node *type* is never consulted, matching classification: ɴsɪ
    /// permits attributes the node type would not imply, and a
    /// `transformationmatrix` on a non-transform node is composed like
    /// any other. See `contracts/resolution.md`.
    fn local_transform(&self, handle: &str) -> Option<[f64; 16]> {
        let node = self.nodes.get(handle)?;
        matrix_of(node.attrs.get(TRANSFORMATION_MATRIX)?)
    }

    /// This node's matrix at `time`.
    ///
    /// A node with no transform samples is constant: its static matrix
    /// applies at every time. A sampled node must have a sample at
    /// exactly `time`; this crate does not interpolate.
    fn local_transform_at(
        &self,
        handle: &str,
        time: f64,
    ) -> Result<Option<[f64; 16]>, ResolveError> {
        let Some(node) = self.nodes.get(handle) else {
            return Ok(None);
        };

        let mut sampled = node
            .time_attrs
            .iter()
            .filter(|(_, attrs)| attrs.contains_key(TRANSFORMATION_MATRIX))
            .peekable();

        if sampled.peek().is_none() {
            Ok(self.local_transform(handle))
        } else {
            match sampled
                .clone()
                .find(|(t, _)| t.total_cmp(&time) == Ordering::Equal)
            {
                Some((_, attrs)) => {
                    Ok(matrix_of(&attrs[TRANSFORMATION_MATRIX]))
                }
                None => Err(ResolveError::MissingSampleAtTime {
                    handle: handle.to_string(),
                    time,
                    available: sampled.map(|(t, _)| *t).collect(),
                }),
            }
        }
    }
}

/// A `transformationmatrix` argument as a row-major 4x4.
///
/// Non-`f64` matrices yield `None`: ɴsɪ documents the attribute as
/// `doublematrix`, and silently reinterpreting an `f32` one would be
/// worse than skipping it.
fn matrix_of(arg: &OwnedArg) -> Option<[f64; 16]> {
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
mod tests {
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
        scene.set_attribute("xf", vec![translate(1.0, 2.0, 3.0)]);
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
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform").unwrap();
        scene.set_attribute("inner", vec![translate(1.0, 0.0, 0.0)]);
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
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform").unwrap();
        scene.set_attribute("inner", vec![scale(2.0)]);
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
        scene.set_attribute("xf", vec![translate(5.0, 0.0, 0.0)]);
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
        scene.set_attribute("left", vec![translate(1.0, 0.0, 0.0)]);
        scene.create("right", "transform").unwrap();
        scene.set_attribute("right", vec![translate(9.0, 0.0, 0.0)]);
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
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
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
        scene.set_attribute("xf", vec![translate(5.0, 0.0, 0.0)]);
        scene.set_attribute_at_time(
            "xf",
            0.5,
            vec![OwnedArg {
                name: "unrelated".to_string(),
                type_tag: Type::F64,
                array_length: 1,
                flags: 0,
                data: OwnedData::F64(vec![1.0]),
            }],
        );
        scene.connect("xf", None, ".root", "objects").unwrap();
        assert_eq!(scene.world_transform("xf").unwrap()[12], 5.0);
    }

    /// The motion API's reason to exist: two samples give two different
    /// world transforms, where `world_transform` refuses outright.
    #[test]
    fn a_sampled_chain_resolves_per_sample() {
        let mut scene = Scene::default();
        scene.create("xf", "transform").unwrap();
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
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
        scene.set_attribute("grp", vec![translate(100.0, 0.0, 0.0)]);
        scene.create("xf", "transform").unwrap();
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
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
        scene.set_attribute_at_time(
            "outer",
            0.5,
            vec![translate(1.0, 0.0, 0.0)],
        );
        scene.set_attribute_at_time(
            "outer",
            0.0,
            vec![translate(0.0, 0.0, 0.0)],
        );
        scene.create("inner", "transform").unwrap();
        scene.set_attribute_at_time(
            "inner",
            2.0,
            vec![translate(0.0, 0.0, 0.0)],
        );
        scene.set_attribute_at_time(
            "inner",
            0.0,
            vec![translate(0.0, 0.0, 0.0)],
        );
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
        scene.set_attribute("xf", vec![translate(5.0, 0.0, 0.0)]);
        // A motion sample of something that is not a transform. The
        // chain is still static as far as transforms go, and counting
        // this would invent motion blur out of an animated colour.
        scene.set_attribute_at_time(
            "xf",
            0.5,
            vec![OwnedArg {
                name: "unrelated".to_string(),
                type_tag: Type::F64,
                array_length: 1,
                flags: 0,
                data: OwnedData::F64(vec![1.0]),
            }],
        );
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
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 0.5, vec![translate(2.0, 0.0, 0.0)]);
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
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
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
        scene.set_attribute_at_time(
            "outer",
            0.25,
            vec![translate(1.0, 0.0, 0.0)],
        );
        scene.create("inner", "transform").unwrap();
        scene.set_attribute_at_time(
            "inner",
            0.75,
            vec![translate(2.0, 0.0, 0.0)],
        );
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
        scene.set_attribute(
            "xf",
            vec![OwnedArg {
                name: "transformationmatrix".to_string(),
                type_tag: Type::MatrixF32,
                array_length: 1,
                flags: 0,
                data: OwnedData::F32(m),
            }],
        );
        scene.connect("xf", None, ".root", "objects").unwrap();
        assert_eq!(scene.world_transform("xf").unwrap(), super::IDENTITY);
    }
}

#[cfg(test)]
mod binding_tests {
    use crate::{OwnedArg, OwnedData, Scene};
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

    /// ɴsɪ: "the definition with the highest priority is selected",
    /// which overrides proximity.
    #[test]
    fn priority_beats_proximity() {
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
        assert_eq!(binding.attributes[0], "outer");
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
        assert!(scene.world_transform("proto").is_ok());
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
}

#[cfg(test)]
mod output_tests {
    use crate::Scene;

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
}

#[cfg(test)]
mod instance_tests {
    use crate::Scene;

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
}
