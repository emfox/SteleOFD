//! Text shaping.
//! Producers already select glyphs, so the read path renders from ids; shaping
//! covers inserted text, re-selection after font substitution and export.

use std::sync::Arc;

use steleofd_error::Result;

use crate::registry::FontId;
use crate::{FontFace, FontManager};

/// Options controlling a shaping pass.
#[derive(Debug, Clone)]
pub struct ShapeOptions {
    /// Font size in millimetres.
    pub size_mm: f64,
    /// Whether codepoints absent from the face may use another face.
    pub allow_fallback: bool,
    /// BCP 47 tag affecting CJK punctuation and ligature rules.
    pub language: Option<String>,
    /// Whether to form ligatures.
    pub ligatures: bool,
}

impl Default for ShapeOptions {
    fn default() -> Self {
        Self {
            size_mm: 3.53, // 10 pt
            allow_fallback: true,
            language: Some("zh-Hans".into()),
            ligatures: false,
        }
    }
}

/// One shaped glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    /// Glyph id in the face actually used.
    pub glyph_id: u16,
    /// Advance in millimetres, already scaled by font size.
    pub advance_mm: f64,
    /// Character index in the source string, not a byte offset.
    pub cluster: u32,
    /// Face that supplied this glyph, which may differ after fallback.
    pub font: FontId,
}

/// Result of shaping one string.
#[derive(Debug, Clone, Default)]
pub struct ShapedRun {
    /// Shaped glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Source text, kept for search, copy and export.
    pub text: String,
    /// Summed advance in millimetres.
    pub total_advance: f64,
    /// Characters dropped because no face had a glyph, for diagnostics.
    pub skipped_chars: Vec<char>,
}

impl ShapedRun {
    /// Number of glyphs produced.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether no glyphs were produced.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Whether more than one face was used, indicating substitution.
    pub fn has_fallback(&self) -> bool {
        let mut it = self.glyphs.iter().map(|g| g.font);
        match it.next() {
            Some(first) => it.any(|f| f != first),
            None => false,
        }
    }

    /// Whether any character was dropped, which means text may differ from source.
    pub fn has_missing_glyphs(&self) -> bool {
        !self.skipped_chars.is_empty()
    }

    /// Advances in millimetres, for building a display list.
    pub fn advances(&self) -> Vec<f64> {
        self.glyphs.iter().map(|g| g.advance_mm).collect()
    }

    /// Glyph ids, for building a display list.
    pub fn glyph_ids(&self) -> Vec<u16> {
        self.glyphs.iter().map(|g| g.glyph_id).collect()
    }

    /// Cluster indices, for building a display list.
    pub fn clusters(&self) -> Vec<u32> {
        self.glyphs.iter().map(|g| g.cluster).collect()
    }
}

/// Shape a string with the given face, substituting when allowed.
pub fn shape_text(
    manager: &FontManager,
    font: FontId,
    text: &str,
    options: &ShapeOptions,
) -> Result<ShapedRun> {
    let face: Arc<FontFace> = manager.face(font)?;
    let scale = options.size_mm / 1000.0;

    let mut glyphs = Vec::with_capacity(text.chars().count());
    let mut skipped = Vec::new();

    for (char_idx, ch) in text.chars().enumerate() {
        let cp = ch as u32;

        let (glyph_id, used_font, used_face) = match face.glyph_id(cp) {
            Some(g) => (g, font, face.clone()),
            None if options.allow_fallback => match manager.fallback_for(cp) {
                Some((alt_id, alt_face)) => match alt_face.glyph_id(cp) {
                    Some(g) => (g, alt_id, alt_face),
                    None => {
                        skipped.push(ch);
                        continue;
                    }
                },
                None => {
                    skipped.push(ch);
                    continue;
                }
            },
            None => {
                skipped.push(ch);
                continue;
            }
        };

        let advance_mm = used_face.advance_width(glyph_id) * scale;
        glyphs.push(ShapedGlyph {
            glyph_id,
            advance_mm,
            cluster: char_idx as u32,
            font: used_font,
        });
    }

    let total_advance = glyphs.iter().map(|g| g.advance_mm).sum();

    if !skipped.is_empty() {
        tracing::warn!(
            family = face.family(),
            skipped = skipped.len(),
            "glyphs missing during shaping; text may be incomplete"
        );
    }

    Ok(ShapedRun {
        glyphs,
        text: text.to_string(),
        total_advance,
        skipped_chars: skipped,
    })
}
