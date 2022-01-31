#![cfg_attr(feature = "nightly", doc(cfg(feature = "toolbelt")))]
//! Convenience methods for an ɴsɪ [`Context`](nsi::context::Context).
//!
//! Names of methods that create nodes are *nouns*. All other methods use
//! *verbs*.

// Where ergonomically suggestible, creation methods names carry postfixes that
// specify the type of node being created, such as `shader`.

use nsi_core as nsi;
use ultraviolet as uv;

#[inline]
fn default_slot_objects(slot: Option<&str>) -> &str {
    slot.unwrap_or("objects")
}

/// Generates a random handle.
pub fn generate_handle() -> String {
    use rand::{distributions::Alphanumeric, rngs::SmallRng, Rng, SeedableRng};
    use std::iter;
    let mut rng = SmallRng::from_entropy();

    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric) as char)
        .take(20)
        .collect()
}

/// Generates a random handle if `handle` is `None` or falls through,
/// returning `"prefix_handle"`, otherwise.
///
/// The `prefix` is ignoed if `handle` is *not* `None`.
#[doc(hidden)]
#[cfg(debug_assertions)]
pub fn generate_or_use_handle(handle: Option<&str>, prefix: Option<&str>) -> String {
    match handle {
        Some(handle) => {
            if let Some(prefix) = prefix {
                String::from(prefix) + "_" + handle
            } else {
                handle.to_string()
            }
        }
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
pub fn generate_or_use_handle(handle: Option<&str>, _prefix: Option<&str>) -> String {
    match handle {
        Some(handle) => {
            if let Some(prefix) = prefix {
                String::from(prefix) + "_" + handle.to_string()
            } else {
                handle.to_string()
            }
        }
        None => generate_handle(),
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
/// # let ctx = nsi::Context::new(None).unwrap();
/// // Create a scaling transform node and append to the scene root.
/// let scale = ctx.append(
///     ".root",
///     // Append the node "tetrahedron", which we created earlier,
///     // to the scale node.
///     &ctx.append(
///         &ctx.scaling(None, &[10., 10., 10.]),
///         "tetrahedron",
///         // Use "objects" slot.
///         None,
///     )
///     .0,
///     // Use "objects" slot.
///     None,
/// );
/// ```
#[inline]
pub fn append<'a, 'b, 'c>(
    ctx: &'a nsi::Context,
    to: &'b str,
    handle: &'c str,
    slot: Option<&str>,
) -> (&'b str, &'c str)
where
    'a: 'b,
    'a: 'c,
{
    ctx.connect(handle, "", to, default_slot_objects(slot), None);

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
/// * `from` – Node to connect tp upstream.
///
/// Returns (`to`, `handle`).
/// # Example
/// ```
/// # let ctx = nsi::Context::new(None).unwrap();
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
    append(ctx, handle, from, handle_slot);
    append(ctx, to, handle, to_slot)
}

/// The same as [`create()`](nsi::context::Context::create()) but with support
/// for automatic handle generation.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn node<'a>(
    ctx: &'a nsi::Context<'a>,
    handle: Option<&str>,
    node_type: impl Into<Vec<u8>>,
    args: &nsi::ArgSlice<'_, 'a>,
) -> String {
    let node_type_copy: Vec<u8> = node_type.into();
    let handle = generate_or_use_handle(
        handle,
        Some(unsafe { std::str::from_utf8_unchecked(&node_type_copy) }),
    );

    ctx.create(handle.as_str(), node_type_copy, None);

    if !args.is_empty() {
        ctx.set_attribute(handle.as_str(), args);
    }

    handle
}

/// Create a traslation transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn translation(ctx: &nsi::Context, handle: Option<&str>, translate: &[f64; 3]) -> String {
    let handle = generate_or_use_handle(handle, Some("translation"));
    ctx.create(handle.as_str(), nsi::NodeType::Transform, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_translation(uv::DVec3::from(translate)).as_array()
        )],
    );

    handle
}

/// Create a rotation transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
///
/// # Arguments
/// * `angle` – Counter-clockwise rotation in degrees.
/// * `up` – A direction defining the up axis (default `[0.0, 1.0, 0.0]`). Does
///   *not* need to be normalized.
pub fn rotation(
    ctx: &nsi::Context,
    handle: Option<&str>,
    angle: f64,
    axis: Option<&[f64; 3]>,
) -> String {
    let handle = generate_or_use_handle(handle, Some("rotation"));
    ctx.create(handle.as_str(), nsi::NodeType::Transform, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_angle_plane(
                (angle * core::f64::consts::TAU / 90.0) as _,
                uv::DBivec3::from_normalized_axis(
                    uv::DVec3::from(axis.unwrap_or(&[0.0, 1.0, 0.0])).normalized()
                )
            )
            .transposed()
            .as_array()
        )],
    );

    handle
}

