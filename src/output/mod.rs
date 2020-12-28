#![cfg_attr(feature = "nightly", doc(cfg(feature = "output")))]
//! # Output Driver Callbacks
//! This module declares several closure types. These can be passed via
//! [`Callback`](crate::argument::Callback)s to an
//! [`OutputDriver`](crate::context::NodeType::OutputDriver) node to stream
//! pixels during and/or after a render, in-memory.
//!
//! There are three types of closure:
//! * [`FnOpen`] is called once when the
//!   [`OutputDriver`](crate::context::NodeType::OutputDriver) is *opened* by
//!   the renderer.
//!
//! * [`FnWrite`] is called for each bucket of pixel data the renderer sends to
//!   the [`OutputDriver`](crate::context::NodeType::OutputDriver).
//!
//! * [`FnFinish`] is called once when the
//!   [`OutputDriver`](crate::context::NodeType::OutputDriver) is *closed* by
//!   the renderer.
//!
//! As a user you can choose how to use this API.
//!
//! * To get a single buffer of pixel data when rendering is finished it is
//!   enough to only
//    implement an [`FnFinish`] closure.
//!
//! * To get the pixel buffer updated while the renderer is working implemet an
//!   [`FnWrite`] closure.
//!
//! The format of the [`Vec<f32>`] buffer is described by the [`PixelFormat`]
//! parameter which is passed to both of these closures.
//!
//! ## Example
//! ```
//! # fn write_exr(_: &str, _: usize, _: usize, _: usize, _: &[f32]) {}
//! # let ctx = nsi::Context::new(&[]).unwrap();
//! // Setup a screen.
//! ctx.create("screen", nsi::NodeType::Screen, &[]);
//! // We pretend we defined a camera node earlier.
//! ctx.connect("screen", "", "camera", "screens", &[]);
//! ctx.set_attribute(
//!     "screen",
//!     &[
//!         // The resolution becomes the width & height passed to
//!         // FnOpen, FnWrite and FnFinish type callbacks.
//!         nsi::integers!("resolution", &[1920, 1080]).array_len(2),
//!     ],
//! );
//!
//! // Setup an RGBA output layer.
//! ctx.create("beauty", nsi::NodeType::OutputLayer, &[]);
//! ctx.set_attribute(
//!     "beauty",
//!     &[
//!         // The Ci variable comes from Open Shading Language.
//!         nsi::string!("variablename", "Ci"),
//!         // We want the data as raw, 32 bit float.
//!         nsi::string!("scalarformat", "float"),
//!         nsi::integer!("withalpha", 1),
//!     ],
//! );
//! ctx.connect("beauty", "", "screen", "outputlayers", &[]);
//!
//! // Setup an output driver.
//! ctx.create("driver", nsi::NodeType::OutputDriver, &[]);
//! ctx.connect("driver", "", "beauty", "outputdrivers", &[]);
//!
//! // Our FnFinish callback. We will be called once.
//! let finish = nsi::output::FinishCallback::new(
//!     |// Passed from the output driver node, below.
//!      image_filename: &str,
//!      // Passed from the screen node, above.
//!      width: usize,
//!      // Passed from the screen node, above.
//!      height: usize,
//!      pixel_format: nsi::output::PixelFormat,
//!      pixel_data: Vec<f32>| {
//!         // Call some function to write our image to an OpenEXR file.
//!         write_exr(
//!             (String::from(image_filename) + ".exr").as_str(),
//!             width,
//!             height,
//!             pixel_format.len(),
//!             &pixel_data,
//!         );
//!         nsi::output::Error::None
//!     },
//! );
//!
//! ctx.set_attribute(
//!     "driver",
//!     &[
//!         // Important: FERRIS is the built-in output driver
//!         // we must use with Rust.
//!         nsi::string!("drivername", nsi::output::FERRIS),
//!         // This will end up in the `name` parameter passed
//!         // to finish().
//!         nsi::string!("imagefilename", "render"),
//!         nsi::callback!("callback.finish", finish),
//!     ],
//! );
//!
//! ctx.render_control(&[nsi::string!("action", "start")]);
//!
//! // The finish() closure will be called once, before the next call returns.
//! ctx.render_control(&[nsi::string!("action", "wait")]);
//! ```
//!
//! ## Color Profiles
//! The pixel color data that the renderer generates is linear and
//! scene-referred. I.e. relative to whatever units you used to describe
//! illuminants in your scene.
//!
//! Using the
//! [`"colorprofile"` attribute](https://nsi.readthedocs.io/en/latest/nodes.html?highlight=outputlayer#the-outputlayer-node)
//! of an [`OutputLayer`](crate::context::NodeType::OutputLayer) you can ask the
//! renderer to apply an [Open Color IO](https://opencolorio.org/) (OCIO)
//! [profile/LUT](https://github.com/colour-science/OpenColorIO-Configs/tree/feature/aces-1.2-config/aces_1.2/luts)
//! before quantizing (see below).
//!
//! Once OCIO has a [Rust wrapper](https://crates.io/crates/opencolorio) you can easily choose to
//! do these color conversions yourself. In the meantime there is the
//! [`colorspace`](https://crates.io/crates/colorspace) crate which has some usefule profiles
//! built in, e.g. [ACEScg](https://en.wikipedia.org/wiki/Academy_Color_Encoding_System#ACEScg).
//!
//! ```
//! # let ctx = nsi::Context::new(&[]).unwrap();
//! ctx.create("beauty", nsi::NodeType::OutputLayer, &[]);
//! ctx.set_attribute(
//!     "beauty",
//!     &[
//!         // The Ci variable comes from Open Shading Language.
//!         nsi::string!("variablename", "Ci"),
//!         // We want the pixel data 'display-referred' in sRGB and quantized down to 0.0..255.0.
//!         nsi::string!("colorprofile", "/home/luts/linear_to_sRGB.spi1d"),
//!         nsi::string!("scalarformat", "uint8"),
//!     ],
//! );
//! ```
//!
//! ## Quantization
//!
//! Using the
//! [`"scalarformat"` attribute](https://nsi.readthedocs.io/en/latest/nodes.html?highlight=outputlayer#the-outputlayer-node)
//! of an [`OutputLayer`](crate::context::NodeType::OutputLayer) you can ask the
//! renderer to quantize data down to a suitable range. For example, setting
//! this to `"uint16"` will get you valid `u16` values from `0.0..65535.0`, but
//! stored in the `f32`s of the `pixel_data` buffer. The value of `1.0` will map
//! to `65535.0` and everything above will be clipped. You can just convert
//! these value straight via `f32 as u16`.
//!
//! Unless you asked the renderer to also apply some color profile (see above)
//! the data is linear and needs to be made display-referred before is will look
//! good on screen.
//!
//! See the `output` example on how to do this with a simple,
//! display-referred sRGB curve.
use crate::argument::CallbackPtr;
use core::ops::Index;
use std::{
    ffi::CStr,
    os::raw::{c_char, c_int, c_void},
};

