//! What applies to a geometry: the `attributes` nodes gathered along
//! its path, and ɴsɪ's precedence between them.

use super::*;

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
/// The **count** is as strict as the type: exactly one, which is what
/// [`OwnedArg::as_i32`] means. Rendered: the same scene with the
/// priority written `"int" 2 [ 10 10 ]` -- or `"int[2]" 1 [ 10 10 ]`,
/// which is the same argument spelled the other way -- leaves the
/// geometry hidden, so 3Delight ranked nothing on it, while the
/// one-value control shows it. Taking the first of several would rank
/// a node the renderer does not, which is the same mistake as reading
/// the `int64`.
pub(super) fn priority_value(arg: &OwnedArg) -> Option<i32> {
    arg.as_i32()
}

impl Scene {
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
                    .map(|(_, _, edge)| edge.from().to_string())
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
    /// # A priority with no attribute beside it
    ///
    /// A node that sets `ATTR.priority` but **not** `ATTR` is a
    /// definition too -- of `ATTR` at its ɴsɪ default -- and it ranks on
    /// that priority like any other. Rendered: an `attributes` node
    /// carrying only `visibility.priority` makes the geometry visible
    /// over a farther `visibility 0`, at priority `10` and at `0`
    /// alike; and one two levels up at priority `10` beats a
    /// `visibility 0` on the node attached to the primitive itself, so
    /// it is not merely winning on proximity. A node with no attributes
    /// at all is not a definition, and neither is one whose priority is
    /// an `int64`, which 3Delight cannot read.
    ///
    /// # Reading the value is not [`OwnedArg::as_i32`]
    ///
    /// 3Delight is far looser about an attribute's **value** than
    /// about its priority. Rendered, on `visibility`: `float 0.4` is
    /// visible and `float 0` is hidden, so a float is read and
    /// non-zero is true; `int64 0` hides; `int 2 [ 0 0 ]` hides, where
    /// the same shape as a *priority* is ignored outright; and a
    /// `string` reads as **true**.
    ///
    /// A wrong-typed value is still a *definition*: rendered, a
    /// `string` on the nearest node beats a `visibility 0` two levels
    /// up and the object is visible, so it wins its ranking rather
    /// than falling through to the next candidate. The trap therefore
    /// has two jaws. A backend that reaches for [`OwnedArg::as_i32`]
    /// here gets `None` for the numeric three and draws an object the
    /// renderer hides; one that reads that `None` as "not defined" and
    /// looks further along the chain draws hidden where the renderer
    /// draws visible. This crate hands back the argument rather than a
    /// decoded flag precisely because the rule is the renderer's, and
    /// it is not the rule beside it.
    ///
    /// Such a winner comes back with [`AttributeValue::arg`] `None`:
    /// this crate names the attribute and leaves ɴsɪ's default for it to
    /// the backend, which is the one thing it cannot supply.
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
            && let Some(arg) = node.effective(name)
        {
            return Some(AttributeValue {
                node: handle,
                name: &arg.name,
                arg: Some(arg),
                priority: 0,
            });
        }

        for (_, _, edge) in
            self.gathered_along(path, &EdgeKind::ShaderAttributes)
        {
            let Some(node) = self.node(edge.from()) else {
                continue;
            };
            if let Some(arg) = node.effective(name) {
                return Some(AttributeValue {
                    node: edge.from(),
                    name: &arg.name,
                    arg: Some(arg),
                    priority: 0,
                });
            }
        }
        None
    }

    /// ɴsɪ's attribute precedence over an already-gathered list.
    pub(super) fn resolve_attribute<'a>(
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
            let Some(node) = self.node(edge.from()) else {
                continue;
            };

            let keys = core::iter::once((1u8, name))
                .chain(fallback.map(|name| (0u8, name)));

            for (specificity, key) in keys {
                let beside = node.effective(&format!("{key}.priority"));
                let priority = beside.and_then(priority_value);

                // A lone `ATTR.priority` is a definition of `ATTR` at
                // its ɴsɪ default, so it is a candidate with no value.
                // Its name is the priority argument's own, less the
                // suffix: a borrow of the scene, where `key` is only
                // borrowed from the caller. An unreadable priority is
                // not a definition -- 3Delight ignores that node.
                let (attribute, arg) = match (node.effective(key), beside) {
                    (Some(arg), _) => (arg.name.as_str(), Some(arg)),
                    (None, Some(beside)) if priority.is_some() => (
                        &beside.name[..beside.name.len() - ".priority".len()],
                        None,
                    ),
                    (None, _) => continue,
                };
                let priority = priority.unwrap_or(0);

                candidates.push((
                    priority,
                    specificity,
                    rank,
                    AttributeValue {
                        node: edge.from(),
                        name: attribute,
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
            sources.push(handle.to_string());
        }
        sources.extend(
            self.gathered_containers(geometry, &EdgeKind::ShaderAttributes)?
                .into_iter()
                .map(|(_, _, edge)| edge.from().to_string()),
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
    pub(super) fn gathered_attributes(
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
    pub(super) fn gathered_containers(
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
    pub(super) fn gathered_along(
        &self,
        chain: &[String],
        kind: &EdgeKind,
    ) -> Vec<(usize, usize, &Edge)> {
        // The geometry, then the sets it belongs to directly, then the
        // transforms above it. With no set memberships this is `chain`
        // and the walk is unchanged.
        let mut sources: Vec<&str> = Vec::with_capacity(chain.len() + 1);
        for node in chain {
            sources.push(node.as_str());
            sources.extend(
                self.edges_from(node.as_str())
                    .filter(|edge| edge.kind == EdgeKind::SetMember)
                    .map(|edge| edge.to()),
            );
        }

        // One set can hold several nodes on the chain. It is a single
        // source at its nearest occurrence -- rendered, a set holding
        // both the mesh and its transform ranks where the mesh does.
        let mut seen = HashSet::new();
        sources.retain(|handle| seen.insert(*handle));

        let mut gathered: Vec<(usize, usize, &Edge)> = sources
            .into_iter()
            .enumerate()
            .flat_map(|(depth, node)| {
                self.edges_to_attr(node, kind.to_attr())
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
    pub(super) fn shader_on(
        &self,
        gathered: &[(usize, usize, &Edge)],
        kind: &EdgeKind,
    ) -> Option<String> {
        gathered
            .iter()
            .enumerate()
            .flat_map(|(rank, (_, _, edge))| {
                self.edges_to_attr(edge.from(), kind.to_attr())
                    .filter(move |shader| shader.kind == *kind)
                    .map(move |shader| (shader.priority(), rank, shader))
            })
            // Highest priority, then earliest in the gathered order.
            .min_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)))
            .map(|(_, _, shader)| shader.from().to_string())
    }
}
