//! Graph rewrites: turning ɴsɪ's scene-graph semantics into the flat
//! facts a renderer wants.
//!
//! Both target renderers need this and neither should re-derive it.
//! Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
//! resolves geometry to world space too. So the chain has to be
//! composed here, once.

use crate::{EdgeKind, OwnedArg, OwnedData, Scene};
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
pub struct Binding {
    /// The `attributes` node handle. Its remaining attributes --
    /// visibility flags in particular -- are read by the backend, which
    /// knows how its renderer encodes them.
    pub attributes: String,
    /// The shader reached through `surfaceshader`, when there is one.
    /// An `attributes` node carrying only visibility has none.
    pub surface_shader: Option<String>,
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
    /// The chain from `handle` up to `.root`, nearest first.
    ///
    /// `handle` is the first entry and `.root` is not an entry. Every
    /// walk up the `objects` hierarchy goes through here, so the three
    /// scenes with no single answer -- more than one parent, a cycle --
    /// are rejected in one place rather than each caller re-deriving
    /// them.
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

            let Some(first) = parents.next() else {
                chain.push(current);
                break;
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
            if parent == crate::ROOT {
                break;
            }
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

    /// Dissolve the `attributes` node bound to a piece of geometry.
    ///
    /// ɴsɪ routes material through an intermediate node —
    /// `shader -> attributes -> geometry` — that neither target renderer
    /// has. Mitsuba wants a `bsdf` on the shape; MoonRay wants a `Layer`
    /// entry. Both need the same two-hop walk, so it happens here once.
    ///
    /// One `attributes` node may bind to many shapes, so this resolves
    /// per geometry rather than producing a single owner.
    ///
    /// # Inheritance
    ///
    /// ɴsɪ binds `geometryattributes` to a transform as readily as to a
    /// geometry, and a binding on a transform applies to everything
    /// beneath it. So the whole chain up to `.root` is searched, not
    /// just the geometry itself, and the winner is picked by:
    ///
    /// 1. highest `"priority"`, ɴsɪ's own tie-breaker;
    /// 2. then the binding nearest the geometry, the more specific one;
    /// 3. then connection order.
    ///
    /// Returns `Ok(None)` for geometry with nothing bound anywhere in
    /// its chain. The attributes handle is returned rather than its
    /// contents because what else lives on that node — visibility flags
    /// above all — is encoded differently by each renderer, and
    /// inventing a common shape for it here would be guesswork.
    ///
    /// # Errors
    ///
    /// [`ResolveError::MultipleParents`] or [`ResolveError::Cycle`],
    /// from walking the chain. A motion-sampled transform does not
    /// affect which attributes bind, so it is not an error here.
    pub fn geometry_binding(
        &self,
        geometry: &str,
    ) -> Result<Option<Binding>, ResolveError> {
        let chain = self.chain(geometry)?;

        let winner = chain
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
                        (edge.priority, depth, order, edge)
                    })
            })
            // Highest priority, then nearest the geometry, then first
            // connected. `depth` and `order` reverse because `max_by`
            // wants the largest and those two want the smallest.
            .max_by(|a, b| {
                a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2))
            });

        Ok(winner.map(|(_, _, _, edge)| {
            let attributes = edge.from.clone();
            let surface_shader = self
                .edges
                .iter()
                .find(|edge| {
                    edge.to == attributes
                        && edge.kind == EdgeKind::SurfaceShader
                })
                .map(|edge| edge.from.clone());

            Binding {
                attributes,
                surface_shader,
            }
        }))
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
        scene.create("mesh", "mesh");
        assert_eq!(scene.world_transform("mesh").unwrap(), super::IDENTITY);
    }

    #[test]
    fn a_single_transform_applies_to_its_child() {
        let mut scene = Scene::default();
        scene.create("xf", "transform");
        scene.set_attribute("xf", vec![translate(1.0, 2.0, 3.0)]);
        scene.create("mesh", "mesh");
        scene.connect("mesh", None, "xf", "objects").unwrap();
        scene.connect("xf", None, ".root", "objects").unwrap();

        let m = scene.world_transform("mesh").unwrap();
        assert_eq!(&m[12..15], &[1.0, 2.0, 3.0]);
    }

    /// Nested translations accumulate.
    #[test]
    fn nested_transforms_compose() {
        let mut scene = Scene::default();
        scene.create("outer", "transform");
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform");
        scene.set_attribute("inner", vec![translate(1.0, 0.0, 0.0)]);
        scene.create("mesh", "mesh");
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
        scene.create("outer", "transform");
        scene.set_attribute("outer", vec![translate(10.0, 0.0, 0.0)]);
        scene.create("inner", "transform");
        scene.set_attribute("inner", vec![scale(2.0)]);
        scene.create("mesh", "mesh");
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
        scene.create("xf", "transform");
        scene.set_attribute("xf", vec![translate(5.0, 0.0, 0.0)]);
        scene.connect("xf", None, ".root", "objects").unwrap();
        assert_eq!(scene.world_transform("xf").unwrap()[12], 5.0);
    }

    /// A cycle must not hang the resolver, and must not answer either.
    /// ɴsɪ does not forbid one; no correct transform exists for it.
    #[test]
    fn a_cycle_is_an_error() {
        let mut scene = Scene::default();
        scene.create("a", "transform");
        scene.create("b", "transform");
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
        scene.create("left", "transform");
        scene.set_attribute("left", vec![translate(1.0, 0.0, 0.0)]);
        scene.create("right", "transform");
        scene.set_attribute("right", vec![translate(9.0, 0.0, 0.0)]);
        scene.create("mesh", "mesh");
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
        scene.create("xf", "transform");
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
        scene.create("mesh", "mesh");
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
        scene.create("xf", "transform");
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
        scene.create("xf", "transform");
        scene.set_attribute_at_time("xf", 0.0, vec![translate(0.0, 0.0, 0.0)]);
        scene.set_attribute_at_time("xf", 1.0, vec![translate(5.0, 0.0, 0.0)]);
        scene.create("mesh", "mesh");
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
        scene.create("grp", "transform");
        scene.set_attribute("grp", vec![translate(100.0, 0.0, 0.0)]);
        scene.create("xf", "transform");
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
        scene.create("outer", "transform");
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
        scene.create("inner", "transform");
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
        scene.create("xf", "transform");
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
        scene.create("xf", "transform");
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
        scene.create("xf", "transform");
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
        scene.create("outer", "transform");
        scene.set_attribute_at_time(
            "outer",
            0.25,
            vec![translate(1.0, 0.0, 0.0)],
        );
        scene.create("inner", "transform");
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
        scene.create("xf", "transform");
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
    use crate::Scene;

    /// The canonical ɴsɪ shape: shader -> attributes -> geometry.
    fn scene_with_material() -> Scene {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        scene.create("attr", "attributes");
        scene.create("shader", "shader");
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
        assert_eq!(binding.attributes, "attr");
        assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
    }

    #[test]
    fn unbound_geometry_has_no_binding() {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        assert!(scene.geometry_binding("mesh").unwrap().is_none());
    }

    /// An attributes node with no shader still binds -- it may carry
    /// only visibility flags.
    #[test]
    fn attributes_without_a_shader_still_bind() {
        let mut scene = Scene::default();
        scene.create("mesh", "mesh");
        scene.create("attr", "attributes");
        scene
            .connect("attr", None, "mesh", "geometryattributes")
            .unwrap();
        let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
        assert_eq!(binding.attributes, "attr");
        assert!(binding.surface_shader.is_none());
    }

    /// One attributes node bound to several shapes must resolve for each
    /// of them. This is the fan-out the spec calls out.
    #[test]
    fn one_attributes_node_fans_out_to_every_shape() {
        let mut scene = Scene::default();
        scene.create("attr", "attributes");
        scene.create("shader", "shader");
        scene
            .connect("shader", None, "attr", "surfaceshader")
            .unwrap();
        for mesh in ["a", "b", "c"] {
            scene.create(mesh, "mesh");
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
        scene.create("grp", "transform");
        scene.create("mesh", "mesh");
        scene.create("attr", "attributes");
        scene.create("shader", "shader");
        scene.connect("mesh", None, "grp", "objects").unwrap();
        scene.connect("grp", None, ".root", "objects").unwrap();
        scene
            .connect("attr", None, "grp", "geometryattributes")
            .unwrap();
        scene
            .connect("shader", None, "attr", "surfaceshader")
            .unwrap();

        let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
        assert_eq!(binding.attributes, "attr");
        assert_eq!(binding.surface_shader.as_deref(), Some("shader"));
    }

    /// At equal priority the more specific binding wins: the one on the
    /// geometry beats the one it inherits from its parent.
    #[test]
    fn the_nearest_binding_wins_at_equal_priority() {
        let mut scene = Scene::default();
        scene.create("grp", "transform");
        scene.create("mesh", "mesh");
        scene.create("outer", "attributes");
        scene.create("own", "attributes");
        scene.connect("mesh", None, "grp", "objects").unwrap();
        scene
            .connect("outer", None, "grp", "geometryattributes")
            .unwrap();
        scene
            .connect("own", None, "mesh", "geometryattributes")
            .unwrap();

        let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
        assert_eq!(binding.attributes, "own");
    }

    /// ɴsɪ's `"priority"` overrides proximity -- that is what it is for.
    /// The ancestor's binding wins over the geometry's own.
    #[test]
    fn priority_beats_proximity() {
        let mut scene = Scene::default();
        scene.create("grp", "transform");
        scene.create("mesh", "mesh");
        scene.create("outer", "attributes");
        scene.create("own", "attributes");
        scene.connect("mesh", None, "grp", "objects").unwrap();
        scene
            .connect_with_priority(
                "outer",
                None,
                "grp",
                "geometryattributes",
                10,
            )
            .unwrap();
        scene
            .connect("own", None, "mesh", "geometryattributes")
            .unwrap();

        let binding = scene.geometry_binding("mesh").unwrap().expect("bound");
        assert_eq!(binding.attributes, "outer");
    }

    /// A cyclic chain has no binding either -- the walk that finds
    /// ancestors is the same one that composes transforms.
    #[test]
    fn a_cycle_is_an_error_for_bindings_too() {
        let mut scene = Scene::default();
        scene.create("a", "transform");
        scene.create("b", "transform");
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
        scene.create("cam", "perspectivecamera");
        scene.create("scr", "screen");
        scene.create("beauty", "outputlayer");
        scene.create("drv", "outputdriver");
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
        scene.create("cam", "perspectivecamera");
        scene.create("scr", "screen");
        scene.connect("scr", None, "cam", "screens").unwrap();
        let outputs = scene.render_outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].layers.is_empty());
    }

    /// Several AOVs on one screen, in connection order.
    #[test]
    fn multiple_layers_keep_connection_order() {
        let mut scene = scene_with_output();
        scene.create("depth", "outputlayer");
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
        scene.create("drv2", "outputdriver");
        scene
            .connect("drv2", None, "beauty", "outputdrivers")
            .unwrap();
        let outputs = scene.render_outputs();
        assert_eq!(outputs[0].layers[0].drivers, vec!["drv", "drv2"]);
    }

    #[test]
    fn no_screen_means_no_outputs() {
        let mut scene = Scene::default();
        scene.create("cam", "perspectivecamera");
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
            scene.create(cam, "perspectivecamera");
            scene.create(scr, "screen");
            scene.create(layer, "outputlayer");
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
        scene.create("inst", "instances");
        scene.create("proto_a", "mesh");
        scene.create("proto_b", "mesh");
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
        scene.create("inst", "instances");
        assert!(scene.instance_sources("inst").is_empty());
    }
}
