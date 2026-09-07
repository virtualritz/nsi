//! Walking the graph: a geometry's chain to `.root`, and the
//! placements a multi-parent geometry has.
//!
//! ɴsɪ's scene graph is a DAG, and every other answer in this module
//! starts by asking which nodes stand between a geometry and the
//! root.

use super::*;

/// One entry of a chain walk, and how it reached its parent.
pub(super) struct ChainLink {
    pub(super) handle: String,
    /// The step to the parent was a `sourcemodels` connection.
    pub(super) via_instancer: bool,
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
    pub(super) fn chain(
        &self,
        handle: &str,
    ) -> Result<Vec<String>, ResolveError> {
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
    pub(super) fn transform_chain(
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
    pub(super) fn linked_chain(
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
                            .map(|edge| edge.to().to_string())
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
                            instancer: instancer.to().to_string(),
                        });
                    }

                    if let Some(second) = instancers.next() {
                        let parents = [instancer, second]
                            .into_iter()
                            .chain(instancers)
                            .map(|edge| edge.to().to_string())
                            .collect();
                        return Err(ResolveError::MultipleParents {
                            handle: current,
                            parents,
                        });
                    }

                    instancer
                }
            };

            let parent = first.to().to_string();
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

    /// The node for `handle`, or `None` for a reserved handle nothing
    /// has been set on yet.
    ///
    /// `.root` and `.global` exist whether or not they were created, so
    /// they have no samples rather than being unknown. Without this the
    /// answer flipped between `Err` and `Ok` depending on whether some
    /// unrelated attribute had been set on `.root` first, and the error
    /// text claimed no node was named `.root`.
    pub(super) fn existing_node(
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

    /// Compose the transforms along an already-walked path.
    ///
    /// The one composition [`Scene::world_transform`] and
    /// [`Scene::placements`] both use, so the two cannot disagree about
    /// multiplication order or about refusing a sampled node -- it was
    /// a copy of this fold, and a reversed `mul` in the copy went
    /// unnoticed because every placement fixture used translations,
    /// which commute.
    pub(super) fn compose_along(
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
    pub(super) fn placements_with(
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
    pub(super) fn build_placements(
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
                            .map(|(_, _, edge)| edge.from().to_string())
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
    pub(super) fn walk_placements(
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
                .map(|edge| edge.to().to_string());

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
}

/// Every node under `.root`, with the world transform composed on the
/// way down.
///
/// Returned by [`Scene::world_transforms`]. One entry per **path**:
/// ɴsɪ's lightweight instancing puts one geometry under several
/// parents, and each of those is a different object to a renderer with
/// a different transform.
pub struct WorldTransforms<'a> {
    scene: &'a Scene,
    /// `Enter` carries the transform of everything above the node;
    /// `Leave` pops it off the ancestor set, which is what makes the
    /// cycle check exact rather than a depth cap.
    stack: Vec<Step<'a>>,
    ancestors: HashSet<&'a str>,
}

enum Step<'a> {
    Enter(&'a str, [f64; 16]),
    Leave(&'a str),
}

impl<'a> Iterator for WorldTransforms<'a> {
    type Item = (&'a str, [f64; 16]);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(step) = self.stack.pop() {
            let (handle, above) = match step {
                Step::Leave(handle) => {
                    self.ancestors.remove(handle);
                    continue;
                }
                Step::Enter(handle, above) => (handle, above),
            };

            // A node already on this path is a cycle, and the
            // single-answer accessors refuse one; the branch stops
            // here rather than looping.
            if !self.ancestors.insert(handle) {
                continue;
            }

            let here = match self.scene.local_transform(handle) {
                Some(local) => mul(above, local),
                None => above,
            };

            self.stack.push(Step::Leave(handle));
            for edge in self.scene.edges_to_attribute(handle, "objects") {
                self.stack.push(Step::Enter(edge.from(), here));
            }

            return Some((handle, here));
        }
        None
    }
}

impl Scene {
    /// Every node under `.root`, with its world transform, composing
    /// each matrix once.
    ///
    /// [`Scene::world_transform`] answers for one geometry by walking
    /// up to `.root`, which is right for one question and quadratic
    /// for the whole scene: measured, a twenty-deep chain with 50 000
    /// geometries under it costs 6.2 us per geometry -- the same
    /// twenty matrices composed fifty thousand times -- of which the
    /// multiply itself is under a tenth. This descends instead,
    /// carrying the accumulated transform down, so each node's matrix
    /// is composed once.
    ///
    /// One entry per path, so a geometry under two parents appears
    /// twice, as it does in [`Scene::placements`] and as it must to a
    /// renderer.
    ///
    /// Nodes are yielded in no particular order beyond a parent
    /// preceding its children. A cycle ends the branch it is on rather
    /// than looping; the single-answer accessors refuse it outright.
    ///
    /// A **motion-sampled** transform contributes its static value
    /// here, which is `None` -- ask [`Scene::world_transform_samples`]
    /// per geometry for those. This is the flat pass a backend makes
    /// once, not the motion pass.
    pub fn world_transforms(&self) -> WorldTransforms<'_> {
        let mut stack = Vec::new();
        for edge in self.edges_to_attribute(crate::ROOT, "objects") {
            stack.push(Step::Enter(edge.from(), IDENTITY));
        }
        WorldTransforms {
            scene: self,
            stack,
            ancestors: HashSet::new(),
        }
    }
}
