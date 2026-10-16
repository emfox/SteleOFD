//! Font face wrapping the shaping backend with a stable metrics and outline API.

use std::sync::Arc;

use steleofd_error::{FontError, Result};
use steleofd_geometry::{Path, PathSegment};

use crate::cff;
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
    /// Whether the face is keyed by CID, which decides how the glyph codes
    /// of a `CGTransform` run are read.
    cid_keyed: bool,
    /// CID to glyph id, sorted by CID and empty for every other face.
    cid_to_gid: Vec<(u16, u16)>,
}

impl std::fmt::Debug for FontFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontFace")
            .field("id", &self.id)
            .field("family", &self.family)
            .field("bytes", &self.data.len())
            .field("is_fallback", &self.is_fallback)
            .field("cid_keyed", &self.cid_keyed)
            .finish()
    }
}

impl FontFace {
    /// Validate the font data and precompute metrics, so a bad font fails at
    /// load time. A bare CFF is wrapped in an sfnt directory before the backend reads it.
    pub fn new(id: FontId, family: String, data: Arc<Vec<u8>>, is_fallback: bool) -> Result<Self> {
        let (data, cid_keyed, cid_to_gid) = if cff::is_bare_cff(&data) {
            let cids = cff::cid_charset(&data);
            let wrapped = cff::wrap_bare_cff(&data).ok_or_else(|| {
                FontError::Parse("a bare CFF font could not be wrapped".to_string())
            })?;
            (
                Arc::new(wrapped),
                cids.is_some(),
                cid_to_gid(cids.as_deref()),
            )
        } else {
            (data, false, Vec::new())
        };
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

        Ok(Self {
            id,
            family,
            data,
            is_fallback,
            metrics,
            units_per_em,
            cid_keyed,
            cid_to_gid,
        })
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

    /// Raw font bytes. A bare CFF arrives as one and is stored as the sfnt
    /// wrapper the backend reads, since that is what every query runs on.
    pub fn data(&self) -> &Arc<Vec<u8>> {
        &self.data
    }

    /// Glyph id for a CID, the index space a CID-keyed font's `CGTransform`
    /// names. A face that is not keyed by CID has no map and answers `None`.
    pub fn glyph_for_cid(&self, cid: u16) -> Option<u16> {
        let at = self
            .cid_to_gid
            .binary_search_by(|(c, _)| c.cmp(&cid))
            .ok()?;
        Some(self.cid_to_gid[at].1)
    }

    /// Outline for one entry of a display-list glyph run: a `CGTransform` run
    /// holds glyph indices, any other run Unicode code points.
    pub fn outline_for_run_code(&self, code: u16, glyph_index: bool) -> Option<Path> {
        if !glyph_index {
            return self.outline_for_codepoint(u32::from(code));
        }
        if self.cid_keyed {
            return self.glyph_for_cid(code).and_then(|gid| self.outline(gid));
        }
        self.outline(code)
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

    /// Outline for a Unicode code point, mapped through the cmap first.
    /// Display lists carry code points in place of glyph ids until shaping
    /// lands, so consumers must not treat the raw value as a glyph index.
    pub fn outline_for_codepoint(&self, codepoint: u32) -> Option<Path> {
        let glyph = self.glyph_id(codepoint)?;
        self.outline(glyph)
    }

    /// Whether the face covers a code point.
    pub fn has_codepoint(&self, codepoint: u32) -> bool {
        self.glyph_id(codepoint).is_some()
    }
}

/// Turn a charset read off a CID-keyed CFF into the reverse map a glyph
/// lookup wants: CID to glyph id, sorted by CID, with `.notdef` left out.
/// A CID the charset names twice keeps its first glyph id.
fn cid_to_gid(cids: Option<&[u16]>) -> Vec<(u16, u16)> {
    let mut out: Vec<(u16, u16)> = cids
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .skip(1)
        .map(|(gid, cid)| (*cid, gid as u16))
        .collect();
    out.sort_unstable_by_key(|(cid, _)| *cid);
    out.dedup_by_key(|(cid, _)| *cid);
    out
}

/// Adapts the backend outline callbacks into this crate's `Path`.
struct OutlineBuilder {
    path: Path,
    scale: f64,
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.segments.push(PathSegment::MoveTo(
            x as f64 * self.scale,
            y as f64 * self.scale,
        ));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.segments.push(PathSegment::LineTo(
            x as f64 * self.scale,
            y as f64 * self.scale,
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cff::tests::synthetic_cff;

    fn face_of(data: &[u8]) -> FontFace {
        FontFace::new(
            FontId(1),
            "TestCFF".to_string(),
            Arc::new(data.to_vec()),
            false,
        )
        .expect("the face loads")
    }

    #[test]
    fn a_bare_cff_face_loads_and_keeps_its_cid_map() {
        let face = face_of(&synthetic_cff());
        assert_eq!(face.units_per_em(), 1000.0);
        assert_eq!(face.glyph_for_cid(963), Some(1));
        assert_eq!(face.glyph_for_cid(1217), Some(2));
        assert_eq!(face.glyph_for_cid(0), None, ".notdef has no CID");
        assert_eq!(face.glyph_for_cid(1), None, "a glyph id is not a CID");
    }

    #[test]
    fn a_cid_run_resolves_through_the_charset() {
        let face = face_of(&synthetic_cff());
        let outline = face
            .outline_for_run_code(963, true)
            .expect("the CID names a glyph");
        assert_eq!(outline.segments.len(), 6, "a square is four lines");
        // The other direction is a code point, and a CFF has no cmap, so it
        // finds nothing rather than the wrong glyph.
        assert!(face.outline_for_run_code(963, false).is_none());
    }

    #[test]
    fn an_sfnt_face_answers_cid_lookups_with_nothing() {
        let orbitron = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
        ))
        .expect("font fixture");
        let face = face_of(&orbitron);
        assert!(face.glyph_for_cid(963).is_none());
        // Without a CGTransform the codes stay code points, which is the
        // path every document without one already takes.
        assert!(face.outline_for_run_code('A' as u16, false).is_some());
    }
}
