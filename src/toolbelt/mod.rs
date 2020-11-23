#![cfg_attr(feature = "nightly", doc(cfg(feature = "toolbelt")))]
//! # Convenience Methods for [`Context`](crate::Context)
//!
//! Names of methods that create nodes are nouns. Methods than modify
//! the node graph afterwards use verbs.
//!
//! Where ergonomically advised creation method names carry postfixes
//! that specify the type of node being created, such as `shader`.
use crate as nsi;
use crate::ArgSlice;
use snafu::Snafu;
use ultraviolet as uv;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("DELIGHT environment variable not set"))]
    DelightEnvironmentNotSet,
}

#[test]
fn test() {
    let ctx = nsi::Context::new(&[]).unwrap();

    //ctx.append(None);
}

fn default_node_root<'a>(node: Option<&'a str>) -> &'a str {
    match node {
        Some(node) => node,
        None => ".root",
    }
}

fn default_slot_objects<'a>(slot: Option<&'a str>) -> &'a str {
    match slot {
        Some(slot) => slot,
        None => "objects",
    }
}

fn generate_or_use_handle(handle: Option<&str>) -> String {
    match handle {
        Some(handle) => handle.to_string(),
        None => {
            use rand::{
                distributions::Alphanumeric, rngs::SmallRng, Rng, SeedableRng,
            };
            use std::iter;
            let mut rng = SmallRng::from_entropy();

            iter::repeat(())
                .map(|()| rng.sample(Alphanumeric))
                .take(20)
                .collect()
        }
    }
}