// Juypiter notebook support ------------------------------------------
#[cfg(feature = "jupyter")]
pub mod jupyter;

pub static FERRIS: &str = "ferris";

/// An error type the callbacks return to communicate with the
/// renderer.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, num_enum::IntoPrimitive)]
pub enum Error {
    /// Everyhing is dandy.
    None = ndspy_sys::PtDspyError_PkDspyErrorNone as _,
    /// We ran out of memory.
    NoMemory = ndspy_sys::PtDspyError_PkDspyErrorNoMemory as _,
    /// We do no support this request.
    Unsupported = ndspy_sys::PtDspyError_PkDspyErrorUnsupported as _,
    BadParameters = ndspy_sys::PtDspyError_PkDspyErrorBadParams as _,
    NoResource = ndspy_sys::PtDspyError_PkDspyErrorNoResource as _,
    /// Something else went wrong.
    Undefined = ndspy_sys::PtDspyError_PkDspyErrorUndefined as _,
    /// Stop the render.
    Stop = ndspy_sys::PtDspyError_PkDspyErrorStop as _,
}

/// A closure which is called once per
/// [`OutputDriver`](crate::context::NodeType::OutputDriver) instance.
///
/// It is passed to NSI via the `"callback.open"` attribute on that node.
///
/// The closure is called once, before the renderer starts sending pixels to the
/// output driver. # Arguments
/// The `pixel_format` parameter is an array of strings that details the
/// composition of the `f32` data that the renderer will send to the [`FnWrite`]
/// and/or [`FnFinish`] closures.
///
/// # Example
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # use nsi::output::PixelFormat;
/// # let ctx = nsi::Context::new(&[]).unwrap();
/// # ctx.create("display_driver", nsi::NodeType::OutputDriver, &[]);
/// let open = nsi::output::OpenCallback::new(
///     |name: &str, width: usize, height: usize, pixel_format: &nsi::output::PixelFormat| {
///         println!(
///             "Resolution: {}×{}\nPixel Format:\n{:?}",
///             width, height, pixel_format
///         );
///         nsi::output::Error::None
///     },
/// );
/// # }
/// ```
pub trait FnOpen<'a>: FnMut(
    // Filename.
    &str,
    // Width.
    usize,
    // Height.
    usize,
    // Pixel format.
    &PixelFormat,
) -> Error
+ 'a {}
impl<'a, T: FnMut(&str, usize, usize, &PixelFormat) -> Error + 'a> FnOpen<'a> for T {}

