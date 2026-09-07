//! Transformation matrices, in the layout ɴsɪ wants them.
//!
//! Separated from the node-creating helpers so the arithmetic can be
//! tested without a renderer. It could not be, before, and all three of
//! the bugs these tests now pin had shipped.
//!
//! # Layout
//!
//! `nsi.pdf`, under `transform.transformationmatrix`, draws the matrix
//! it wants:
//!
//! ```text
//! w11 w12 w13 0
//! w21 w22 w23 0
//! w31 w32 w33 0
//! Tx  Ty  Tz  1
//! ```
//!
//! Translation in the last row, so the last four values of the flat
//! array are `Tx, Ty, Tz, 1`. `ultraviolet` stores matrices
//! column-major, and puts a translation in the fourth column, so
//! `DMat4::from_translation(t).as_array()` already lands `Tx, Ty, Tz,
//! 1` in the last four slots -- the array matches ɴsɪ as it stands, and
//! nothing here transposes.
//!
//! That matters because the old [`rotation`](crate::rotation) *did*
//! transpose, alone among these, and the transpose of a rotation is its
//! inverse: it turned the wrong way. `tests/render.rs` settles it
//! against the renderer rather than against this reasoning.

use ultraviolet as uv;

/// A 4x4 matrix, flat, in the order ɴsɪ reads it.
pub type Matrix = [f64; 16];

/// Non-uniform scale.
pub fn scaling_matrix(scale: &[f64; 3]) -> Matrix {
    *uv::DMat4::from_nonuniform_scale(uv::DVec3::from(scale)).as_array()
}

/// Translation.
pub fn translation_matrix(translate: &[f64; 3]) -> Matrix {
    *uv::DMat4::from_translation(uv::DVec3::from(translate)).as_array()
}

/// Rotation of `angle` **degrees** about `axis`.
///
/// `axis` need not be normalised.
///
/// The conversion used to be `angle * TAU / 90.0`, which is four times
/// too large: a requested 90 degrees emitted a full 360 degree turn, so
/// the most natural thing to test with was exactly the case that
/// silently did nothing.
pub fn rotation_matrix(angle: f64, axis: &[f64; 3]) -> Matrix {
    *uv::DMat4::from_angle_plane(
        angle.to_radians() as _,
        uv::DBivec3::from_normalized_axis(uv::DVec3::from(axis).normalized()),
    )
    .as_array()
}

/// A transform placing a camera at `eye`, looking at `to`.
///
/// Inverted, because ɴsɪ wants the camera's placement in the world, not
/// a view matrix.
pub fn look_at_matrix(eye: &[f64; 3], to: &[f64; 3], up: &[f64; 3]) -> Matrix {
    *uv::DMat4::look_at(
        uv::DVec3::from(eye),
        uv::DVec3::from(to),
        uv::DVec3::from(up),
    )
    .inversed()
    .as_array()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies `matrix` to a point the way ɴsɪ's layout implies: the
    /// row vector `[x y z 1]` times the matrix.
    fn transform_point(matrix: &Matrix, point: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (column, value) in out.iter_mut().enumerate() {
            *value = point[0] * matrix[column]
                + point[1] * matrix[4 + column]
                + point[2] * matrix[8 + column]
                + matrix[12 + column];
        }
        out
    }

    fn assert_close(actual: [f64; 3], expected: [f64; 3]) {
        for axis in 0..3 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 1e-9,
                "{actual:?} != {expected:?}"
            );
        }
    }

    /// ɴsɪ draws `Tx Ty Tz 1` as the last row, so those must be the
    /// last four values of the flat array.
    #[test]
    fn translation_lands_in_the_last_row() {
        let matrix = translation_matrix(&[1.0, 2.0, 3.0]);
        assert_eq!([1.0, 2.0, 3.0, 1.0], matrix[12..16]);
        assert_close(transform_point(&matrix, [0.0; 3]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn scaling_is_diagonal() {
        let matrix = scaling_matrix(&[2.0, 3.0, 4.0]);
        assert_eq!(2.0, matrix[0]);
        assert_eq!(3.0, matrix[5]);
        assert_eq!(4.0, matrix[10]);
        assert_close(
            transform_point(&matrix, [1.0, 1.0, 1.0]),
            [2.0, 3.0, 4.0],
        );
    }

    /// The regression that matters most: 180 degrees must be a half
    /// turn. The old `angle * TAU / 90.0` made it 720 degrees -- the
    /// identity -- so a point at +X stayed at +X and the bug looked
    /// like "nothing happened".
    #[test]
    fn a_half_turn_about_y_reverses_x_and_z() {
        let matrix = rotation_matrix(180.0, &[0.0, 1.0, 0.0]);

        assert_close(
            transform_point(&matrix, [1.0, 0.0, 0.0]),
            [-1.0, 0.0, 0.0],
        );
        assert_close(
            transform_point(&matrix, [0.0, 0.0, 1.0]),
            [0.0, 0.0, -1.0],
        );
        // The axis itself is fixed.
        assert_close(
            transform_point(&matrix, [0.0, 1.0, 0.0]),
            [0.0, 1.0, 0.0],
        );
    }

    /// And a quarter turn must be a quarter turn, not a full one.
    #[test]
    fn a_quarter_turn_about_y_is_not_the_identity() {
        let matrix = rotation_matrix(90.0, &[0.0, 1.0, 0.0]);
        let moved = transform_point(&matrix, [1.0, 0.0, 0.0]);

        assert!(
            moved[0].abs() < 1e-9,
            "a quarter turn must clear the X axis, got {moved:?}"
        );
        assert!(
            moved[2].abs() > 0.999,
            "a quarter turn must land on Z, got {moved:?}"
        );
    }

    /// Four quarter turns are one full turn. Catches any scale error in
    /// the degree conversion without depending on the sign convention.
    #[test]
    fn four_quarter_turns_are_the_identity() {
        let quarter = rotation_matrix(90.0, &[0.3, 1.0, -0.7]);
        let point = [0.4, -1.2, 2.0];
        let mut moved = point;
        for _ in 0..4 {
            moved = transform_point(&quarter, moved);
        }
        assert_close(moved, point);
    }

    /// A rotation is orthonormal; its inverse is its transpose. If the
    /// matrix were transposed on the way out -- as `rotation` used to do
    /// -- this would still hold, so it is *not* what pins the
    /// direction. `tests/render.rs` does that.
    #[test]
    fn a_rotation_preserves_length() {
        let matrix = rotation_matrix(37.0, &[1.0, 2.0, 3.0]);
        let point = [1.0, -2.0, 0.5];
        let moved = transform_point(&matrix, point);

        let length =
            |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!((length(moved) - length(point)).abs() < 1e-9);
    }

    /// The camera's placement, not a view matrix: an eye at +Z looking
    /// at the origin must sit at +Z.
    #[test]
    fn look_at_places_the_camera_where_the_eye_is() {
        let matrix =
            look_at_matrix(&[0.0, 0.0, 5.0], &[0.0; 3], &[0.0, 1.0, 0.0]);
        assert_close(transform_point(&matrix, [0.0; 3]), [0.0, 0.0, 5.0]);
    }
}
