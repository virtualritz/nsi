//! Time: which samples stand, and what the transform is between
//! them.
//!
//! ɴsɪ's rules here are stated over *calls*, so the typing rule
//! ([`sampled_attr`]) and the hold-the-ends rule ([`locate_sample`])
//! are each written once and read by every accessor that needs them.

use super::*;

/// What ɴsɪ's typing rule makes of one attribute's samples.
///
/// Returned by [`Scene::sampled_attribute`], and what the resolver
/// itself reads for `transformationmatrix` and the instancer's
/// attributes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Sampled<'a> {
    /// Not sampled. The static value applies and [`Node::effective`]
    /// gives it -- **run the same predicate over that too**: the rule
    /// is about the call, not about `SetAttributeAtTime`. Rendered, a
    /// static `"nvertices" "int64" 1` is `E6007`, then `E6020`, and
    /// the mesh does not draw.
    No,
    /// Sampled, but the last call cannot be read as the attribute's
    /// type -- so the attribute is unset, at every time.
    ///
    /// This is not "no samples": 3Delight rejects the argument at the
    /// call and the attribute has no value at all afterwards.
    /// Rendered on a mesh's `P` -- good at `t=0`, a `float` at `t=1` --
    /// it logs `E6007` and then `E6020`, and **draws nothing**. A
    /// backend that fell back to the earlier sample here would draw a
    /// mesh the renderer does not.
    Unset,
    /// The surviving samples.
    ///
    /// Never empty: `keep_from` is one past the last *unreadable*
    /// definition and equals the count only in the `Unset` case, so at
    /// least one survivor is always left. Three call sites carried an
    /// `is_empty` branch for a case that cannot arise; they are gone.
    ///
    /// The name is resolved here rather than at every consumer, which
    /// is what lets the survivors be an arbitrary subset of the
    /// timeline.
    Yes {
        /// In **time** order, which is what interpolation needs.
        samples: Vec<(f64, &'a OwnedArgument)>,
        /// The survivor defined **last**, which is what an attribute
        /// that is not motion data takes for the whole shutter -- and
        /// is not the sample at the greatest time for a scene whose
        /// samples did not arrive in time order.
        last_defined: &'a OwnedArgument,
    },
}

impl<'a> Sampled<'a> {
    /// The surviving samples, in time order.
    pub(super) fn samples(
        self,
    ) -> impl Iterator<Item = (f64, &'a OwnedArgument)> {
        match self {
            Self::Yes { samples, .. } => samples.into_iter(),
            Self::No | Self::Unset => Vec::new().into_iter(),
        }
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
pub(super) fn sampled_attr<'a>(
    node: &'a Node,
    name: &str,
    readable: impl Fn(&OwnedArgument) -> bool,
) -> Sampled<'a> {
    // In **call** order, not time order: 3Delight rejects an unreadable
    // argument at the call, so what survives is what was set after it.
    // `Node::samples` is a call log for exactly this reason; a table
    // keyed by time answers by position on the timeline, which is a
    // different set of survivors for any scene whose samples did not
    // arrive in time order.
    let Some(calls) = node.sample_calls(name) else {
        return Sampled::No;
    };

    let mut keep_from = 0;
    for (index, (_, arg)) in calls.iter().enumerate() {
        if !readable(arg) {
            keep_from = index + 1;
        }
    }

    // The last call is itself unreadable, so nothing survives it and
    // the attribute is unset at every time. Also the empty log a caller
    // could put here by hand.
    let Some((_, last_defined)) = calls[keep_from..].last() else {
        return Sampled::Unset;
    };

    Sampled::Yes {
        // The same-time rule is `latest_per_time`'s, run over the
        // survivors rather than over every call: a re-set that
        // superseded an unreadable sample stands alone, where one that
        // superseded a good sample merely replaces it.
        samples: crate::scene::latest_per_time(&calls[keep_from..]),
        last_defined,
    }
}

