//! Output layers and image extents.
//!
//! One [`Layer`] describes one `outputlayer` node connected to the stream
//! `outputdriver`. Each layer is individually addressable in a publication
//! (US4); a publication never interleaves layers into a single plane.
//!
//! # Colorimetry
//!
//! Pixel data is **linear and scene-referred** in the layer's declared
//! format at every transport (`publication-lifecycle.md`, invariants). This
//! crate performs no conversion whatsoever -- no tone mapping, no transfer
//! function, no primaries change. The display transform is the client's
//! post stack (`spec.md`, non-goals).

/// Pixel format of one [`Layer`].
///
/// R6 requires RGBA f16 and f32 as the minimum set. The discriminants are
/// part of the version-1 shared-memory layout (see [`crate::transport::shm`])
/// and must not be renumbered without a `stream.version` bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum LayerFormat {
    /// Half-precision float per channel.
    #[default]
    RgbaF16 = 1,
    /// Single-precision float per channel.
    RgbaF32 = 2,
}

impl LayerFormat {
    /// Bytes occupied by one channel of one pixel.
    #[inline]
    pub const fn bytes_per_channel(self) -> usize {
        match self {
            Self::RgbaF16 => 2,
            Self::RgbaF32 => 4,
        }
    }

    /// Canonical name, as used in diagnostics and in the `outputlayer`
    /// `scalarformat`/`layertype` vocabulary.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RgbaF16 => "rgba16f",
            Self::RgbaF32 => "rgba32f",
        }
    }

    /// The version-1 wire discriminant.
    #[inline]
    pub const fn as_wire(self) -> u32 {
        self as u32
    }

    /// Decode a version-1 wire discriminant.
    #[inline]
    pub const fn from_wire(wire: u32) -> Option<Self> {
        match wire {
            1 => Some(Self::RgbaF16),
            2 => Some(Self::RgbaF32),
            _ => None,
        }
    }
}

impl core::fmt::Display for LayerFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Image dimensions in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Extent {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Extent {
    /// Construct an extent.
    #[inline]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Number of pixels.
    #[inline]
    pub const fn pixels(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Whether either dimension is zero.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl core::fmt::Display for Extent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// One connected `outputlayer`.
///
/// `name` is the ɴsɪ node handle, `variable_name` is the `outputlayer`
/// `variablename` attribute (the AOV the client asks for, e.g. `"Ci"` or
/// `"id.object"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layer {
    /// Node handle of the `outputlayer`.
    pub name: String,
    /// The `variablename` attribute of the `outputlayer`.
    pub variable_name: String,
    /// Declared pixel format.
    pub format: LayerFormat,
    /// Declared channel count (4 for RGBA).
    pub channels: u32,
}

impl Layer {
    /// Construct a layer.
    pub fn new(
        name: impl Into<String>,
        variable_name: impl Into<String>,
        format: LayerFormat,
        channels: u32,
    ) -> Self {
        Self {
            name: name.into(),
            variable_name: variable_name.into(),
            format,
            channels,
        }
    }

    /// Construct a four-channel RGBA layer.
    pub fn rgba(
        name: impl Into<String>,
        variable_name: impl Into<String>,
        format: LayerFormat,
    ) -> Self {
        Self::new(name, variable_name, format, 4)
    }

    /// Bytes occupied by one pixel of this layer.
    #[inline]
    pub const fn bytes_per_pixel(&self) -> usize {
        self.channels as usize * self.format.bytes_per_channel()
    }

    /// Bytes occupied by one row of `extent.width` pixels.
    #[inline]
    pub const fn row_bytes(&self, extent: Extent) -> usize {
        extent.width as usize * self.bytes_per_pixel()
    }

    /// Bytes occupied by one plane of this layer at `extent`.
    ///
    /// Planes are tightly packed: no row padding, no slice padding. This is
    /// part of the version-1 shared-memory layout.
    #[inline]
    pub const fn plane_bytes(&self, extent: Extent) -> usize {
        extent.pixels() * self.bytes_per_pixel()
    }
}

/// A rectangular region of one layer, as delivered by a renderer's bucket
/// callback.
///
/// Bucket writes are the only way pixels enter the driver's accumulation
/// buffer (see [`crate::ring`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bucket {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Bucket {
    /// Construct a bucket.
    #[inline]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The bucket covering a whole `extent`.
    #[inline]
    pub const fn full(extent: Extent) -> Self {
        Self::new(0, 0, extent.width, extent.height)
    }

    /// Whether the bucket fits inside `extent`.
    #[inline]
    pub const fn fits(&self, extent: Extent) -> bool {
        self.x.saturating_add(self.width) <= extent.width
            && self.y.saturating_add(self.height) <= extent.height
    }

    /// Number of pixels covered.
    #[inline]
    pub const fn pixels(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_size_follows_declared_format() {
        let extent = Extent::new(16, 8);
        let f16 = Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16);
        let f32 = Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32);

        assert_eq!(f16.plane_bytes(extent), 16 * 8 * 4 * 2);
        assert_eq!(f32.plane_bytes(extent), 16 * 8 * 4 * 4);
    }

    #[test]
    fn format_wire_round_trip() {
        [LayerFormat::RgbaF16, LayerFormat::RgbaF32]
            .into_iter()
            .for_each(|format| {
                assert_eq!(
                    LayerFormat::from_wire(format.as_wire()),
                    Some(format)
                );
            });

        assert_eq!(LayerFormat::from_wire(0), None);
    }
}