/// Create a scaling transform node.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
#[inline]
pub fn scaling(ctx: &nsi::Context, handle: Option<&str>, scale: &[f64; 3]) -> String {
    let handle = generate_or_use_handle(handle, Some("scaling"));
    ctx.create(handle.as_str(), nsi::NodeType::Transform, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::from_nonuniform_scale(uv::DVec3::from(scale)).as_array()
        )],
    );

    handle
}

/// Creates a transformation matrix based on a direction that can be used to
/// position a camera or light.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
///
/// # Arguments
/// * `from` – The position of the eye/sensor/film plate/light source..
/// * `to` – The position the object is 'looking at'.
/// * `up` – A direction defining the up axis (default `[0.0, 1.0, 0.0]`). Does
///   *not* need to be normalized.
pub fn look_at(
    ctx: &nsi::Context,
    handle: Option<&str>,
    from: &[f64; 3],
    to: &[f64; 3],
    up: Option<&[f64; 3]>,
) {
    let handle = generate_or_use_handle(handle, Some("look_at"));
    ctx.create(handle.as_str(), nsi::NodeType::Transform, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::look_at(
                uv::DVec3::from(from),
                uv::DVec3::from(to),
                uv::DVec3::from(up.unwrap_or(&[0.0, 1.0, 0.0])),
            )
            .inversed()
            .as_array()
        )],
    );
}

/// Creates a transformation node based on a bounding box that can be used to
/// position a camera.
///
/// A connected perspective camera with the same *field of view* with a screen
/// node with the same *aspect ratio* will contain the specified bounding box.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// Returns `handle` for convenience.
///
/// # Arguments
/// * `direction` – The axis the camera should be looking along. Does *not* need
///   to be normalized.
/// * `vertical_fov` – The vertical *field of view* in degrees.
/// * `bounds` – Axis-aligned bounding box in the form `[x_min, y_min, z_min,
///   x_max, y_max, z_max]`.
/// * `up` – A direction defining the up axis (default `[0.0, 1.0, 0.0]`). Does
///   *not* need to be normalized.
/// * `aspect_ratio` – The ratio of  *with* ÷ *height* of the camera's
///   sensor/film plate.
pub fn look_at_bounds_perspective_camera(
    ctx: &nsi::Context,
    handle: Option<&str>,
    direction: &[f64; 3],
    vertical_fov: f32,
    bounds: &[f64; 6],
    up: Option<&[f64; 3]>,
    aspect_ratio: Option<f32>,
) -> String {
    let aspect_ratio = aspect_ratio.unwrap_or(1.0);
    let vertical_fov = if aspect_ratio < 1.0 {
        // Portrait.
        2.0 * (aspect_ratio * (0.5 * vertical_fov * core::f32::consts::PI / 180.0).tan()).atan()
    } else {
        vertical_fov * core::f32::consts::PI / 180.0
    } as f64;

    println!("{}", vertical_fov);

    // Make a cube from the bounds.
    let cube = [
        uv::DVec3::new(bounds[0], bounds[1], bounds[2]),
        uv::DVec3::new(bounds[0], bounds[4], bounds[2]),
        uv::DVec3::new(bounds[0], bounds[1], bounds[5]),
        uv::DVec3::new(bounds[3], bounds[4], bounds[5]),
        uv::DVec3::new(bounds[3], bounds[1], bounds[5]),
        uv::DVec3::new(bounds[3], bounds[4], bounds[2]),
    ];

    let bounds_center = 0.5 * (cube[0] + cube[3]);

    println!("{:?}", bounds_center);

    let bounding_sphere_radius = cube
        .iter()
        .fold(0.0f64, |max, point| {
            max.max((bounds_center - *point).mag_sq())
        })
        .sqrt();

    let distance = bounding_sphere_radius / (vertical_fov * 0.5).sin();

    println!("{}", distance);

    let handle = generate_or_use_handle(handle, Some("look_at"));

    ctx.create(handle.as_str(), nsi::NodeType::Transform, None);

    ctx.set_attribute(
        handle.as_str(),
        &[nsi::double_matrix!(
            "transformationmatrix",
            uv::DMat4::look_at(
                bounds_center - distance * uv::DVec3::from(direction).normalized(),
                bounds_center,
                uv::DVec3::from(up.unwrap_or(&[0.0, 1.0, 0.0]))
            )
            .inversed()
            .as_array()
        )],
    );

    handle
}
