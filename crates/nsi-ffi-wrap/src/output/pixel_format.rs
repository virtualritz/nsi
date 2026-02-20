use core::ops::Deref;
use std::ffi::CStr;

/// The scalar type of pixel channel data.
///
/// This enum represents the data type of individual channel values
/// as specified by the ndspy format constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum ScalarType {
    /// 32-bit IEEE 754 floating point (f32)
    #[default]
    Float32 = 1,
    /// 16-bit IEEE 754 floating point (f16/half)
    Float16 = 12,
    /// 32-bit unsigned integer (u32)
    Unsigned32 = 2,
    /// 32-bit signed integer (i32)
    Signed32 = 3,
    /// 16-bit unsigned integer (u16)
    Unsigned16 = 4,
    /// 16-bit signed integer (i16)
    Signed16 = 5,
    /// 8-bit unsigned integer (u8)
    Unsigned8 = 6,
    /// 8-bit signed integer (i8)
    Signed8 = 7,
}

impl ScalarType {
    /// Create a ScalarType from an ndspy type constant.
    /// Returns None for unknown type values.
    pub fn from_ndspy_type(type_: u32) -> Option<Self> {
        match type_ {
            1 => Some(ScalarType::Float32),
            2 => Some(ScalarType::Unsigned32),
            3 => Some(ScalarType::Signed32),
            4 => Some(ScalarType::Unsigned16),
            5 => Some(ScalarType::Signed16),
            6 => Some(ScalarType::Unsigned8),
            7 => Some(ScalarType::Signed8),
            12 => Some(ScalarType::Float16),
            _ => None,
        }
    }

    /// Returns the size in bytes of this scalar type.
    pub const fn size_bytes(self) -> usize {
        match self {
            ScalarType::Float32
            | ScalarType::Unsigned32
            | ScalarType::Signed32 => 4,
            ScalarType::Float16
            | ScalarType::Unsigned16
            | ScalarType::Signed16 => 2,
            ScalarType::Unsigned8 | ScalarType::Signed8 => 1,
        }
    }
}

/// Description of an [`OutputLayer`](crate::OUTPUT_LAYER) node
/// inside a flat, raw pixel.
#[derive(Debug, Clone, Default, PartialEq)]
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

    /// Returns true if the [depth](LayerDepth) of this layer contains an alpha
    /// channel. This is a shortcut for calling `depth().has_alpha()`.
    #[inline]
    pub fn has_alpha(&self) -> bool {
        self.depth.has_alpha()
    }
}

/// The depth (number and type of channels) a pixel in a [`Layer`] is
/// composed of.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerDepth {
    /// A single channel. Obtained when setting `"layertype"` `"scalar"` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    #[default]
    OneChannel,
    /// A single channel with alpha. Obtained when setting `"layertype"`
    /// `"scalar"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    OneChannelAndAlpha,
    /// An `rgb` color triplet. Obtained when setting `"layertype"` `"color"`
    /// on an [`OutputLayer`](crate::OUTPUT_LAYER).
    Color,
    /// An `rgb` color triplet with alpha. Obtained when setting `"layertype"`
    /// `"color"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    ColorAndAlpha,
    /// An `xyz` triplet. Obtained when setting `"layertype"` `"vector"` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    Vector,
    /// An `xyz` triplet with alpha. Obtained when setting `"layertype"`
    /// `"vector"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    VectorAndAlpha,
    /// An quadruple of values. Obtained when setting `"layertype"` `"quad"` on
    /// an [`OutputLayer`](crate::OUTPUT_LAYER).
    FourChannels,
    /// An quadruple of values with alpha. Obtained when setting `"layertype"`
    /// `"quad"` and `"withalpha"` `1` on an
    /// [`OutputLayer`](crate::OUTPUT_LAYER).
    FourChannelsAndAlpha,
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

    /// Returns `true`` if this layer contains an alpha channel.
    pub fn has_alpha(&self) -> bool {
        [
            LayerDepth::OneChannelAndAlpha,
            LayerDepth::ColorAndAlpha,
            LayerDepth::VectorAndAlpha,
            LayerDepth::FourChannelsAndAlpha,
        ]
        .contains(self)
    }
}

