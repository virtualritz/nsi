#![cfg_attr(feature = "nightly", doc(cfg(feature = "toolbelt")))]
//! # Convenience Methods for an ɴsɪ [`Context`](nsi::Context)
//!
//! Names of methods that create nodes are nouns. Methods than modify
//! the node graph afterwards use verbs.
//!
//! Where ergonomically advised, creation methods names carry postfixes
//! that specify the type of node being created, such as `shader`.
use nsi_core as nsi;
use ultraviolet as uv;
//use uv::{DVec3, DMat4};

/// Generates a random handle if `handle` is `None` or falls through,
/// otherwise.
#[doc(hidden)]
#[cfg(debug_assertions)]
pub fn generate_or_use_handle(
    handle: Option<&str>,
    prefix: Option<&str>,
) -> String {
    match handle {
        Some(handle) => handle.to_string(),
        None => {
            if let Some(prefix) = prefix {
                String::from(prefix) + "_" + &petname::petname(3, "_")
            } else {
                petname::petname(3, "_")
            }
        }
    }
}

#[doc(hidden)]
#[cfg(not(debug_assertions))]
pub fn generate_or_use_handle(
    handle: Option<&str>,
    _prefix: Option<&str>,
) -> String {
    match handle {
        Some(handle) => handle.to_string(),
        None => {
            use rand::{
                distributions::Alphanumeric, rngs::SmallRng, Rng, SeedableRng,
            };
            use std::iter;
            let mut rng = SmallRng::from_entropy();

            iter::repeat(())
                .map(|()| rng.sample(Alphanumeric) as char)
                .take(20)
                .collect()
        }
    }
}

/// Append node `handle` to node `to`.
///
/// # Arguments
/// * `to` – Node to connect to downstream.
///
/// * `slot` – Slot on target node to connect to. If [`None`], `"objects"` is
///   used.
///
/// * `handle` – Handle of node to append.
///
/// Returns (`to`, `handle`).
/// # Example
/// ```
/// # use nsi_core as nsi;
/// # use nsi_toolbelt::{append, scaling};
/// # let ctx = nsi::Context::new(None).unwrap();
/// // Create a scaling transform node and append to the scene root.
/// let scale = append(
///     &ctx,
///     ".root",
///     // Use "objects" slot.
///     None,
///     // Append the node "tetrahedron", which we created earlier,
///     // to the scale node.
///     append(
///         &ctx,
///         &scaling(&ctx, None, &[10., 10., 10.]),
///         // Use "objects" slot.
///         None,
///         "tetrahedron",
///     )
///     .0,
/// );
/// ```
#[inline]
pub fn append<'a, 'b, 'c>(
    ctx: &'a nsi::Context,
    to: &'b str,
    slot: Option<&str>,
    handle: &'c str,
) -> (&'b str, &'c str)
where
    'a: 'b,
    'a: 'c,
{
    ctx.connect(handle, None, to, slot.unwrap_or("objects"), None);

    (to, handle)
}

/// Insert node `handle` in-between `to` and `from`.
///
/// # Arguments
/// * `to` – Node to connect to downstream.
///
/// * `to_slot` – Slot on `to` node to connect to. If [`None`], `"objects"` is
///   used.    .
///
/// * `handle` – Handle of node to insert.
///
/// * `handle_slot` – Slot on `handle` node to connect to. If [`None`],
///   `"objects"` is used.
///
/// * `from` – Node to connect to upstream.
///
/// Returns (`to`, `handle`).
/// # Example
/// ```
/// # use nsi_core as nsi;
/// # use nsi_toolbelt::insert;
/// # let ctx = nsi::Context::new(None).unwrap();
/// // Insert the node "tetrahedron" between the ".root" and
/// // "terahedron_attrib" nodes.
/// insert(
///     &ctx,
///     ".root",
///     None,
///     "tetrahedron",
///     Some("geometryattributes"),
///     "terahedron_attrib",
/// );
/// ```
#[inline]
pub fn insert<'a, 'b, 'c>(
    ctx: &'a nsi::Context,
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
    append(ctx, handle, handle_slot, from);
    append(ctx, to, to_slot, handle)
}