impl<'a> nsi::Context<'a> {
    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Append node `handle` to node `to`.
    ///
    /// # Arguments
    /// * `handle` – Node handle. If [`None`] a random handle is
    ///     generated.
    ///
    /// * `to` – Node to connect to downstream. If [`None`],
    ///     [`SceneRoot`](crate::context::NodeType::Root) is used.
    ///
    /// * `slot` – Slot on target node to connect to.
    ///     If [`None`], `"objects"` is used.
    ///
    /// Returns `handle` for convenience.
    /// # Example
    /// ```
    /// # let ctx = nsi::Context::new(&[]).unwrap();
    /// // Create a scaling transform node and append to the scene root.
    /// let scale = ctx.append(
    ///     Some(&ctx.scaling(None, &[10., 10., 10.])),
    ///     None,
    ///     None,
    /// );
    /// // Append the node "tetrahedron", which we created earlier,
    /// // to the scale node.
    /// ctx.append(Some("tetrahedron"), Some(&scale), None);
    /// ```
    #[inline]
    pub fn append(
        &self,
        handle: &str,
        to: Option<&str>,
        slot: Option<&str>,
    ) -> String {
        self.connect(
            handle,
            "",
            default_node_root(to),
            default_slot_objects(slot),
            &[],
        );
        handle.to_string()
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Insert node `handle` in-between `to` and `from`.
    ///
    /// # Arguments
    /// * `handle` – Node handle. If [`None`] a random handle is
    ///     generated.
    ///
    /// * `to` – Node to connect to downstream. If [`None`],
    ///     [`SceneRoot`](crate::context::NodeType::Root) is used.
    ///
    /// * `to_slot` – Slot on `to` node to connect to.
    ///     If [`None`], `"objects"` is used.
    ///
    /// * `from` – Node to connect tp upstream.
    ///
    /// * `from_slot` – Slot on `from` node to connect to.
    ///     If [`None`], `"objects"` is used.
    ///
    /// Returns `handle` for convenience.
    /// # Example
    /// ```
    /// # let ctx = nsi::Context::new(&[]).unwrap();
    /// // Insert the node "tetrahedron" between the ".root" and
    /// // "terahedron_attrib" nodes.
    /// ctx.insert(
    ///     Some("tetrahedron"),
    ///     None,
    ///     None,
    ///     "terahedron_attrib",
    ///     Some("geometryattributes"),
    /// );
    /// ```
    #[inline]
    pub fn insert(
        &self,
        handle: &str,
        to: Option<&str>,
        to_slot: Option<&str>,
        from: &str,
        from_slot: Option<&str>,
    ) -> String {
        self.append(handle, to, to_slot);
        self.append(handle, Some(from), from_slot)
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// The same as [`create()`](crate::context::Context::create()) but
    /// with support for autmatic handle generation.
    ///
    /// If `handle` is [`None`] a random handle is generated.
    ///
    /// Returns `handle` for convenience.
    #[inline]
    pub fn node(
        &self,
        handle: Option<&str>,
        node_type: impl Into<Vec<u8>>,
        args: &ArgSlice<'_, 'a>,
    ) -> String {
        let handle = generate_or_use_handle(handle);
        self.create(handle.as_str(), node_type, args);

        handle
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Create a scaling transform node.
    ///
    /// If `handle` is [`None`] a random handle is generated.
    ///
    /// Returns `handle` for convenience.
    #[inline]
    pub fn scaling(&self, handle: Option<&str>, scale: &[f64; 3]) -> String {
        let handle = generate_or_use_handle(handle);
        self.create(handle.as_str(), nsi::NodeType::Transform, &[]);

        self.set_attribute(
            handle.as_str(),
            &[double_matrix!(
                "transformationmatrix",
                uv::DMat4::from_nonuniform_scale(uv::DVec3::from(scale))
                    .as_array()
            )],
        );

        handle
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Create a traslation transform node.
    ///
    /// If `handle` is [`None`] a random handle is generated.
    ///
    /// Returns `handle` for convenience.
    #[inline]
    pub fn translation(
        &self,
        handle: Option<&str>,
        translate: &[f64; 3],
    ) -> String {
        let handle = generate_or_use_handle(handle);
        self.create(handle.as_str(), nsi::NodeType::Transform, &[]);

        self.set_attribute(
            handle.as_str(),
            &[double_matrix!(
                "transformationmatrix",
                uv::DMat4::from_translation(uv::DVec3::from(translate))
                    .as_array()
            )],
        );

        handle
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Create a traslation transform node.
    ///
    /// If `handle` is [`None`] a random handle is generated.
    ///
    /// The `angle` is specified in radians.
    ///
    /// Returns `handle` for convenience.
    pub fn rotation(
        &self,
        handle: Option<&str>,
        angle: f64,
        axis: &[f64; 3],
    ) -> String {
        let handle = generate_or_use_handle(handle);
        self.create(handle.as_str(), nsi::NodeType::Transform, &[]);

        self.set_attribute(
            handle.as_str(),
            &[double_matrix!(
                "transformationmatrix",
                uv::DMat4::from_angle_plane(
                    angle as _,
                    uv::DBivec3::from_normalized_axis(
                        uv::DVec3::from(axis).normalized()
                    )
                )
                .as_array()
            )],
        );

        handle
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Creates an environment light.
    ///
    /// If `handle` is [`None`] a random handle is generated.
    ///
    /// The `angle` is specified in radians.
    ///
    /// The `exposure` scales the intensity in
    /// [stops or EV values](https://en.wikipedia.org/wiki/Exposure_value).
    ///
    /// Returns `handle` for convenience or
    /// [`DelightEnvironmentNotSet`](Error::DelightEnvironmentNotSet)
    /// if the `DELIGHT` environemt variable is not set to find
    /// shaders.
    pub fn environment(
        &self,
        handle: Option<&str>,
        texture: &str,
        angle: Option<f64>,
        exposure: Option<f32>,
        visible: bool,
    ) -> String {
        // Create a rotation transform – this is the handle we return.
        let rotation =
            self.rotation(None, angle.unwrap_or(0.0), &[0.0, 1.0, 0.0]);

        let environment = generate_or_use_handle(handle);

        // Set up an environment light.
        self.append(
            &self.node(
                Some(environment.as_str()),
                nsi::NodeType::Environment,
                &[],
            ),
            Some(&rotation),
            None,
        );

        let attributes = self.append(
            &self.node(None, nsi::NodeType::Attributes, &[]),
            Some(&environment),
            Some("geometryattributes"),
        );
        self.set_attribute(
            attributes.as_str(),
            &[nsi::integer!("visibility.camera", visible as _)],
        );

        let shader = self.append(
            &self.node(None, nsi::NodeType::Shader, &[]),
            Some(&attributes),
            Some("surfaceshader"),
        );
        // Environment light attributes.
        self.set_attribute(
            shader,
            &[
                nsi::string!(
                    "shaderfilename",
                    "${DELIGHT}/osl/environmentLight"
                ),
                nsi::float!("intensity", 2.0f32.powf(exposure.unwrap_or(0.0))),
                nsi::string!("image", texture),
            ],
        );

        rotation
    }
}
