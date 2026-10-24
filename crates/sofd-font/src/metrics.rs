//! Font metrics and glyph outlines.

use serde::{Deserialize, Serialize};
use sofd_geometry::Path;

/// Global font metrics, in font units per em.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FontMetrics {
    /// Font units per em, usually 1000 or 2048.
    pub units_per_em: u16,
    /// Ascent above the baseline.
    pub ascender: f64,
    /// Descent below the baseline.
    pub descender: f64,
    /// Recommended extra leading between lines.
    pub line_gap: f64,
    /// Height of capital letters.
    pub cap_height: f64,
    /// Height of lowercase x.
    pub x_height: f64,
}

impl FontMetrics {
    /// Nominal line height for single spacing.
    pub fn line_height(&self) -> f64 {
        self.ascender - self.descender + self.line_gap
    }
}

/// Result of asking a face for one glyph's outline.
#[derive(Debug, Clone)]
pub enum GlyphOutline {
    /// A real outline, normalized to font units.
    Path(Path),
    /// No outline, as for spaces and control characters.
    Empty,
}

impl GlyphOutline {
    /// Extract the path, or `None` for an empty outline.
    pub fn into_path(self) -> Option<Path> {
        match self {
            Self::Path(p) => Some(p),
            Self::Empty => None,
        }
    }
}