// FIXME once trait aliases are in stable.
/*
trait FnOpen<'a> = FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // Pixel format.
        &PixelFormat,
    ) -> Error
    + 'a
*/

/// A closure which is called for each bucket of pixels the
/// [`OutputDriver`](crate::context::NodeType::OutputDriver) instance sends
/// during rendering.
///
/// It is passed to NSI via the `"callback.write"` attribute on that node.
/// # Example
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # let ctx = nsi::Context::new(&[]).unwrap();
/// # ctx.create("display_driver", nsi::NodeType::OutputDriver, &[]);
/// let write = nsi::output::WriteCallback::new(
///     |name: &str,
///      width: usize,
///      height: usize,
///      x_min: usize,
///      x_max_plus_one: usize,
///      y_min: usize,
///      y_max_plus_one: usize,
///      pixel_format: &nsi::output::PixelFormat,
///      pixel_data: &[f32]| {
///         /// Send our pixels to some texture for realtime display.
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "oxidized_output_driver",
///     &[
///         nsi::string!("drivername", "ferris"),
///         // While rendering, send all pixel buckets to the write closure.
///         nsi::callback!("callback.write", write),
///     ],
/// );
/// # }
/// ```
pub trait FnWrite<'a>: FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // x_min.
        usize,
        // x_max_plus_one
        usize,
        // y_min.
        usize,
        // y_max_plus_one,
        usize,
        // Pixel format.
        &PixelFormat,
        // Pixel data.
        &[f32],
    ) -> Error
    + 'a {}
impl<
        'a,
        T: FnMut(&str, usize, usize, usize, usize, usize, usize, &PixelFormat, &[f32]) -> Error + 'a,
    > FnWrite<'a> for T
{
}

// FIXME once trait aliases are in stable.
/*
pub trait FnWrite<'a> = FnMut(
    // Filename.
    &str,
    // Width.
    usize,
    // Height.
    usize,
    // x_min.
    usize,
    // x_max_plus_one
    usize,
    // y_min.
    usize,
    // y_max_plus_one,
    usize,
    // Pixel format.
    &PixelFormat,
    // Pixel data.
    &mut [f32],
) -> Error
+ 'a
*/

