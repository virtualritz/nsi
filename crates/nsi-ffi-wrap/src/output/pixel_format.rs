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
/// The resp. callbacks deliver pixels as a flat [`prim@f32`] buffer.
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
///      pixel_format: nsi::output::PixelFormat| {
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
    /// Groups the renderer's flat channel list into [`Layer`]s.
    ///
    /// ndspy names each channel `<layer>.<channel>`, so a layer boundary
    /// is a change of the *name* part -- which is what this reads. The
    /// previous implementation instead guessed boundaries from the
    /// channel part, opening a layer only on `r`, `x` or `s`; a layer
    /// led by anything else (`depth.z`, say) never opened one, so it was
    /// merged into its predecessor, and with
    /// `["Ci.r","Ci.g","Ci.b","Ci.a","albedo.r","albedo.g","albedo.b",
    /// "N.x","N.y","N.z","depth.z"]` the whole `N` layer disappeared:
    /// its name was overwritten by `depth` and a channel was lost.
    ///
    /// The one place a name change is *not* a boundary is a bare `a`
    /// following a named layer -- that is the layer's alpha.
    ///
    /// ndspy passes exactly one `PtDspyDevFormat` per channel, so
    /// [`channels()`](PixelFormat::channels) always equals `format.len()`
    /// by construction here. That is not cosmetic: `nsi-display`'s
    /// `shim_data` uses it as the length of a slice over the renderer's
    /// bucket buffer, so over-reporting reads out of bounds and
    /// under-reporting silently drops AOVs.
    #[inline]
    pub(crate) fn new(format: &[ndspy_sys::PtDspyDevFormat]) -> Self {
        let mut layers = Vec::<Layer>::new();
        // The layer being accumulated: its name, and one entry per
        // channel seen so far.
        let mut name = "";
        let mut channel_ids = Vec::<&str>::new();
        let mut offset = 0;

        let mut flush = |name: &str, ids: &mut Vec<&str>, offset: &mut usize| {
            for depth in Self::depths_for(ids.len(), ids.first().copied()) {
                let unnamed = if layers.is_empty() {
                    // The first unnamed layer is the beauty pass.
                    "Ci"
                } else {
                    // A later one has no name to take: 3Delight sends
                    // built-in variables unprefixed, so all this layer
                    // is known by is its first channel.
                    ids.first().copied().unwrap_or("Ci")
                };
                layers.push(Layer {
                    name: if name.is_empty() { unnamed } else { name }
                        .to_string(),
                    depth,
                    offset: *offset,
                });
                *offset += depth.channels();
            }
            ids.clear();
        };

        for entry in format {
            // SAFETY: `name` is a valid C string from the renderer.
            let channel = unsafe { CStr::from_ptr(entry.name) }
                .to_str()
                .unwrap_or("<invalid>");
            let (layer_name, channel_id) =
                Self::split_into_layer_name_and_channel_id(channel);

            // Two ways a channel can belong to the layer being
            // accumulated. Its name must match -- and when there are no
            // names, as 3Delight does for built-in variables, its role
            // must continue the sequence.
            // A bare `a` is the previous layer's alpha even though it
            // carries no name of its own.
            let named_the_same = layer_name == name
                || (layer_name.is_empty() && "a" == channel_id);
            let continues =
                named_the_same && Self::continues(&channel_ids, channel_id);

            if !channel_ids.is_empty() && !continues {
                flush(name, &mut channel_ids, &mut offset);
            }
            if channel_ids.is_empty() {
                name = layer_name;
            }
            channel_ids.push(channel_id);
        }
        flush(name, &mut channel_ids, &mut offset);

        PixelFormat(layers)
    }

    /// Whether `channel_id` continues the layer accumulated so far.
    ///
    /// 3Delight names channels of built-in variables with no layer
    /// prefix at all -- a beauty plus a depth arrives as
    /// `["r","g","b","a","z"]`, measured, not assumed. So when the name
    /// part cannot separate two layers, their channel *roles* must:
    /// `r`,`g`,`b` and `x`,`y`,`z` are positional, so a channel
    /// continues only from the position its predecessor left off, and
    /// `a` closes a layer rather than opening one. A trailing `z` after
    /// `r`,`g`,`b`,`a` is therefore a new layer -- it cannot be the
    /// third channel of a group that already holds four.
    fn continues(accumulated: &[&str], channel_id: &str) -> bool {
        if accumulated.is_empty() {
            return true;
        }
        // An alpha joins whatever it follows, and closes it.
        if "a" == channel_id {
            return accumulated.len() < 5
                && accumulated.last() != Some(&"a");
        }
        let position = |id: &str| match id {
            "r" | "x" => Some(0),
            "g" | "y" => Some(1),
            "b" | "z" => Some(2),
            _ => None,
        };
        let family = |id: &str| match id {
            "r" | "g" | "b" => Some(0),
            "x" | "y" | "z" => Some(1),
            _ => None,
        };
        match (position(channel_id), accumulated.first()) {
            (Some(next), Some(first)) => {
                next == accumulated.len() && family(channel_id) == family(first)
            }
            // A scalar -- an indexed channel, say -- is a layer of its own.
            _ => false,
        }
    }

    /// The [`LayerDepth`]s spanning `count` channels, whose first is
    /// `first` (`x`, `y` or `z` meaning a vector rather than a colour).
    ///
    /// Normally one depth. ɴsɪ's layer types top out at five channels
    /// (`quad` with alpha), and `LayerDepth` can represent no more, so a
    /// longer run -- which 3Delight does not currently emit -- is split
    /// into four-channel pieces sharing the layer's name. Splitting
    /// rather than truncating keeps the total channel count equal to the
    /// number of format entries, which is the invariant memory safety
    /// downstream rests on.
    fn depths_for(count: usize, first: Option<&str>) -> Vec<LayerDepth> {
        let is_vector = matches!(first, Some("x" | "y" | "z"));
        let mut depths = Vec::new();
        let mut left = count;
        while left > 5 {
            depths.push(LayerDepth::FourChannels);
            left -= 4;
        }
        depths.extend(match (left, is_vector) {
            (0, _) => None,
            (1, _) => Some(LayerDepth::OneChannel),
            (2, _) => Some(LayerDepth::OneChannelAndAlpha),
            (3, true) => Some(LayerDepth::Vector),
            (3, false) => Some(LayerDepth::Color),
            (4, true) => Some(LayerDepth::VectorAndAlpha),
            (4, false) => Some(LayerDepth::ColorAndAlpha),
            (5, _) => Some(LayerDepth::FourChannelsAndAlpha),
            _ => unreachable!("the loop above leaves at most five"),
        });
        depths
    }

    /// Builds a `PixelFormat` from the format array ndspy hands a
    /// display driver.
    ///
    /// Out-of-crate drivers (see the `nsi-display` crate) receive
    /// `PtDspyDevFormat[]` in `DspyImageOpen` and need this to interpret
    /// the buckets that follow.
    #[inline]
    pub fn from_ndspy(format: &[ndspy_sys::PtDspyDevFormat]) -> Self {
        Self::new(format)
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
        // Get the layer name if there are more parts.
        // For 2-part names like "beauty.r", this is "beauty".
        // For 3+ part names, this is the leftmost part (skipping middle).
        match split.last() {
            Some(prefix) => (prefix, postfix),
            None => ("", postfix),
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
