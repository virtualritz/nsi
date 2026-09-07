//! ɴsɪ's `instances` node: the matrices it draws its prototypes
//! with, and the per-instance attributes that select them.

use super::{
    motion::{Located, locate_sample, matrices_of, sampled_attr},
    *,
};
use std::collections::{HashMap, HashSet};

impl Scene {
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
    /// The last **defined**, which is not the last by time for a
    /// stream that sets `t=1` before `t=0`. That was a divergence for
    /// as long as the record was a table keyed by time;
    /// [`Node::samples`] is a call log, and this reads the last of
    /// them.
    pub(super) fn instance_ints(
        &self,
        node: &Node,
        name: &str,
    ) -> Option<Vec<i32>> {
        // The shared typing rule, not a sixth hand-rolled copy of it:
        // the last sample that *names* the attribute wins, and a type
        // that cannot be read unsets it. Rendered, a good `int` at
        // `t=0` followed by an `int64` draws *both* instances.
        match sampled_attr(node, name, |arg| {
            matches!(arg.data, OwnedData::I32(_))
        }) {
            Sampled::No => None,
            Sampled::Unset => Some(Vec::new()),
            // The last *defined*, which 3Delight applies for the whole
            // shutter as an overwriting `SetAttribute` would. Taking
            // the last by time answered a different sample for any
            // scene whose samples did not arrive in time order.
            Sampled::Yes { last_defined, .. } => {
                Some(match &last_defined.data {
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
    pub(super) fn instance_matrices_at(
        &self,
        node: &Node,
        instances: &str,
        time: Option<f64>,
    ) -> Result<Option<Vec<f64>>, ResolveError> {
        // The same typing rule: a wrong-typed last sample unsets the
        // matrices, and 3Delight then draws **nothing** rather than the
        // discarded earlier set.
        let mut samples: Vec<(f64, &[f64])> = match sampled_attr(
            node,
            MATRICES,
            |arg| matrices_of(arg).is_some(),
        ) {
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
            found @ Sampled::Yes { .. } => found
                .samples()
                .filter_map(|(t, arg)| Some((t, matrices_of(arg)?)))
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

    /// The instancer's instances, **without copying the matrices**.
    ///
    /// The streaming twin of [`Scene::instance_transforms`], with the
    /// same rules: `modelindices` matched against each connection's
    /// index attribute rather than connection order,
    /// `disabledinstances` applied, and a sampled instancer refused
    /// rather than reported empty -- ask
    /// [`Scene::instance_transforms_at`] for that one.
    ///
    /// This is the shape a renderer's own instancer wants: create one
    /// object per [`Scene::instance_sources`] entry, then feed it
    /// `(source, transform)` pairs. Measured on a million instances,
    /// the copying form spends 110 ms and returns 136 MB against the
    /// 128 MB the scene already holds; this borrows.
    ///
    /// Both per-instance lookups are hashed rather than scanned. The
    /// copying form scans the source list and the disabled list for
    /// every instance, which is fine for the ten-prototype scene it
    /// was written against and quadratic for a set-dressing instancer
    /// with thousands.
    ///
    /// # Errors
    ///
    /// As [`Scene::instance_transforms`].
    pub fn instances(
        &self,
        instances: &str,
    ) -> Result<InstanceIter<'_>, ResolveError> {
        let Some(node) = self.node(instances) else {
            return Ok(InstanceIter::empty());
        };

        // Refuses a sampled instancer exactly as the copying form
        // does, and for the same reason: an interpolated matrix is
        // computed, so there is nothing to borrow.
        self.instance_matrices_at(node, instances, None)?;

        let matrices = node
            .attrs
            .get(MATRICES)
            .and_then(matrices_of)
            .unwrap_or(&[]);
        if !matrices.len().is_multiple_of(16) {
            return Err(ResolveError::MalformedInstanceMatrices {
                instances: instances.to_string(),
                values: matrices.len(),
            });
        }

        let model_indices = match node.attrs.get(MODEL_INDICES) {
            Some(arg) => arg.as_i32s().unwrap_or(&[]),
            None => &[],
        };
        let disabled: HashSet<i32> = match node.attrs.get(DISABLED) {
            Some(arg) => arg.as_i32s().unwrap_or(&[]).iter().copied().collect(),
            None => HashSet::new(),
        };

        let sources = self.sorted_instance_sources(instances);
        let by_model: HashMap<i32, usize> = sources
            .iter()
            .enumerate()
            .map(|(position, (index, _))| (*index, position))
            .collect();

        // Every model index is checked once, here, so iterating is
        // infallible: a stream that yields a `Result` per instance
        // makes every consumer handle an error that is a property of
        // the scene, not of the instance.
        for model in model_indices.iter().copied().filter(|model| *model >= 0) {
            if !by_model.contains_key(&model) {
                return Err(ResolveError::UnknownModelIndex {
                    instances: instances.to_string(),
                    model,
                });
            }
        }

        Ok(InstanceIter {
            matrices: matrices.as_chunks::<16>().0,
            model_indices,
            by_model,
            disabled,
            at: 0,
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
    pub(super) fn instances_with(
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
            (None, Some(arg)) => matrices_of(arg).unwrap_or(&[]),
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
    pub(super) fn sorted_instance_sources(
        &self,
        instances: &str,
    ) -> Vec<(i32, String)> {
        let mut sources = self
            .edges_to_attr(instances, EdgeKind::InstanceSource.to_attr())
            .filter(|edge| edge.kind == EdgeKind::InstanceSource)
            .enumerate()
            .map(|(order, edge)| (edge.index(), order, edge.from().to_string()))
            .collect::<Vec<_>>();
        sources.sort_by_key(|(index, order, _)| (*index, *order));
        sources
            .into_iter()
            .map(|(index, _, from)| (index, from))
            .collect()
    }
}

/// The instances of one `instances` node, borrowed.
///
/// Returned by [`Scene::instances`]. Yields in instance order, with
/// the disabled ones and those whose `modelindices` entry is negative
/// left out -- ɴsɪ says a negative index draws nothing.
pub struct InstanceIter<'a> {
    matrices: &'a [[f64; 16]],
    model_indices: &'a [i32],
    by_model: HashMap<i32, usize>,
    disabled: HashSet<i32>,
    at: usize,
}

impl InstanceIter<'_> {
    fn empty() -> Self {
        Self {
            matrices: &[],
            model_indices: &[],
            by_model: HashMap::new(),
            disabled: HashSet::new(),
            at: 0,
        }
    }
}

impl<'a> Iterator for InstanceIter<'a> {
    type Item = InstanceRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.at < self.matrices.len() {
            let at = self.at;
            self.at += 1;

            if self.disabled.contains(&i32::try_from(at).unwrap_or(-1)) {
                continue;
            }
            let model = self.model_indices.get(at).copied().unwrap_or_default();
            if model < 0 {
                continue;
            }
            // Checked in `Scene::instances`, so this cannot miss.
            let Some(source) = self.by_model.get(&model).copied() else {
                continue;
            };

            return Some(InstanceRef {
                source,
                transform: &self.matrices[at],
            });
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.matrices.len() - self.at))
    }
}
