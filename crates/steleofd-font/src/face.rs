//! Font face wrapping the shaping backend with a stable metrics and outline API.

use std::sync::Arc;

use steleofd_error::{FontError, Result};
use steleofd_geometry::{Path, PathSegment};

use crate::metrics::{FontMetrics, GlyphOutline};
use crate::registry::FontId;

/// A usable font face.
/// `ttf_parser::Face` wants a `'static` borrow, so bytes live behind an
/// `Arc<Vec<u8>>` and each query builds a temporary `Face` from them.
pub struct FontFace {
    id: FontId,
    family: String,
    data: Arc<Vec<u8>>,
    is_fallback: bool,
    metrics: FontMetrics,
    units_per_em: f64,
}

impl std::fmt::Debug for FontFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontFace")
            .field("id", &self.id)
            .field("family", &self.family)
            .field("bytes", &self.data.len())
            .field("is_fallback", &self.is_fallback)
            .finish()
    }
}

impl FontFace {
    /// Validate the font data and precompute metrics.
    /// Invalid fonts fail at load time rather than surfacing during rendering.
    pub fn new(
        id: FontId,
        family: String,
        data: Arc<Vec<u8>>,
        is_fallback: bool,
    ) -> Result<Self> {
        let (units_per_em, metrics) = {
            let face = ttf_parser::Face::parse(data.as_slice(), 0)
                .map_err(|e| FontError::Parse(format!("font parse failed: {e:?}")))?;

            let upem = face.units_per_em() as f64;
            if upem <= 0.0 {
                return Err(FontError::Parse("font units_per_em is zero".to_string()).into());
            }
            let k = 1000.0 / upem;

            let m = FontMetrics {
                units_per_em: face.units_per_em(),
                ascender: face.ascender() as f64 * k,
                descender: face.descender() as f64 * k,
                line_gap: face.line_gap() as f64 * k,
                cap_height: face.capital_height().unwrap_or(face.ascender()) as f64 * k,
                x_height: face.x_height().unwrap_or(0) as f64 * k,
            };
            (upem, m)
        };

        Ok(Self { id, family, data, is_fallback, metrics, units_per_em })
    }

    /// Font identifier.
    pub fn id(&self) -> FontId {
        self.id
    }

    /// Family name.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Whether this face substitutes for a missing font.
    pub fn is_fallback(&self) -> bool {
        self.is_fallback
    }

    /// Font units per em.
    pub fn units_per_em(&self) -> f64 {
        self.units_per_em
    }

    /// Cached global metrics.
    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    /// Raw font bytes.
    pub fn data(&self) -> &Arc<Vec<u8>> {
        &self.data
    }

    /// Build a temporary backend face for one query.
    fn with_face<T>(&self, f: impl FnOnce(&ttf_parser::Face<'_>) -> T) -> Option<T> {
        let face = ttf_parser::Face::parse(self.data.as_slice(), 0).ok()?;
        Some(f(&face))
    }

    /// Glyph id for a Unicode code point.
    pub fn glyph_id(&self, codepoint: u32) -> Option<u16> {
        let ch = char::from_u32(codepoint)?;
        self.with_face(|f| f.glyph_index(ch).map(|g| g.0)).flatten()
    }

    /// Reverse lookup from glyph id to Unicode, needed for search and copy.
    /// Walks the `cmap` subtables at O(cmap size), so callers should cache.
    pub fn reverse_cmap(&self, glyph: u16) -> Option<u32> {
        self.with_face(|face| {
            let cmap = face.tables().cmap?;
            for subtable in cmap.subtables {
                let mut found: Option<u32> = None;
                subtable.codepoints(|cp| {
                    if found.is_none()
                        && let Some(g) = subtable.glyph_index(cp)
                        && g.0 == glyph
                    {
                        found = Some(cp);
                    }
                });
                if found.is_some() {
                    return found;
                }
            }
            None
        })
        .flatten()
    }

    /// Horizontal advance in 1/1000 em, the OFD convention.
    pub fn advance_width(&self, glyph: u16) -> f64 {
        self.with_face(|f| f.glyph_hor_advance(ttf_parser::GlyphId(glyph)).unwrap_or(0) as f64)
            .unwrap_or(0.0)
    }

    /// Glyph outline normalized to the 1/1000 em space.
    /// Size scaling is left to the render layer's CTM, so one outline can be
    /// reused across font sizes.
    pub fn outline(&self, glyph: u16) -> Option<Path> {
        self.with_face(|face| {
            let mut builder = OutlineBuilder {
                path: Path::new(),
                scale: 1000.0 / face.units_per_em() as f64,
            };
            face.outline_glyph(ttf_parser::GlyphId(glyph), &mut builder)?;
            Some(builder.path)
        })
        .flatten()
    }

    /// Outline result that distinguishes an empty glyph such as a space.
    pub fn glyph_outline(&self, glyph: u16) -> GlyphOutline {
        match self.outline(glyph) {
            Some(p) if !p.is_empty() => GlyphOutline::Path(p),
            _ => GlyphOutline::Empty,
        }
    }

    /// Whether the face covers a code point.
    pub fn has_codepoint(&self, codepoint: u32) -> bool {
        self.glyph_id(codepoint).is_some()
    }
}

/// Adapts the backend outline callbacks into this crate's `Path`.
struct OutlineBuilder {
    path: Path,
    scale: f64,
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path
            .segments
            .push(PathSegment::MoveTo(x as f64 * self.scale, y as f64 * self.scale));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path
            .segments
            .push(PathSegment::LineTo(x as f64 * self.scale, y as f64 * self.scale));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.segments.push(PathSegment::QuadTo {
            c: (x1 as f64 * self.scale, y1 as f64 * self.scale),
            to: (x as f64 * self.scale, y as f64 * self.scale),
        });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.segments.push(PathSegment::CurveTo {
            c1: (x1 as f64 * self.scale, y1 as f64 * self.scale),
            c2: (x2 as f64 * self.scale, y2 as f64 * self.scale),
            to: (x as f64 * self.scale, y as f64 * self.scale),
        });
    }

    fn close(&mut self) {
        self.path.segments.push(PathSegment::Close);
    }
}
