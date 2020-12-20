#![cfg_attr(feature = "nightly", doc(cfg(feature = "toolbelt")))]
//! # Convenience Methods for [`Context`](crate::Context)
//!
//! Names of methods that create nodes are nouns. Methods than modify
//! the node graph afterwards use verbs.
//!
//! Where ergonomically advised, creation methods names carry postfixes
//! that specify the type of node being created, such as `shader`.
use crate as nsi;
use crate::ArgSlice;
use ultraviolet as uv;

#[inline]
fn _default_node_root<'a>(node: Option<&'a str>) -> &'a str {
    match node {
        Some(node) => node,
        None => ".root",
    }
}

#[inline]
fn default_slot_objects<'a>(slot: Option<&'a str>) -> &'a str {
    match slot {
        Some(slot) => slot,
        None => "objects",
    }
}

/// Generates a random handle if `handle` is `None` or falls through,
/// otherwise.
pub fn generate_or_use_handle(handle: Option<&str>) -> String {
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
    /// * `to` – Node to connect to downstream. If [`None`],
    ///     [`Root`](crate::context::NodeType::Root) is used.
    ///
    /// * `slot` – Slot on target node to connect to.
    ///     If [`None`], `"objects"` is used.
    ///
    /// * `handle` – Handle of node to append.
    ///
    /// Returns (`to`, `handle`).
    /// # Example
    /// ```
    /// # let ctx = nsi::Context::new(&[]).unwrap();
    /// // Create a scaling transform node and append to the scene root.
    /// let scale = ctx.append(
    ///     ".root",
    ///     // Use "objects" slot.
    ///     None,
    ///     // Append the node "tetrahedron", which we created earlier,
    ///     // to the scale node.
    ///     &ctx.append(
    ///         &ctx.scaling(None, &[10., 10., 10.]),
    ///         // Use "objects" slot.
    ///         None,
    ///         "tetrahedron",
    ///     ).0,
    /// );
    /// ```
    #[inline]
    pub fn append<'b, 'c>(
        &self,
        to: &'b str,
        slot: Option<&str>,
        handle: &'c str,
    ) -> (&'b str, &'c str)
    where
        'a: 'b,
        'a: 'c,
    {
        self.connect(handle, "", to, default_slot_objects(slot), &[]);

        (to, handle)
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Insert node `handle` in-between `to` and `from`.
    ///
    /// # Arguments
    /// * `to` – Node to connect to downstream. If [`None`],
    ///     [`SceneRoot`](crate::context::NodeType::Root) is used.
    ///
    /// * `to_slot` – Slot on `to` node to connect to.
    ///     If [`None`], `"objects"` is used.    .
    ///
    /// * `handle` – Handle of node to insert.
    ///
    /// * `handle_slot` – Slot on `handle` node to connect to.
    ///     If [`None`], `"objects"` is used.
    ///
    /// * `from` – Node to connect tp upstream.
    ///
    /// Returns (`to`, `handle`).
    /// # Example
    /// ```
    /// # let ctx = nsi::Context::new(&[]).unwrap();
    /// // Insert the node "tetrahedron" between the ".root" and
    /// // "terahedron_attrib" nodes.
    /// ctx.insert(
    ///     ".root",
    ///     None,
    ///     "tetrahedron",
    ///     Some("geometryattributes"),
    ///     "terahedron_attrib",
    /// );
    /// ```
    #[inline]
    pub fn insert<'b, 'c>(
        &self,
        to: &'b str,
        to_slot: Option<&str>,
        handle: &'c str,
        handle_slot: Option<&str>,
        from: &str,
    ) -> (&'b str, &'c str)
    where
        'a: 'b,
        'a: 'c,
    {
        self.append(handle, handle_slot, from);
        self.append(to, to_slot, handle)
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
    pub fn look_at_camera(
        &self,
        handle: Option<&str>,
        eye: &[f64; 3],
        to: &[f64; 3],
        up: &[f64; 3],
    ) {
        let handle = generate_or_use_handle(handle);
        self.create(handle.as_str(), nsi::NodeType::Transform, &[]);

        self.set_attribute(
            handle.as_str(),
            &[double_matrix!(
                "transformationmatrix",
                uv::DMat4::look_at(
                    uv::DVec3::from(eye),
                    uv::DVec3::from(to),
                    uv::DVec3::from(up),
                )
                .inversed()
                .as_array()
            )],
        );
    }

    /// **Convenience method; not part of the official ɴsɪ API.**
    ///
    /// Creates a transformation matrix that can be used to position
    /// a camera. Its view will contains the perspective-projected
    /// bounding box under the specified field-of-view and aspect ratio
    /// (*with*÷*height*).
    /// # Arguments
    /// * `direction` – The axis the camera should be looking along.
    ///     Does *not* need to be normalized.
    /// * `up` – A direction to look
    /// * `bounding_box` – Axis-aligned bounding box in the form
    ///     `[x_min, y_min, z_min, x_max, y_max, z_max]`.
    pub fn look_at_bounding_box_perspective_camera(
        &self,
        handle: Option<&str>,
        direction: &[f64; 3],
        up: &[f64; 3],
        vertical_fov: f32,
        aspect_ratio: Option<f32>,
        bounding_box: &[f64; 6],
    ) -> String {
        // FIXME with a && chain once https://github.com/rust-lang/rust/issues/53667
        // arrives in stable.
        let vertical_fov = if let Some(aspect_ratio) = aspect_ratio {
            if aspect_ratio < 1.0 {
                // Portrait.
                (aspect_ratio
                    * (vertical_fov * core::f32::consts::PI / 90.0).tan())
                .atan()
            } else {
                vertical_fov * core::f32::consts::TAU / 90.0
            }
        } else {
            vertical_fov * core::f32::consts::TAU / 90.0
        } as f64;

        // Make a cube from the bounds.
        let cube = [
            uv::DVec3::new(bounding_box[0], bounding_box[1], bounding_box[2]),
            uv::DVec3::new(bounding_box[0], bounding_box[4], bounding_box[2]),
            uv::DVec3::new(bounding_box[0], bounding_box[1], bounding_box[5]),
            uv::DVec3::new(bounding_box[3], bounding_box[4], bounding_box[5]),
            uv::DVec3::new(bounding_box[3], bounding_box[1], bounding_box[5]),
            uv::DVec3::new(bounding_box[3], bounding_box[4], bounding_box[2]),
        ];

        let bounding_box_center = 0.5 * (cube[0] + cube[3]);

        let bounding_sphere_radius = cube
            .iter()
            .fold(0.0f64, |max, point| {
                max.max((bounding_box_center - *point).mag_sq())
            })
            .sqrt();

        let distance =
            (bounding_sphere_radius * 2.0) / (vertical_fov * 0.5).sin();

        let handle = generate_or_use_handle(handle);

        self.create(handle.as_str(), nsi::NodeType::Transform, &[]);

        self.set_attribute(
            handle.as_str(),
            &[double_matrix!(
                "transformationmatrix",
                uv::DMat4::look_at(
                    bounding_box_center
                        - distance * uv::DVec3::from(direction).normalized(),
                    bounding_box_center,
                    uv::DVec3::from(up)
                )
                .inversed()
                .as_array()
            )],
        );

        handle
    }
}
