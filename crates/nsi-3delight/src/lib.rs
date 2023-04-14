//! Helpers for using ɴsɪ with 3Delight.
//!
//! Shortcuts for instancing common nodes.
use nsi_core as nsi;
use nsi_toolbelt::{append, generate_or_use_handle, node, rotation};

/// Creates a typical environment node.
///
/// A latitutde-lungitude environment map will be aligned as-shot
/// with the horizon along the X-Z plane at infinity.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// # Arguments
/// * `angle` – In degrees; specifies how much to rotate the environment around
///   the Y (up) axis.
///
/// * `visible` – If the environment is visible to the camera.
///
/// Returns `handle` and the handle of the created `shader`.
///
/// Note that the `shader` node is empty. It is up to the user
/// to set the resp. attributes on the node or hook up an OSL
/// network below it.
pub fn environment(
    ctx: &nsi::Context,
    handle: Option<&str>,
    angle: Option<f64>,
    visible: Option<bool>,
) -> (String, String) {
    // Create a rotation transform – this is the handle we return.
    let rotation = rotation(ctx, None, angle.unwrap_or(0.0), &[0.0, 1.0, 0.0]);

    let environment = generate_or_use_handle(handle, Some("environment"));

    // Set up an environment light.
    append(
        ctx,
        &rotation,
        None,
        &node(
            ctx,
            Some(environment.as_str()),
            nsi::node::ENVIRONMENT,
            None,
        ),
    );

    let shader = node(ctx, None, nsi::node::SHADER, None);

    append(
        ctx,
        &environment,
        Some("geometryattributes"),
        append(
            ctx,
            &node(
                ctx,
                None,
                nsi::node::ATTRIBUTES,
                Some(&[nsi::integer!(
                    "visibility.camera",
                    visible.unwrap_or(true) as _
                )]),
            ),
            Some("surfaceshader"),
            shader.as_str(),
        )
        .0,
    );

    (rotation, shader)
}

/// Creates a textured environment light.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// # Arguments
/// * `texture – A latitude-longitude texture map in one of these formats:
///     * TIFF
///     * JPEG
///     * Radiance
///     * OpenEXR
///     * GIF
///     * IFF
///     * SGI
///     * PIC
///     * Photoshop PSD
///     * TGA
///
/// * `angle` – In degrees; specifies how much to rotate the environment around
///   the Y (up) axis.
///
/// * `exposure` – Scales the intensity in [stops or EV values](https://en.wikipedia.org/wiki/Exposure_value).
///
/// * `visible` – If the environment is visible to the camera.
///
/// Returns `handle` and the handle of the created `shader`.
///
/// Note that the `shader` node is empty. It is up to the user
/// to set the resp. attributes on the node or hook up an OSL
/// network below it.
pub fn environment_texture<'a, 'b>(
    ctx: &nsi::Context<'a>,
    handle: Option<&str>,
    texture: &str,
    angle: Option<f64>,
    exposure: Option<f32>,
    visible: Option<bool>,
    args: Option<&nsi::ArgSlice<'b, 'a>>,
) -> (String, String)
where
    'a: 'b,
{
    let (rotation, shader) = environment(ctx, handle, angle, visible);

    // Environment light attributes.
    ctx.set_attribute(
        shader.as_str(),
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/environmentLight"),
            nsi::float!("intensity", 2.0f32.powf(exposure.unwrap_or(0.0))),
            nsi::string!("image", texture),
        ],
    );

    if let Some(args) = args {
        ctx.set_attribute(shader.as_str(), args);
    }

    (rotation, shader)
}

/// **Convenience method; not part of the official ɴsɪ API.**
///
/// Creates a physically plausible, procedural sky environment light.
///
/// If `handle` is [`None`] a random handle is generated.
///
/// # Arguments
/// * `angle` – In degrees; specifies how much to rotate the environment around
///   the Y (up) axis.
///
/// * `exposure` – Scales the intensity in [stops or EV values](https://en.wikipedia.org/wiki/Exposure_value).
///
/// * `visible` – If the environment is visible to the camera.
///
/// Returns `handle` and the handle of the created `shader`.
///
/// Note that this instances a `dlSky` shader. Using the returned  `shader`
/// handle you can set more attributes on this node.
pub fn environment_sky<'a, 'b>(
    ctx: &nsi::Context<'a>,
    handle: Option<&str>,
    angle: Option<f64>,
    exposure: Option<f32>,
    visible: Option<bool>,
    args: Option<&nsi::ArgSlice<'b, 'a>>,
) -> (String, String)
where
    'a: 'b,
{
    let (rotation, shader) = environment(ctx, handle, angle, visible);

    // Environment light attributes.
    ctx.set_attribute(
        shader.as_str(),
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlSky"),
            nsi::float!("intensity", 2.0f32.powf(exposure.unwrap_or(0.0))),
        ],
    );

    if let Some(args) = args {
        ctx.set_attribute(shader.as_str(), args);
    }

    (rotation, shader)
}
