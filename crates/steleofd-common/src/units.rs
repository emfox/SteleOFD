//! Length units and geometric primitives, plain values with no third-party deps.

use serde::{Deserialize, Serialize};

/// Base length unit of the OFD standard: millimetres.
/// GB/T 33190-2016 measures everything in millimetres, so values are stored that
/// way and converted to pixels only at render time, keeping DPI out of parsing.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millimeter(pub f64);

/// Conversion helpers and unit constants.
pub trait Unit {
    /// Millimetres in one inch.
    const MM_PER_INCH: f64 = 25.4;
    /// Millimetres in one typographic point.
    const MM_PER_POINT: f64 = 25.4 / 72.0;

    /// Convert millimetres to pixels at the given resolution.
    fn mm_to_px(mm: f64, dpi: f64) -> f64 {
        mm / Self::MM_PER_INCH * dpi
    }

    /// Convert pixels to millimetres at the given resolution.
    fn px_to_mm(px: f64, dpi: f64) -> f64 {
        px * Self::MM_PER_INCH / dpi
    }
}

impl Unit for Millimeter {}

impl Millimeter {
    /// Wrap a raw value.
    pub const fn new(v: f64) -> Self {
        Self(v)
    }
    /// Unwrap the raw value.
    pub const fn value(self) -> f64 {
        self.0
    }
    /// Convert to pixels at the given resolution.
    pub fn to_px(self, dpi: f64) -> f64 {
        <Self as Unit>::mm_to_px(self.0, dpi)
    }
    /// Build from pixels at the given resolution.
    pub fn from_px(px: f64, dpi: f64) -> Self {
        Self(<Self as Unit>::px_to_mm(px, dpi))
    }
}

/// A point in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Point {
    /// Build a point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    /// The origin.
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };
}

/// An axis-aligned rectangle in millimetres.
///
/// Maps to OFD `Boundary`, `PhysicalBox` and similar attributes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// Build a rectangle.
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// A4 portrait, 210 by 297 millimetres.
    pub const A4_PORTRAIT: Self = Self::new(0.0, 0.0, 210.0, 297.0);

    /// A4 landscape, 297 by 210 millimetres.
    pub const A4_LANDSCAPE: Self = Self::new(0.0, 0.0, 297.0, 210.0);

    /// The empty rectangle, the identity for [`Rect::union`].
    /// Not the same as a zero-sized rectangle at the origin: that one is a
    /// legitimate position and would drag the union towards (0, 0).
    pub const EMPTY: Self = Self::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );

    /// Whether this is [`Rect::EMPTY`] rather than a real region.
    pub fn is_empty(self) -> bool {
        self.w < 0.0 || self.h < 0.0
    }

    /// Enclosed area.
    pub fn area(self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.w * self.h
        }
    }

    /// Pixel dimensions at the given resolution, never smaller than one pixel.
    pub fn size_px(self, dpi: f64) -> (u32, u32) {
        let w = (<Millimeter as Unit>::mm_to_px(self.w, dpi).ceil() as u32).max(1);
        let h = (<Millimeter as Unit>::mm_to_px(self.h, dpi).ceil() as u32).max(1);
        (w, h)
    }

    /// Smallest rectangle containing both inputs; [`Rect::EMPTY`] is absorbed.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w).max(other.x + other.w);
        let y2 = (self.y + self.h).max(other.y + other.h);
        Self::new(x1, y1, x2 - x1, y2 - y1)
    }

    /// Whether the two rectangles overlap; used for culling and tiled render.
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn millimetre_round_trips_through_wrapping() {
        let m = Millimeter::new(12.5);
        assert!((m.value() - 12.5).abs() < EPS);
    }

    #[test]
    fn mm_to_px_at_96_dpi_matches_the_css_pixel() {
        // 1 inch is 25.4 mm, so 25.4 mm at 96 dpi is 96 px.
        assert!((Millimeter::new(25.4).to_px(96.0) - 96.0).abs() < EPS);
    }

    #[test]
    fn px_to_mm_inverts_mm_to_px() {
        let original = 37.0;
        let px = Millimeter::new(original).to_px(300.0);
        let back = Millimeter::from_px(px, 300.0);
        assert!((back.value() - original).abs() < EPS);
    }

    #[test]
    fn a4_width_is_about_8_27_inches() {
        // 210 mm / 25.4 = 8.2677...
        let inches = Millimeter::new(210.0).to_px(25.4);
        assert!((inches - 210.0).abs() < EPS);
    }

    #[test]
    fn rect_area_multiplies_sides() {
        assert!((Rect::new(2.0, 3.0, 4.0, 5.0).area() - 20.0).abs() < EPS);
    }

    #[test]
    fn a4_portrait_dimensions_are_210_by_297() {
        assert!((Rect::A4_PORTRAIT.w - 210.0).abs() < EPS);
        assert!((Rect::A4_PORTRAIT.h - 297.0).abs() < EPS);
    }

    #[test]
    fn size_px_rounds_up_and_never_reaches_zero() {
        // 210 mm at 96 dpi is 793.7 px, rounded up to 794.
        let (w, h) = Rect::A4_PORTRAIT.size_px(96.0);
        assert_eq!(w, 794);
        assert_eq!(h, 1123);
        // A degenerate box still yields one pixel rather than zero.
        assert_eq!(Rect::new(0.0, 0.0, 0.0, 0.0).size_px(96.0), (1, 1));
    }

    #[test]
    fn union_covers_both_rectangles() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        let u = a.union(b);
        assert!((u.x - 0.0).abs() < EPS);
        assert!((u.y - 0.0).abs() < EPS);
        assert!((u.w - 15.0).abs() < EPS);
        assert!((u.h - 15.0).abs() < EPS);
    }

    #[test]
    fn union_with_a_disjoint_rect_spans_the_gap() {
        let a = Rect::new(0.0, 0.0, 1.0, 1.0);
        let b = Rect::new(10.0, 10.0, 1.0, 1.0);
        let u = a.union(b);
        assert!((u.w - 11.0).abs() < EPS);
        assert!((u.h - 11.0).abs() < EPS);
    }

    #[test]
    fn intersect_detects_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(a.intersects(Rect::new(-5.0, -5.0, 10.0, 10.0)));
    }

    #[test]
    fn touching_edges_do_not_count_as_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(Rect::new(10.0, 0.0, 10.0, 10.0)));
        assert!(!a.intersects(Rect::new(0.0, 10.0, 10.0, 10.0)));
    }

    #[test]
    fn a_rect_intersects_itself() {
        let a = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert!(a.intersects(a));
    }

    #[test]
    fn point_origin_is_zero() {
        assert_eq!(Point::ORIGIN, Point::new(0.0, 0.0));
    }
}
