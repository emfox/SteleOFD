//! Affine transform matrices.
//! Uses the six-parameter convention shared by PDF and OFD, mapping a point as
//! `x' = a*x + c*y + e` and `y' = b*x + d*y + f`.

use serde::{Deserialize, Serialize};

/// A 2D affine transform.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    /// Horizontal scale and rotation component.
    pub a: f64,
    /// Vertical shear component.
    pub b: f64,
    /// Horizontal shear component.
    pub c: f64,
    /// Vertical scale and rotation component.
    pub d: f64,
    /// Horizontal translation in millimetres.
    pub e: f64,
    /// Vertical translation in millimetres.
    pub f: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    /// The identity transform.
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Build from raw coefficients.
    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Pure translation.
    pub const fn translate(tx: f64, ty: f64) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    /// Pure scaling.
    pub const fn scale(sx: f64, sy: f64) -> Self {
        Self::new(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    /// Rotation by an angle in radians.
    pub fn rotate(rad: f64) -> Self {
        let (s, c) = rad.sin_cos();
        Self::new(c, s, -s, c, 0.0, 0.0)
    }

    /// Rotation by an angle in degrees.
    pub fn rotate_deg(deg: f64) -> Self {
        Self::rotate(deg.to_radians())
    }

    /// Compose, applying `self` first and `next` second.
    /// OFD CTM semantics left-multiply: a child CTM is the parent CTM times the
    /// local transform.
    pub fn then(self, next: Self) -> Self {
        Self {
            a: self.a * next.a + self.b * next.c,
            b: self.a * next.b + self.b * next.d,
            c: self.c * next.a + self.d * next.c,
            d: self.c * next.b + self.d * next.d,
            e: self.e * next.a + self.f * next.c + next.e,
            f: self.e * next.b + self.f * next.d + next.f,
        }
    }

    /// Transform a point.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Transform a direction, ignoring translation.
    pub fn apply_vector(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    /// Determinant of the linear part.
    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Whether the transform can be inverted.
    pub fn is_invertible(&self) -> bool {
        self.determinant().abs() > 1e-12
    }

    /// Inverse transform, or `None` when singular.
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() <= 1e-12 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            e: (self.c * self.f - self.d * self.e) * inv_det,
            f: (self.b * self.e - self.a * self.f) * inv_det,
        })
    }

    /// Whether this is effectively the identity, so the op can be skipped.
    pub fn is_identity(&self) -> bool {
        const EPS: f64 = 1e-9;
        (self.a - 1.0).abs() < EPS
            && self.b.abs() < EPS
            && self.c.abs() < EPS
            && (self.d - 1.0).abs() < EPS
            && self.e.abs() < EPS
            && self.f.abs() < EPS
    }

    /// Whether the transform is translation only, which enables fast paths.
    pub fn is_translation_only(&self) -> bool {
        const EPS: f64 = 1e-9;
        (self.a - 1.0).abs() < EPS
            && self.b.abs() < EPS
            && self.c.abs() < EPS
            && (self.d - 1.0).abs() < EPS
    }

    /// Average scale factor, for mapping line widths and font sizes to device space.
    pub fn scale_factor(&self) -> f64 {
        let sx = (self.a * self.a + self.b * self.b).sqrt();
        let sy = (self.c * self.c + self.d * self.d).sqrt();
        ((sx * sy).abs()).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    /// Compare two points, allowing for floating point drift.
    fn assert_point_eq(actual: (f64, f64), expected: (f64, f64)) {
        assert!(
            (actual.0 - expected.0).abs() < EPS && (actual.1 - expected.1).abs() < EPS,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn identity_leaves_points_unchanged() {
        assert_point_eq(Affine::IDENTITY.apply(3.0, 4.0), (3.0, 4.0));
        assert!(Affine::IDENTITY.is_identity());
    }

    #[test]
    fn default_is_identity() {
        assert_eq!(Affine::default(), Affine::IDENTITY);
    }

    #[test]
    fn translate_shifts_points() {
        let t = Affine::translate(10.0, -5.0);
        assert_point_eq(t.apply(0.0, 0.0), (10.0, -5.0));
        assert_point_eq(t.apply(1.0, 2.0), (11.0, -3.0));
    }

    #[test]
    fn scale_multiplies_coordinates() {
        let s = Affine::scale(2.0, 3.0);
        assert_point_eq(s.apply(4.0, 5.0), (8.0, 15.0));
    }

    #[test]
    fn rotate_deg_turns_a_quarter_circle() {
        let r = Affine::rotate_deg(90.0);
        let (x, y) = r.apply(1.0, 0.0);
        assert!((x - 0.0).abs() < EPS, "x was {x}");
        assert!((y - 1.0).abs() < EPS, "y was {y}");
    }

    #[test]
    fn apply_vector_ignores_translation() {
        let t = Affine::new(2.0, 0.0, 0.0, 2.0, 100.0, 100.0);
        assert_point_eq(t.apply_vector(1.0, 1.0), (2.0, 2.0));
        assert_point_eq(t.apply(1.0, 1.0), (102.0, 102.0));
    }

    #[test]
    fn then_applies_self_before_next() {
        let translate = Affine::translate(10.0, 0.0);
        let scale = Affine::scale(2.0, 2.0);

        // Scale first, then translate: (1, 0) -> (2, 0) -> (12, 0).
        assert_point_eq(scale.then(translate).apply(1.0, 0.0), (12.0, 0.0));
        // Translate first, then scale: (1, 0) -> (11, 0) -> (22, 0).
        assert_point_eq(translate.then(scale).apply(1.0, 0.0), (22.0, 0.0));
    }

    #[test]
    fn then_is_associative() {
        let a = Affine::translate(3.0, 4.0);
        let b = Affine::scale(2.0, 5.0);
        let c = Affine::rotate_deg(37.0);

        let left = a.then(b).then(c);
        let right = a.then(b.then(c));
        assert!((left.a - right.a).abs() < EPS);
        assert!((left.b - right.b).abs() < EPS);
        assert!((left.c - right.c).abs() < EPS);
        assert!((left.d - right.d).abs() < EPS);
        assert!((left.e - right.e).abs() < EPS);
        assert!((left.f - right.f).abs() < EPS);
    }

    #[test]
    fn identity_is_neutral_for_then() {
        let t = Affine::new(1.5, 0.5, -0.5, 1.5, 3.0, 7.0);
        let left = Affine::IDENTITY.then(t);
        let right = t.then(Affine::IDENTITY);
        for (x, y) in [(left.a, right.a), (left.b, right.b), (left.c, right.c)] {
            assert!((x - y).abs() < EPS);
        }
        assert_point_eq(left.apply(2.0, 3.0), t.apply(2.0, 3.0));
        assert_point_eq(right.apply(2.0, 3.0), t.apply(2.0, 3.0));
    }

    #[test]
    fn determinant_is_one_for_rotation_and_translation() {
        assert!((Affine::rotate_deg(123.0).determinant() - 1.0).abs() < EPS);
        assert!((Affine::translate(9.0, -2.0).determinant() - 1.0).abs() < EPS);
        assert!((Affine::scale(3.0, 4.0).determinant() - 12.0).abs() < EPS);
    }

    #[test]
    fn inverse_round_trips_a_point() {
        let t = Affine::new(1.5, 0.25, -0.75, 2.0, 12.0, -8.0);
        let inv = t.inverse().expect("invertible");
        let (x, y) = t.apply(5.0, 7.0);
        assert_point_eq(inv.apply(x, y), (5.0, 7.0));
    }

    #[test]
    fn inverse_composed_with_original_is_identity() {
        let t = Affine::new(0.5, 0.0, 0.0, 0.5, 20.0, 30.0);
        let inv = t.inverse().expect("invertible");
        let product = inv.then(t);
        assert_point_eq(product.apply(11.0, -4.0), (11.0, -4.0));
        assert!(product.is_identity());
    }

    #[test]
    fn singular_transform_has_no_inverse() {
        let degenerate = Affine::scale(0.0, 5.0);
        assert!(!degenerate.is_invertible());
        assert!(degenerate.inverse().is_none());
    }

    #[test]
    fn is_identity_tolerates_tiny_drift() {
        assert!(Affine::new(1.0 + 1e-12, 0.0, 0.0, 1.0, -1e-12, 0.0).is_identity());
        assert!(!Affine::new(1.001, 0.0, 0.0, 1.0, 0.0, 0.0).is_identity());
    }

    #[test]
    fn is_translation_only_ignores_translation_but_not_scale() {
        assert!(Affine::translate(5.0, 6.0).is_translation_only());
        assert!(Affine::IDENTITY.is_translation_only());
        assert!(!Affine::scale(2.0, 1.0).is_translation_only());
        assert!(!Affine::rotate_deg(10.0).is_translation_only());
    }

    #[test]
    fn scale_factor_averages_both_axes() {
        // Uniform scale is reported exactly.
        assert!((Affine::scale(3.0, 3.0).scale_factor() - 3.0).abs() < EPS);
        // Non-uniform scales average geometrically: sqrt(4 * 16) == 8.
        assert!((Affine::scale(4.0, 16.0).scale_factor() - 8.0).abs() < EPS);
        // Rotation preserves unit scale.
        assert!((Affine::rotate_deg(45.0).scale_factor() - 1.0).abs() < EPS);
    }
}