/// The same as [`create()`](nsi::context::Context::create()) but
/// with support for automatic handle generation.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn node<'a>(
    ctx: &nsi::Context<'a>,
    handle: Option<&str>,
    node_type: &str,
    args: Option<&nsi::ArgSlice<'_, 'a>>,
) -> String {
    let handle = generate_or_use_handle(handle, Some(node_type));

    ctx.create(handle.as_str(), node_type, None);

    if let Some(args) = args {
        ctx.set_attribute(handle.as_str(), args);
    }

    handle
}

/// Create a scaling transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn scaling(
    ctx: &nsi::Context,
    handle: Option<&str>,
    scale: &[f64; 3],
) -> String {
    let handle = generate_or_use_handle(handle, Some("scaling"));
    ctx.create(handle.as_str(), nsi::node::TRANSFORM, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_nonuniform_scale(uv::DVec3::from(scale)).as_array()
        )],
    );

    handle
}

/// Create a translation transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn translation(
    ctx: &nsi::Context,
    handle: Option<&str>,
    translate: &[f64; 3],
) -> String {
    let handle = generate_or_use_handle(handle, Some("translation"));
    ctx.create(handle.as_str(), nsi::node::TRANSFORM, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_translation(uv::DVec3::from(translate)).as_array()
        )],
    );

    handle
}

/// Create a translation transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// The `angle` is specified in degrees.
///
/// Returns `handle` for convenience.
pub fn rotation(
    ctx: &nsi::Context,
    handle: Option<&str>,
    angle: f64,
    axis: &[f64; 3],
) -> String {
    let handle = generate_or_use_handle(handle, Some("rotation"));
    ctx.create(handle.as_str(), nsi::node::TRANSFORM, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_angle_plane(
                (angle * core::f64::consts::TAU / 90.0) as _,
                uv::DBivec3::from_normalized_axis(
                    uv::DVec3::from(axis).normalized()
                )
            )
            .transposed()
            .as_array()
        )],
    );

    handle
}

/// **Convenience method; not part of the official ɴsɪ API.**
pub fn look_at_camera(
    ctx: &nsi::Context,
    handle: Option<&str>,
    eye: &[f64; 3],
    to: &[f64; 3],
    up: &[f64; 3],
) {
    let handle = generate_or_use_handle(handle, Some("look_at"));
    ctx.create(handle.as_str(), nsi::node::TRANSFORM, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
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

/// Creates a transformation matrix that can be used to position
/// a camera. Its view will contains the perspective-projected
/// bounding box under the specified field-of-view and aspect ratio
/// (*with*÷*height*).
/// # Arguments
/// * `direction` – The axis the camera should be looking along. Does *not* need
///   to be normalized.
/// * `up` – A direction to look
/// * `bounding_box` – Axis-aligned bounding box in the form `[x_min, y_min,
///   z_min, x_max, y_max, z_max]`.
pub fn look_at_bounding_box_perspective_camera(
    ctx: &nsi::Context,
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
            2.0 * (aspect_ratio
                * (0.5 * vertical_fov * core::f32::consts::PI / 180.0).tan())
            .atan()
        } else {
            vertical_fov * core::f32::consts::PI / 180.0
        }
    } else {
        vertical_fov * core::f32::consts::PI / 180.0
    } as f64;

    //println!("{}", vertical_fov);

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

    //println!("{:?}", bounding_box_center);

    let bounding_sphere_radius = cube
        .iter()
        .fold(0.0f64, |max, point| {
            max.max((bounding_box_center - *point).mag_sq())
        })
        .sqrt();

    let distance = bounding_sphere_radius / (vertical_fov * 0.5).sin();

    //println!("{}", distance);

    let handle = generate_or_use_handle(handle, Some("look_at"));

    ctx.create(handle.as_str(), nsi::node::TRANSFORM, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
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
