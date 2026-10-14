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
    pub const IDENTITY: Self = Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

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
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
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