/// A closure which is called once per
/// [`OutputDriver`](crate::context::NodeType::OutputDriver) instance.
///
/// It is passed to NSI via the `"callback.finish"` attribute on that node.
///
/// The closure is called once, before after renderer has finished sending
/// pixels to the output driver.
/// # Example
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # let ctx = nsi::Context::new(&[]).unwrap();
/// # ctx.create("display_driver", nsi::NodeType::OutputDriver, &[]);
/// let finish = nsi::output::FinishCallback::new(
///     |name: &str,
///      width: usize,
///      height: usize,
///      pixel_format: nsi::output::PixelFormat,
///      pixel_data: Vec<f32>| {
///         println!(
///             "The top, left pixel the first channel in the {:?} layer has the value {}.",
///             pixel_format[0].name(),
///             pixel_data[0],
///         );
///         nsi::output::Error::None
///     },
/// );
///
/// ctx.set_attribute(
///     "oxidized_output_driver",
///     &[
///         nsi::string!("drivername", "ferris"),
///         // When done, send all pixels to the finish closure.
///         nsi::callback!("callback.finish", finish),
///     ],
/// );
/// # }
/// ```
pub trait FnFinish<'a>: FnMut(
    // Filename.
    &str,
    // Width.
    usize,
    // Height.
    usize,
    // Pixel format.
    PixelFormat,
    // Pixel data.
    Vec<f32>,
) -> Error
+ 'a {}
impl<'a, T: FnMut(&str, usize, usize, PixelFormat, Vec<f32>) -> Error + 'a> FnFinish<'a> for T {}

// FIXME once trait aliases are in stable.
/*
pub trait FnFinish<'a> = dyn FnMut(
        // Filename.
        &str,
        // Width.
        usize,
        // Height.
        usize,
        // Pixel format.
        PixelFormat,
        // Pixel data.
        Vec<f32>,
    ) -> Error
    + 'a;
*/

enum Query {}

trait FnQuery<'a>: FnMut(Query) -> Error + 'a {}
impl<'a, T: FnMut(Query) -> Error + 'a> FnQuery<'a> for T {}

// FIXME once trait aliases are in stable.
/*
pub trait FnQuery<'a> = dyn FnMut(Query) -> Error + 'a;
*/

// Why do we need a triple Box here? No idea and neither had anyone
// from the Rust community. But omitting a single Box wrapper layer
// leads to an instant crash.
pub struct OpenCallback<'a>(Box<Box<Box<dyn FnOpen<'a>>>>);

impl<'a> OpenCallback<'a> {
    pub fn new<F>(fn_open: F) -> Self
    where
        F: FnOpen<'a>,
    {
        OpenCallback(Box::new(Box::new(Box::new(fn_open))))
    }
}

impl CallbackPtr for OpenCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

pub struct WriteCallback<'a>(Box<Box<Box<dyn FnWrite<'a>>>>);

impl<'a> WriteCallback<'a> {
    pub fn new<F>(fn_write: F) -> Self
    where
        F: FnWrite<'a>,
    {
        WriteCallback(Box::new(Box::new(Box::new(fn_write))))
    }
}

impl CallbackPtr for WriteCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

pub struct FinishCallback<'a>(Box<Box<Box<dyn FnFinish<'a>>>>);

impl<'a> FinishCallback<'a> {
    pub fn new<F>(fn_finish: F) -> Self
    where
        F: FnFinish<'a>,
    {
        FinishCallback(Box::new(Box::new(Box::new(fn_finish))))
    }
}

impl CallbackPtr for FinishCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

struct DisplayData<'a> {
    name: &'a str,
    width: usize,
    height: usize,
    pixel_format: PixelFormat,
    pixel_data: Vec<f32>,
    fn_open: Option<Box<Box<Box<dyn FnOpen<'a>>>>>,
    fn_write: Option<Box<Box<Box<dyn FnWrite<'a>>>>>,
    fn_finish: Option<Box<Box<Box<dyn FnFinish<'a>>>>>,
    // Unused atm.
    fn_query: Option<Box<Box<Box<dyn FnQuery<'a>>>>>,
}

