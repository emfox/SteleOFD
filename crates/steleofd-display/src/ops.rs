//! Draw commands and their paint attributes.
//! The shared vocabulary of every backend: adding one means translating these
//! commands, not understanding the OFD source format.

use serde::{Deserialize, Serialize};
use steleofd_common::{Point, Rect};
use steleofd_font::FontId;
use steleofd_geometry::{Affine, Path};

use crate::resource::ImageRef;
use crate::text::GlyphRun;

/// A single drawing command.
/// Coordinates are already flattened to absolute page space in millimetres;
/// no command performs nested lookup or resource resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawOp {
    /// Push the current graphics state.
    Save,

    /// Pop the graphics state.
    Restore,

    /// Concatenate a transform onto the current state.
    Transform(Affine),

    /// Intersect the clip with a path.
    ClipPath {
        /// Clip geometry.
        path: Path,
        /// Fill rule used to interpret the path.
        rule: PaintRule,
    },

    /// Intersect the clip with the outlines of a text run. The region is the
    /// glyph shapes themselves, so a backend resolves them the way it resolves
    /// drawn text; a run whose font cannot be found clips nothing.
    ClipText(GlyphRun),

    /// Fill and/or stroke a path.
    Path {
        /// Path geometry.
        path: Path,
        /// Fill paint; `None` leaves the interior untouched.
        fill: Option<Paint>,
        /// Stroke paint; `None` leaves the outline untouched.
        stroke: Option<Paint>,
        /// Stroke geometry parameters.
        stroke_style: StrokeStyle,
        /// Fill rule.
        rule: PaintRule,
    },

    /// Draw shaped text.
    Text(GlyphRun),

    /// Draw an image.
    Image {
        /// Resolved image source.
        image: ImageRef,
        /// Destination rectangle in absolute page space.
        dst: Rect,
        /// Placement transform.
        transform: Affine,
    },

    /// Draw a linear gradient clipped to a path.
    LinearGradient {
        /// Gradient start point.
        start: Point,
        /// Gradient end point.
        end: Point,
        /// Colour stops.
        stops: Vec<GradientStop>,
        /// Region the gradient is painted into.
        path: Path,
    },

    /// Draw a radial gradient clipped to a path.
    RadialGradient {
        /// Centre of the start circle.
        center: Point,
        /// Radius of the start circle.
        radius: f64,
        /// Centre of the end circle.
        end: Point,
        /// Radius of the end circle.
        end_radius: f64,
        /// Colour stops.
        stops: Vec<GradientStop>,
        /// Region the gradient is painted into.
        path: Path,
    },
}

/// Straight sRGB colour with alpha, each channel in `0..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; 255 is opaque.
    pub a: u8,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Build a colour from raw channels.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse `#RRGGBB` or `RRGGBB`; alpha defaults to opaque.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(Self {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
            a: 255,
        })
    }
}

/// How a region is filled or stroked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Paint {
    /// Uniform colour.
    Solid(Color),
    /// Index into the gradient table built from [`DrawOp::LinearGradient`].
    LinearGradientRef(usize),
    /// Index into the gradient table built from [`DrawOp::RadialGradient`].
    RadialGradientRef(usize),
    /// Tiled image.
    Pattern(ImageRef),
}

/// One colour stop of a gradient.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position along the gradient, `0.0..=1.0`.
    pub offset: f64,
    /// Colour at this position.
    pub color: Color,
}

/// Rule deciding which regions a path covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaintRule {
    /// Non-zero winding.
    #[default]
    NonZero,
    /// Even-odd.
    EvenOdd,
}

/// Stroke geometry parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeStyle {
    /// Line width in millimetres.
    pub width: f64,
    /// Dash pattern; empty means solid.
    pub dash: Vec<f64>,
    /// Offset into the dash pattern, in millimetres.
    pub dash_offset: f64,
    /// End cap style.
    pub cap: LineCap,
    /// Segment join style.
    pub join: LineJoin,
    /// Miter length limit before a join is bevelled.
    pub miter_limit: f64,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            dash: Vec::new(),
            dash_offset: 0.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// How an open subpath is terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineCap {
    /// Stop exactly at the endpoint.
    #[default]
    Butt,
    /// Extend by a half-circle.
    Round,
    /// Extend by half the line width.
    Square,
}

/// How two segments meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineJoin {
    /// Extend to a sharp corner.
    #[default]
    Miter,
    /// Round the corner.
    Round,
    /// Cut the corner flat.
    Bevel,
}

/// Opaque handle letting a backend look up font data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontHandle(pub FontId);
