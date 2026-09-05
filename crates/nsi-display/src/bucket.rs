//! One region of pixels, as handed to `DspyImageData`.

use nsi_ffi_wrap::output::PixelType;

/// A rectangular region of the image, with its pixel data.
///
/// The data is the **bucket only**, not the full image, and is borrowed
/// from the renderer for the duration of the call.
#[derive(Copy, Clone)]
pub struct Bucket<'a, T: PixelType> {
    x_min: usize,
    x_max: usize,
    y_min: usize,
    y_max: usize,
    channels: usize,
    pixels: &'a [T],
}

impl<'a, T: PixelType> Bucket<'a, T> {
    /// # Panics
    /// If `pixels` is not exactly `width * height * channels` long.
    /// The shim computes it from the same numbers it passes here, so a
    /// mismatch is a bug in this crate, not in the author's driver.
    pub fn new(
        x_min: usize,
        x_max: usize,
        y_min: usize,
        y_max: usize,
        channels: usize,
        pixels: &'a [T],
    ) -> Self {
        let expected = (x_max - x_min) * (y_max - y_min) * channels;
        assert_eq!(
            expected,
            pixels.len(),
            "bucket geometry disagrees with the pixel slice"
        );
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            channels,
            pixels,
        }
    }

    #[inline]
    pub fn x_min(&self) -> usize {
        self.x_min
    }

    #[inline]
    pub fn x_max(&self) -> usize {
        self.x_max
    }

    #[inline]
    pub fn y_min(&self) -> usize {
        self.y_min
    }

    #[inline]
    pub fn y_max(&self) -> usize {
        self.y_max
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.x_max - self.x_min
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.y_max - self.y_min
    }

    #[inline]
    pub fn channels(&self) -> usize {
        self.channels
    }

    #[inline]
    pub fn pixels(&self) -> &'a [T] {
        self.pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixel slice is exactly the announced region, channels
    /// included. Getting this wrong is a buffer overread, so the
    /// constructor asserts it.
    #[test]
    fn geometry_matches_the_pixel_slice() {
        let pixels = [0.5f32; 2 * 3 * 4]; // 2x3 pixels, 4 channels
        let bucket = Bucket::new(0, 2, 0, 3, 4, &pixels);

        assert_eq!(2, bucket.width());
        assert_eq!(3, bucket.height());
        assert_eq!(4, bucket.channels());
        assert_eq!(pixels.len(), bucket.pixels().len());
    }

    #[test]
    #[should_panic(expected = "bucket geometry")]
    fn a_mismatched_slice_is_rejected() {
        let pixels = [0.5f32; 3];
        let _ = Bucket::new(0, 2, 0, 3, 4, &pixels);
    }
}