impl<'a> DisplayData<'a> {
    /// Used to dissect DisplayData into its components before being dropped.
    #[allow(clippy::type_complexity)]
    fn boxed_into_tuple(
        display_data: Self,
    ) -> (
        &'a str,
        usize,
        usize,
        PixelFormat,
        Vec<f32>,
        Option<Box<Box<Box<dyn FnFinish<'a>>>>>,
    ) {
        // FIXME: These boxes somehow get deallocated twice if we don't suppress this
        // here. No f*cking idea why.
        if let Some(fn_open) = display_data.fn_open {
            Box::into_raw(fn_open);
        }
        if let Some(fn_write) = display_data.fn_write {
            Box::into_raw(fn_write);
        }
        if let Some(fn_query) = display_data.fn_query {
            Box::into_raw(fn_query);
        }

        (
            display_data.name,
            display_data.width,
            display_data.height,
            display_data.pixel_format,
            display_data.pixel_data,
            display_data.fn_finish,
        )
    }
}

/// Description of an [`OutputLayer`](crate::context::NodeType::OutputLayer)
/// inside a flat, raw pixel.
#[derive(Debug, Clone, Default)]
pub struct Layer {
    name: String,
    depth: LayerDepth,
    offset: usize,
}

impl Layer {
    /// The name of the layer.
    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The [depth](LayerDepth) of this layer.
    #[inline]
    pub fn depth(&self) -> LayerDepth {
        self.depth
    }

    /// The channel offset of the layer inside the [`PixelFormat`].
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The number of channels in this layer. This is a shortcut for calling
    /// `depth().channels()`.
    #[inline]
    pub fn channels(&self) -> usize {
        self.depth.channels()
    }
}

/// The depth (number and type of channels) a pixel in a [`Layer`] is
/// composed of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerDepth {
    /// A single channel. Obtained when setting `"layertype"` `"scalar"` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    OneChannel,
    /// A single channel with alpha. Obtained when setting `"layertype"`
    /// `"scalar"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    OneChannelAndAlpha,
    /// An `rgb` color triplet. Obtained when setting `"layertype"` `"color"` on
    /// an [`OutputLayer`](crate::context::NodeType::OutputLayer).
    Color,
    /// An `rgb` color triplet with alpha. Obtained when setting `"layertype"`
    /// `"color"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    ColorAndAlpha,
    /// An `xyz` triplet. Obtained when setting `"layertype"` `"vector"` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    Vector,
    /// An `xyz` triplet with alpha. Obtained when setting `"layertype"`
    /// `"vector"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    VectorAndAlpha,
    /// An quadruple of values. Obtained when setting `"layertype"` `"quad"` on
    /// an [`OutputLayer`](crate::context::NodeType::OutputLayer).
    FourChannels,
    /// An quadruple of values with alpha. Obtained when setting `"layertype"`
    /// `"quad"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::context::NodeType::OutputLayer).
    FourChannelsAndAlpha,
}

impl Default for LayerDepth {
    fn default() -> Self {
        LayerDepth::OneChannel
    }
}

impl LayerDepth {
    /// Returns the number of channels this layer type consists of.
    pub fn channels(&self) -> usize {
        match self {
            LayerDepth::OneChannel => 1,
            LayerDepth::OneChannelAndAlpha => 2,
            LayerDepth::Color => 3,
            LayerDepth::Vector => 3,
            LayerDepth::ColorAndAlpha => 4,
            LayerDepth::VectorAndAlpha => 4,
            LayerDepth::FourChannels => 4,
            LayerDepth::FourChannelsAndAlpha => 5,
        }
    }
}

