//! Pixel scalar type marker trait for zero-cost generic pixel handling.

/// Marker trait for pixel scalar types supported by ndspy.
///
/// This trait enables compile-time generic pixel handling with zero runtime
/// cost. Each implementation maps a Rust type to its corresponding ndspy
/// format constant.
///
/// # Safety
///
/// Implementors must ensure that:
/// - `NDSPY_TYPE` matches the memory layout of `Self`
/// - The type is `Copy`, `Default`, `Send`, `Sync`, and `'static`
///
/// These requirements are enforced by the trait bounds.
pub unsafe trait PixelType:
    Copy + Default + Send + Sync + 'static
{
    /// The ndspy type constant (PkDspyFloat32, PkDspyUnsigned8, etc.)
    const NDSPY_TYPE: u32;
}

// SAFETY: f32 matches PkDspyFloat32 (IEEE 754 single-precision)
unsafe impl PixelType for f32 {
    // PkDspyFloat32.
    const NDSPY_TYPE: u32 = 1;
}

// SAFETY: u32 matches PkDspyUnsigned32
unsafe impl PixelType for u32 {
    // PkDspyUnsigned32.
    const NDSPY_TYPE: u32 = 2;
}

// SAFETY: i32 matches PkDspySigned32
unsafe impl PixelType for i32 {
    // PkDspySigned32.
    const NDSPY_TYPE: u32 = 3;
}

// SAFETY: u16 matches PkDspyUnsigned16
unsafe impl PixelType for u16 {
    // PkDspyUnsigned16.
    const NDSPY_TYPE: u32 = 4;
}

// SAFETY: i16 matches PkDspySigned16
unsafe impl PixelType for i16 {
    // PkDspySigned16.
    const NDSPY_TYPE: u32 = 5;
}

// SAFETY: u8 matches PkDspyUnsigned8
unsafe impl PixelType for u8 {
    // PkDspyUnsigned8.
    const NDSPY_TYPE: u32 = 6;
}

// SAFETY: i8 matches PkDspySigned8
unsafe impl PixelType for i8 {
    // PkDspySigned8.
    const NDSPY_TYPE: u32 = 7;
}

// TODO: Add f16 support behind a feature flag
// #[cfg(feature = "half")]
// unsafe impl PixelType for half::f16 {
//     const NDSPY_TYPE: u32 = 12; // PkDspyFloat16
// }