/// Where a time falls among a set of samples.
pub(super) enum Located<'a, T> {
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
pub(super) fn locate_sample<T>(
    samples: &[(f64, T)],
    time: f64,
) -> Option<Located<'_, T>> {
    // `-0.0` names the sample at `0.0`: the recorder folds the two when
    // storing, and the renderer reads `-0` as `+0`. Without this,
    // `total_cmp` misses the exact hit, the `<=`/`>=` ends do not clamp
    // an *interior* `0.0`, and the strict windows bracket nothing -- so
    // querying at `-0.0` errored on a sample that exists.
    let time = time + 0.0;

    // A NaN names no sample and brackets nothing. The linear scan
    // refused it for free -- every comparison against a NaN is false --
    // but a search does not: `total_cmp` sorts a NaN above every real,
    // so the partition would put it past the last sample and **hold
    // the end**, answering where the scan errored. The suite caught
    // that; this is what it caught.
    if time.is_nan() {
        return None;
    }

    let first = samples.first()?;
    let last = samples.last()?;

    // The samples are in time order, so the scan is a search. It used
    // to be three linear passes -- an exact-hit `find`, then the two
    // clamps, then a `windows(2)` -- which is the T x N that made
    // `world_transform_samples` quadratic in a scene sampled per
    // frame. The answers are the same three.
    let at =
        samples.partition_point(|(t, _)| t.total_cmp(&time) == Ordering::Less);

    if let Some((t, value)) = samples.get(at)
        && t.total_cmp(&time) == Ordering::Equal
    {
        return Some(Located::At(value));
    }

    // Before the first sample or after the last: the end is held,
    // never extrapolated.
    if at == 0 {
        return Some(Located::At(&first.1));
    }
    if at == samples.len() {
        return Some(Located::At(&last.1));
    }

    let (from_time, from) = &samples[at - 1];
    let (to_time, to) = &samples[at];
    let alpha = (time - from_time) / (to_time - from_time);
    Some(Located::Between(from, to, alpha))
}

/// One chain node's transform with the typing rule already applied.
pub(super) enum Local {
    /// Constant at every time; `None` when the node sets none.
    Constant(Option<[f64; 16]>),
    /// Sampled, in time order.
    Sampled(Vec<(f64, [f64; 16])>),
}

/// One node's transform at `time`.
///
/// Outside the sampled range the end sample is held, because that is
/// what 3Delight does. Rendered: samples at `t=0` and `t=1` with the
/// shutter open over `[-1, 2]` leaves **zero** alpha beyond the two
/// sampled positions -- an extrapolating renderer would sweep half
/// again as far each way -- with a peak at each end, 2.7x the swept
/// middle, where a third of the shutter is held. `locate_sample`
/// states that rule once.
pub(super) fn local_at(
    handle: &str,
    local: &Local,
    time: f64,
) -> Result<Option<[f64; 16]>, ResolveError> {
    let samples = match local {
        Local::Constant(matrix) => return Ok(*matrix),
        Local::Sampled(samples) => samples,
    };

    match locate_sample(samples, time) {
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
            available: samples.iter().map(|(t, _)| *t).collect(),
        }),
    }
}

/// Compose an already-resolved chain at `time`.
///
/// The one interpolating fold: `world_transform_interpolated_at`
/// resolves the chain and calls it once, `world_transform_samples`
/// resolves the chain and calls it per time. A copied composition has
/// drifted four times in this crate, which is why the second caller
/// takes this rather than a fold of its own.
pub(super) fn interpolate_resolved(
    resolved: &[(&str, Local)],
    time: f64,
) -> Result<[f64; 16], ResolveError> {
    resolved
        .iter()
        .try_fold(IDENTITY, |matrix, (handle, local)| {
            Ok(match local_at(handle, local, time)? {
                Some(local) => mul(matrix, local),
                None => matrix,
            })
        })
}

/// A `transformationmatrix` argument as a row-major 4x4.
///
/// Non-`f64` matrices yield `None`: ɴsɪ documents the attribute as
/// `doublematrix`, and silently reinterpreting an `f32` one would be
/// worse than skipping it.
pub(super) fn matrices_of(arg: &OwnedArgument) -> Option<&[f64]> {
    // The declared type, array suffix included -- one statement of it,
    // in `OwnedArgument::as_matrices`. Rendered twice over: sixteen
    // `double`s are not a `doublematrix`, and a `doublematrix[2]` is
    // not one either (`E6007`, nothing drawn, where the plain
    // `doublematrix` draws two copies). Both leniencies drew what the
    // renderer refuses, and the second outlived the first because the
    // rule was stated in three places.
    arg.as_matrices()
}

/// A `transformationmatrix` argument as a row-major 4x4.
///
/// Non-`f64` matrices yield `None`: ɴsɪ documents the attribute as
/// `doublematrix`, and silently reinterpreting an `f32` one would be
/// worse than skipping it.
pub(super) fn matrix_of(arg: &OwnedArgument) -> Option<[f64; 16]> {
    arg.as_matrix()
}