/// Accessor for the pixel format the renderer sends in [`FnOpen`], [`FnWrite`]
/// and [`FnFinish`].
///
/// This is a stack of [`OutputLayer`](crate::context::NodeType::OutputLayer)
/// descriptions.
///
/// # Example
///
/// A typical format for a pixel containing two such layers, an *RGBA* **color**
/// + **alpha** output layer and a world space **normal**, will look like this:
///
/// [`name`](Layer::name()) | [`layer_type`](Layer::layer_type())                  | [`offset`](Layer::offset())
/// ------------------------|------------------------------------------------------|----------------------------
/// `Ci`                    | [`ColorAndAlpha`](LayerType::ColorAndAlpha) (`rgba`) | `0`
/// `N_world`               | [`Vector`](LayerType::Vector) (`xyz`)                | `4`
///
/// ## RAW Layout
///
/// The resp. callbacks deliver pixels as a flat [`f32`] buffer.
/// For the above example the actual layout of a single pixel in the
/// buffer is:
///
/// Value  | `r`ed   | `g`reen | `b`lue  | `a`lpha | `x` | `y` | `z`
/// -------|---------|---------|---------|---------|-----|-----|----
/// Offset | `0`     | `1`     | `2`     | `3`     | `4` | `5` | `6`
///
/// The `offset` is the offset into the pixel buffer to obtain the 1st element.
/// For example, the **y** coordinate of the the normal will be stored in
/// channel at offset `5` (`4` + `1`).
///
/// The pixel format is in the order in which
/// [`OutputLayer`](crate::context::NodeType::OutputLayer)s were defined in the
/// [ɴsɪ scene](https://nsi.readthedocs.io/en/latest/guidelines.html#basic-scene-anatomy).
#[derive(Debug, Default)]
pub struct PixelFormat(Vec<Layer>);

impl PixelFormat {
    #[inline]
    fn new(format: &[ndspy_sys::PtDspyDevFormat]) -> Self {
        let (mut previous_layer_name, mut previous_channel_id) =
            Self::split_into_layer_name_and_channel_id(
                unsafe { CStr::from_ptr(format[0].name) }.to_str().unwrap(),
            );

        let mut depth = LayerDepth::OneChannel;
        let mut offset = 0;

        PixelFormat(
            // This loops through each format (channel), r, g, b, a etc.
            format
                .iter()
                .enumerate()
                .cycle()
                .take(format.len() + 1)
                .filter_map(|format| {
                    // FIXME: add support for specifying AOV and detect type
                    // for indexing (.r vs .x)
                    let name = unsafe { CStr::from_ptr(format.1.name) }.to_str().unwrap();

                    let (layer_name, channel_id) = Self::split_into_layer_name_and_channel_id(name);

                    // A boundary between two layers will be when the postfix
                    // is a combination of those above.
                    if ["b", "z", "s", "a"].contains(&previous_channel_id)
                        && ["r", "x", "s"].contains(&channel_id)
                    {
                        let tmp_layer_name = if previous_layer_name.is_empty() {
                            "Ci"
                        } else {
                            previous_layer_name
                        };
                        previous_layer_name = layer_name;

                        previous_channel_id = channel_id;

                        let tmp_depth = depth;
                        depth = LayerDepth::OneChannel;

                        let tmp_offset = offset;
                        offset = format.0;

                        Some(Layer {
                            name: tmp_layer_name.to_string(),
                            depth: tmp_depth,
                            offset: tmp_offset,
                        })
                    } else {
                        // Do we we have a lonely alpha -> it belongs to the current
                        // layer.
                        if layer_name.is_empty() && "a" == channel_id {
                            depth = match &depth {
                                LayerDepth::OneChannel => LayerDepth::OneChannelAndAlpha,
                                LayerDepth::Color => LayerDepth::ColorAndAlpha,
                                LayerDepth::Vector => LayerDepth::VectorAndAlpha,
                                LayerDepth::FourChannels => LayerDepth::FourChannelsAndAlpha,
                                _ => unreachable!(),
                            };
                        }
                        // Are we still on the same layer?
                        else if layer_name == previous_layer_name {
                            // We only check for first channel.
                            match channel_id {
                                "r" | "g" | "b" => depth = LayerDepth::Color,
                                "x" | "y" | "z" => depth = LayerDepth::Vector,
                                "a" => {
                                    if layer_name.is_empty() {
                                        depth = match &depth {
                                            LayerDepth::OneChannel => {
                                                LayerDepth::OneChannelAndAlpha
                                            }
                                            LayerDepth::Color => LayerDepth::ColorAndAlpha,
                                            LayerDepth::Vector => LayerDepth::VectorAndAlpha,
                                            _ => unreachable!(),
                                        };
                                    } else {
                                        depth = LayerDepth::FourChannels;
                                    }
                                }
                                _ => (),
                            }
                            previous_layer_name = layer_name;
                        // We have a new layer.
                        } else {
                            previous_layer_name = layer_name;
                        }
                        previous_channel_id = channel_id;
                        None
                    }
                })
                .collect::<Vec<_>>(),
        )
    }