/// Accessor for the pixel format the renderer sends in
/// [`FnOpen`](crate::output::FnOpen), [`FnWrite`](crate::output::FnWrite) and
/// [`FnFinish`](crate::output::FnFinish)
///
/// This is a stack of [`Layer`]s. Where each layer describes an
/// [`OutputLayer`](crate::OUTPUT_LAYER).
///
/// # Example
///
/// A typical format for a pixel containing two such layers, an *RGBA* **color**
/// & **alpha** output layer and a world space **normal**, will look like this:
///
/// | [`name`](Layer::name()) | [`depth`](Layer::depth())                           | [`offset`](Layer::offset())
/// |-------------------------|-----------------------------------------------------|----------------------------
/// | `Ci`                    |[`ColorAndAlpha`](LayerDepth::ColorAndAlpha)(`rgba`) | `0`
/// | `N_world`               | [`Vector`](LayerDepth::Vector)(`xyz`)               | `4`
///
/// ## RAW Layout
///
/// The resp. callbacks deliver pixels as a flat [`f32`] buffer.
/// For the above example the actual layout of a single pixel in the
/// buffer is:
///
/// | Value  | `r`ed   | `g`reen | `b`lue  | `a`lpha | `x` | `y` | `z`
/// |--------|---------|---------|---------|---------|-----|-----|----
/// | Offset | `0`     | `1`     | `2`     | `3`     | `4` | `5` | `6`
///
/// The `offset` is the offset into the pixel buffer to obtain the 1st element.
/// For example, the **y** coordinate of the the normal will be stored in
/// channel at offset `5` (`4` + `1`).
///
/// The pixel format is in the order in which
/// [`OutputLayer`](crate::OUTPUT_LAYER)s were defined in the
/// [ɴsɪ scene](https://nsi.readthedocs.io/en/latest/guidelines.html#basic-scene-anatomy).
///
/// # Accessing Layers
///
/// To access the [`Layer`]s inside a `PixelFormat` use the [`Deref`] operator
/// to obtain the underlying [`Vec`]<`Layer`>.
///
/// # Examples
///
/// Dump all layers to `stdout` after a frame has finished rendering:
///
/// ```
/// # #[cfg(feature = "output")]
/// # {
/// # use nsi_ffi_wrap as nsi;
/// let finish = nsi::output::FinishCallback::new(
///     |_: String,
///      _: usize,
///      _: usize,
///      pixel_format: nsi::output::PixelFormat,
///      _: Vec<f32>| {
///         // Dump all layer descriptions to stdout.
///         for layer in &*pixel_format {
///             println!("{:?}", layer);
///         }
///
///         nsi::output::Error::None
///     },
/// );
/// # }
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PixelFormat(Vec<Layer>);

impl PixelFormat {
    #[inline]
    pub(crate) fn new(format: &[ndspy_sys::PtDspyDevFormat]) -> Self {
        if format.is_empty() {
            return PixelFormat::default();
        }

        let (mut previous_layer_name, mut previous_channel_id) =
            Self::split_into_layer_name_and_channel_id({
                // SAFETY: format.name should be a valid C string from the renderer
                let c_str = unsafe { CStr::from_ptr(format[0].name) };
                c_str.to_str().unwrap_or("<invalid>")
            });

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
                    // SAFETY: format.name should be a valid C string from the renderer
                    let name = match unsafe { CStr::from_ptr(format.1.name) }
                        .to_str()
                    {
                        Ok(n) => n,
                        Err(_) => "<invalid>",
                    };

                    let (layer_name, channel_id) =
                        Self::split_into_layer_name_and_channel_id(name);

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
                        // Do we we have a lonely alpha -> it belongs to the
                        // current layer.
                        if layer_name.is_empty() && "a" == channel_id {
                            depth = match &depth {
                                LayerDepth::OneChannel => {
                                    LayerDepth::OneChannelAndAlpha
                                }
                                LayerDepth::Color => LayerDepth::ColorAndAlpha,
                                LayerDepth::Vector => {
                                    LayerDepth::VectorAndAlpha
                                }
                                LayerDepth::FourChannels => {
                                    LayerDepth::FourChannelsAndAlpha
                                }
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
                                            LayerDepth::Color => {
                                                LayerDepth::ColorAndAlpha
                                            }
                                            LayerDepth::Vector => {
                                                LayerDepth::VectorAndAlpha
                                            }
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

    /// Returns the total number of channels in a pixel.
    /// This is the sum of the number of channels in all [`Layer`]s.
    #[inline]
    pub fn channels(&self) -> usize {
        self.0
            .iter()
            .fold(0, |total, layer| total + layer.channels())
    }
}

impl Deref for PixelFormat {
    type Target = Vec<Layer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Vec<Layer>> for PixelFormat {
    fn as_ref(&self) -> &Vec<Layer> {
        &self.0
    }
}