impl Scene {
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
        Ok(self.motion_times_along(&chain))
    }

    /// The same, along a path already walked.
    ///
    /// [`Scene::motion_times`] refuses a geometry with more than one
    /// parent, because it walks the chain itself and there is no single
    /// chain. A backend emitting per-sample transforms for an instanced
    /// object needs the times of *its* path, which is what a
    /// [`Placement`] carries -- pass `placement.path`.
    pub fn motion_times_along(&self, path: &[String]) -> Vec<f64> {
        // The typing rule again: an unreadable sample is not a motion
        // time. Reporting one made `world_transform_samples` iterate a
        // time that `world_transform_at` then refused -- the same scene
        // answered differently depending on which accessor asked.
        let mut times = path
            .iter()
            .filter_map(|node| self.node(node))
            .flat_map(|node| {
                sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
                    matrix_of(arg).is_some()
                })
                .samples()
            })
            .map(|(time, _)| time)
            .collect::<Vec<_>>();

        // `total_cmp` throughout, matching how the samples were keyed.
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| a.total_cmp(b) == Ordering::Equal);

        times
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

        // The times a value stands at, which is not one per call: a
        // re-set at a time already recorded is another call and the
        // same sample.
        Ok(node
            .sample_calls(name)
            .map(|calls| {
                crate::scene::latest_per_time(calls)
                    .into_iter()
                    .map(|(time, _)| time)
                    .collect()
            })
            .unwrap_or_default())
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
    ) -> Result<Vec<(f64, &OwnedArgument)>, ResolveError> {
        let Some(node) = self.existing_node(handle)? else {
            return Ok(Vec::new());
        };

        Ok(node
            .sample_calls(name)
            .map(crate::scene::latest_per_time)
            .unwrap_or_default())
    }

    /// The same, with ɴsɪ's **typing rule** applied.
    ///
    /// [`Scene::attribute_samples`] reports what was recorded, because
    /// `name` is any attribute and this crate does not carry ɴsɪ's
    /// type for each one. A backend does know: it knows `P` is a
    /// `point` and `N` a `normal`. Hand that knowledge in as
    /// `readable` and this applies the rule the resolver applies to
    /// `transformationmatrix`, rather than making every backend
    /// re-derive it.
    ///
    /// The rule, rendered on a mesh's deforming `P` and identical to
    /// the transform case: an unreadable call unsets the attribute
    /// **at the call**, so it discards what was defined before it and
    /// only later calls rebuild it. Good at `t=0` with a `float` at
    /// `t=1` draws **nothing** (`Sampled::Unset`); the same two with a
    /// good `P` re-set at `t=1` afterwards draws static at that `P`.
    /// A backend that dropped the unreadable sample and kept the rest
    /// would sweep through both, which is the answer 3Delight gives to
    /// neither.
    ///
    /// # What `readable` should say
    ///
    /// 3Delight does not type-check every attribute, so the strictest
    /// predicate is not always the right one. Rendered: `P` declared
    /// `vector`, `normal`, `color`, `float 12` or `double 12` is
    /// `E6007` and the mesh does not draw -- there an exact
    /// `type_tag == Type::Point` matches the renderer. But `N`
    /// declared `vector` raises **no** error and renders identically
    /// to a `normal`, so a predicate demanding `Type::Normal` reports
    /// `Unset` for a call 3Delight accepts. The rule this applies is
    /// proven for the attributes the renderer type-checks; for the
    /// others, `readable` is the backend's own policy and this crate
    /// does not know better.
    ///
    /// # Errors
    ///
    /// [`ResolveError::UnknownHandle`] if no such node exists. A node
    /// that exists but never had the attribute set is
    /// [`Sampled::No`], not an error.
    pub fn sampled_attribute(
        &self,
        handle: &str,
        name: &str,
        readable: impl Fn(&OwnedArgument) -> bool,
    ) -> Result<Sampled<'_>, ResolveError> {
        let Some(node) = self.existing_node(handle)? else {
            return Ok(Sampled::No);
        };
        Ok(sampled_attr(node, name, readable))
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
    pub(super) fn interpolate_along(
        &self,
        path: &[String],
        time: f64,
    ) -> Result<[f64; 16], ResolveError> {
        let resolved: Vec<(&str, Local)> = path
            .iter()
            .map(|node| (node.as_str(), self.local_resolved(node)))
            .collect();
        interpolate_resolved(&resolved, time)
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
    /// A chain whose nodes are sampled at *different* times resolves:
    /// each node is interpolated from its own samples and the results
    /// composed, and the answer agrees with
    /// [`Scene::world_transform_interpolated_at`] at every time. This
    /// paragraph said the opposite -- that such a chain was
    /// `MissingSampleAtTime` -- for several commits after it stopped
    /// being true.
    ///
    /// # Errors
    ///
    /// Whatever walking the chain refuses, and exactly what
    /// [`Scene::world_transform_interpolated_at`] refuses for the same
    /// scene: the two walk one chain and are held to the same error by
    /// `the_two_interpolating_accessors_refuse_alike`.
    /// `MissingSampleAtTime` cannot arise here -- the times come from
    /// [`Scene::motion_times`], so every one of them has a sample.
    pub fn world_transform_samples(
        &self,
        handle: &str,
    ) -> Result<Vec<(f64, [f64; 16])>, ResolveError> {
        // Resolve the chain **once**, then compose at each time. Asking
        // `world_transform_interpolated_at` per time re-applied the
        // typing rule and re-decoded every matrix on every node at
        // every time, which is the T x N this row was `Open` for.
        // `transform_chain`, as `world_transform_interpolated_at` and
        // `motion_times` use: `chain` walks *through* an `instances`
        // node, so the two accessors refused a prototype under a
        // detached or shared instancer with different errors -- one
        // saying `Detached`, its twin `Instanced`. A reviewer found it
        // by mutating this line and watching nothing go red.
        let chain = self.transform_chain(handle)?;
        let resolved: Vec<(&str, Local)> = chain
            .iter()
            .map(|node| (node.as_str(), self.local_resolved(node)))
            .collect();

        self.motion_times_along(&chain)
            .into_iter()
            .map(|time| Ok((time, interpolate_resolved(&resolved, time)?)))
            .collect()
    }

    /// Whether any motion sample on `handle` sets a transform.
    ///
    /// Read before composing, because [`Scene::world_transform`] reads
    /// static attributes only and a motion-sampled node has none.
    pub(super) fn has_motion_transform(&self, handle: &str) -> bool {
        // The typing rule applies here too: a node whose samples are
        // all unreadable, or whose last one is, has no motion -- and
        // reporting motion for it made `world_transform` refuse a node
        // the other accessors resolve to identity.
        self.node(handle).is_some_and(|node| {
            // No allocation: this runs per node per fold.
            sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
                matrix_of(arg).is_some()
            })
            .samples()
            .next()
            .is_some()
        })
    }

    /// This node's own matrix, if it carries one.
    ///
    /// The node *type* is never consulted, matching classification: ɴsɪ
    /// permits attributes the node type would not imply, and a
    /// `transformationmatrix` on a non-transform node is composed like
    /// any other. See `contracts/resolution.md`.
    pub(super) fn local_transform(&self, handle: &str) -> Option<[f64; 16]> {
        let node = self.node(handle)?;
        matrix_of(node.attribute(TRANSFORMATION_MATRIX)?)
    }

    /// This node's matrix at `time`, interpolated.
    ///
    /// Linear between the bracketing samples, and held at the nearest
    /// sample outside the sampled range -- which is what 3Delight does;
    /// see [`Scene::world_transform_interpolated_at`].
    /// One chain node's transform, resolved once: the typing rule
    /// applied and the matrices decoded.
    ///
    /// Split from answering *at a time* because a caller asking at
    /// many times paid for this at every one of them --
    /// `world_transform_samples` asks at T times over N nodes, and
    /// re-resolving made that T x N over the samples.
    pub(super) fn local_resolved(&self, handle: &str) -> Local {
        let Some(node) = self.node(handle) else {
            return Local::Constant(None);
        };

        // A wrong-typed *last* sample unsets the attribute, so the
        // static value applies -- rendered, 3Delight draws the node at
        // identity rather than at the discarded earlier sample.
        let found = sampled_attr(node, TRANSFORMATION_MATRIX, |arg| {
            matrix_of(arg).is_some()
        });
        if matches!(found, Sampled::No | Sampled::Unset) {
            return Local::Constant(self.local_transform(handle));
        }

        Local::Sampled(
            found
                .samples()
                .filter_map(|(t, arg)| matrix_of(arg).map(|m| (t, m)))
                .collect(),
        )
    }

    pub(super) fn local_transform_at(
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
            .clone()
            .samples()
            .find(|(t, _)| t.total_cmp(&time) == Ordering::Equal)
        {
            Some((_, arg)) => Ok(matrix_of(arg)),
            None => Err(ResolveError::MissingSampleAtTime {
                handle: handle.to_string(),
                time,
                available: found.samples().map(|(t, _)| t).collect(),
            }),
        }
    }
}