    fn split_into_layer_name_and_channel_id(name: &str) -> (&str, &str) {
        let mut split = name.rsplitn(3, '.');
        // We know we never get an empty string so we can safely unwrap
        // here.
        let mut postfix = split.next().unwrap();
        if "000" == postfix {
            postfix = "s";
            // Reset iterator.
            split = name.rsplitn(2, '.');
        }
        // Skip the middle part.
        if split.next().is_some() {
            // We know that if there is middle part we always have a prefix
            // so we can safely unwrap here.
            (split.next().unwrap(), postfix)
        } else {
            ("", postfix)
        }
    }

    /// Returns the number of layers in a pixel.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Checks is pixel contains any data at all.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the total number of channels in a pixel.
    /// This is the sum of the number of channels in all layers.
    #[inline]
    pub fn channels(&self) -> usize {
        self.0
            .iter()
            .fold(0, |total, layer| total + layer.channels())
    }
}

impl Index<usize> for PixelFormat {
    type Output = Layer;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

fn get_parameter_triple_box<T: ?Sized>(
    name: &str,
    type_: u8,
    len: usize,
    parameters: &mut [ndspy_sys::UserParameter],
) -> Option<Box<Box<Box<T>>>> {
    for p in parameters.iter() {
        let p_name = unsafe { CStr::from_ptr(p.name) }.to_str().unwrap();

        if name == p_name && type_ == p.valueType as _ && len == p.valueCount as _ {
            if !p.value.is_null() {
                return Some(unsafe { Box::from_raw(p.value as *mut Box<Box<T>>) });
            } else {
                // Parameter exists but value is missing –
                // exit quietly.
                break;
            }
        }
    }
    None
}

/// Trampoline function for the FnOpen callback.
#[no_mangle]
pub(crate) extern "C" fn image_open(
    image_handle_ptr: *mut ndspy_sys::PtDspyImageHandle,
    _driver_name: *const c_char,
    output_filename: *const c_char,
    width: c_int,
    height: c_int,
    parameters_count: c_int,
    parameters: *const ndspy_sys::UserParameter,
    format_count: c_int,
    format: *mut ndspy_sys::PtDspyDevFormat,
    flag_stuff: *mut ndspy_sys::PtFlagStuff,
) -> ndspy_sys::PtDspyError {
    // FIXME: check that driver_name is "ferris".
    if (image_handle_ptr.is_null()) || (output_filename.is_null()) {
        return Error::BadParameters.into();
    }

    let parameters = unsafe {
        // We need to const->mut transmute() here because we need
        // pointers to FnMut below.
        std::slice::from_raw_parts_mut(
            parameters as *mut ndspy_sys::UserParameter,
            parameters_count as _,
        )
    };

    let mut display_data = Box::new(DisplayData {
        name: unsafe { CStr::from_ptr(output_filename).to_str().unwrap() },
        width: width as _,
        height: height as _,
        pixel_format: PixelFormat::default(),
        pixel_data: vec![0.0f32; (width * height * format_count) as _],
        fn_open: get_parameter_triple_box::<dyn FnOpen>("callback.open", b'p', 1, parameters),
        fn_write: get_parameter_triple_box::<dyn FnWrite>("callback.write", b'p', 1, parameters),
        fn_query: None, /* get_parameter_triple_box::<FnQuery>("callback.query", b'p', 1,
                         * parameters), */
        fn_finish: get_parameter_triple_box::<dyn FnFinish>("callback.finish", b'p', 1, parameters),
    });

    let format = unsafe { std::slice::from_raw_parts_mut(format, format_count as _) };

    // We want f32/channel data.
    format
        .iter_mut()
        .for_each(|format| format.type_ = ndspy_sys::PkDspyFloat32);

    display_data.pixel_format = PixelFormat::new(format);

    let error = if let Some(ref mut fn_open) = display_data.fn_open {
        fn_open(
            display_data.name,
            width as _,
            height as _,
            &display_data.pixel_format,
        )
    } else {
        Error::None
    };

    unsafe {
        *image_handle_ptr = Box::into_raw(display_data) as _;
    }

    // We want to be called for all buckets.
    unsafe {
        (*flag_stuff).flags = ndspy_sys::PkDspyFlagsWantsEmptyBuckets as _;
    }

    error.into()
}

/// FIXME: this will be used for a FnProgress callback later.
#[no_mangle]
pub(crate) extern "C" fn image_query(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    query_type: ndspy_sys::PtDspyQueryType,
    data_len: c_int,
    data: *mut c_void,
) -> ndspy_sys::PtDspyError {
    match query_type {
        ndspy_sys::PtDspyQueryType_PkRenderProgress => {
            if (data_len as usize) < core::mem::size_of::<ndspy_sys::PtDspyRenderProgressFuncPtr>()
            {
                Error::BadParameters
            } else {
                *unsafe {
                    &mut std::mem::transmute::<_, ndspy_sys::PtDspyRenderProgressFuncPtr>(data)
                } = Some(image_progress);
                Error::None
            }
        }
        _ => Error::Unsupported,
    }
    .into()
}

/// Trampoline function for the FnWrite callback.
#[no_mangle]
pub(crate) extern "C" fn image_write(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    _entry_size: c_int,
    pixel_data: *const u8,
) -> ndspy_sys::PtDspyError {
    let display_data = unsafe { &mut *(image_handle_ptr as *mut DisplayData) };

    // _entry_size is pixel_length in u8s, we need pixel length in f32s.
    let pixel_length = display_data.pixel_format.channels();

    let pixel_data = unsafe {
        std::slice::from_raw_parts(
            pixel_data as *const f32,
            pixel_length * ((x_max_plus_one - x_min) * (y_max_plus_one - y_min)) as usize,
        )
    };

    let bucket_width = pixel_length * (x_max_plus_one - x_min) as usize;

    let mut source_index = 0;
    for y in y_min as usize..y_max_plus_one as _ {
        let dest_index = (y * display_data.width + x_min as usize) * pixel_length;

        // We memcpy() each scanline.
        (&mut display_data.pixel_data[dest_index..dest_index + bucket_width])
            .copy_from_slice(&(pixel_data[source_index..source_index + bucket_width]));

        source_index += bucket_width;
    }

    // Call the closure.
    if let Some(ref mut fn_write) = display_data.fn_write {
        fn_write(
            display_data.name,
            display_data.width,
            display_data.height,
            x_min as _,
            x_max_plus_one as _,
            y_min as _,
            y_max_plus_one as _,
            &display_data.pixel_format,
            display_data.pixel_data.as_mut_slice(),
        )
    } else {
        Error::None
    }
    .into()
}

/// Trampoline function for the FnFinish callback.
#[no_mangle]
pub(crate) extern "C" fn image_close(
    image_handle_ptr: ndspy_sys::PtDspyImageHandle,
) -> ndspy_sys::PtDspyError {
    let display_data = unsafe { Box::from_raw(image_handle_ptr as *mut DisplayData) };

    let (name, width, height, pixel_format, pixel_data, fn_finish) =
        DisplayData::boxed_into_tuple(*display_data);

    if let Some(mut fn_finish) = fn_finish {
        let error = fn_finish(name, width, height, pixel_format, pixel_data);
        Box::into_raw(fn_finish);
        error
    } else {
        Error::None
    }
    .into()
}

#[no_mangle]
extern "C" fn image_progress(
    _image_handle_ptr: ndspy_sys::PtDspyImageHandle,
    progress: f32,
) -> ndspy_sys::PtDspyError {
    println!("\rProgress: {}", progress * 100.0);
    Error::None.into()
}
