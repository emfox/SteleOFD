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

    /// Enclosed area.
    pub fn area(self) -> f64 {
        self.w * self.h
    }

    /// Pixel dimensions at the given resolution, never smaller than one pixel.
    pub fn size_px(self, dpi: f64) -> (u32, u32) {
        let w = (<Millimeter as Unit>::mm_to_px(self.w, dpi).ceil() as u32).max(1);
        let h = (<Millimeter as Unit>::mm_to_px(self.h, dpi).ceil() as u32).max(1);
        (w, h)
    }

    /// Smallest rectangle containing both inputs.
    pub fn union(self, other: Self) -> Self {
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
