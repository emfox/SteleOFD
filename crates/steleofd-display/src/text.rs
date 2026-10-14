//! Text types: glyph runs and character-level text spans.

use serde::{Deserialize, Serialize};
use steleofd_common::{Millimeter, Rect};
use steleofd_font::FontId;
use steleofd_geometry::Affine;

/// A run of text sharing one font and transform.
/// Carries glyph ids and Unicode text together, since the OFD `TextCode` and
/// `CGTransform` mapping may be many-to-one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphRun {
    /// Font handle.
    pub font: FontId,
    /// Font size in millimetres.
    pub size: Millimeter,
    /// Glyph ids.
    pub glyphs: Vec<u16>,
    /// Horizontal advance per glyph in millimetres; same length as `glyphs`.
    pub advances: Vec<f64>,
    /// Original Unicode text; may be absent for subset fonts.
    pub text: Option<String>,
    /// Glyph-to-character index map, same length as `glyphs`.
    pub cluster_map: Option<Vec<u32>>,
    /// Text transform, combining position, rotation and scale.
    pub transform: Affine,
    /// Fill paint.
    pub fill: Option<crate::ops::Paint>,
    /// Stroke paint, used for outlined glyphs.
    pub stroke: Option<crate::ops::Paint>,
    /// Extra spacing between characters, in millimetres.
    pub letter_spacing: f64,
    /// Extra spacing between words, in millimetres.
    pub word_spacing: f64,
    /// Absolute page-space bounds, used for hit testing and search anchoring.
    pub bbox: Rect,
}

impl GlyphRun {
    /// Number of glyphs in the run.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the run contains no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Extracted plain text, or an empty string when unavailable.
    pub fn plain_text(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

/// A [`GlyphRun`] expanded to character granularity.
/// Search hits and annotations need per-character boxes while a run has only
/// one, so search and edit expand on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpan {
    /// Unicode text of this span, usually one character or one word.
    pub text: String,
    /// Absolute page-space bounds.
    pub bbox: Rect,
    /// Page this span belongs to.
    pub page: usize,
    /// Index of the source glyph run, for writing annotations back.
    pub run_index: usize,
    /// First glyph offset within the run.
    pub glyph_start: usize,
    /// Number of glyphs covered.
    pub glyph_count: usize,
}
