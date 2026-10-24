//! Pages, viewports and render parameters.

use serde::{Deserialize, Serialize};
use sofd_common::{Millimeter, Rect};

/// Page metadata that requires no flattening, for fast thumbnail listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Zero-based page index.
    pub index: usize,
    /// Physical page size.
    pub page_box: Rect,
    /// Rotation in degrees; one of 0, 90, 180, 270.
    pub rotation: u16,
    /// Declared page role, such as body, cover or attachment.
    pub page_type: Option<String>,
}

/// The region of a page to render.
///
/// Tiled rendering and on-demand rendering of very large pages both rely on this.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Viewport {
    /// Visible region in page coordinates, in millimetres.
    pub area: Rect,
    /// Output resolution in dots per inch.
    pub dpi: f64,
    /// Additional zoom on top of `dpi`, for continuous zoom controls.
    pub zoom: f64,
}

impl Viewport {
    /// A viewport covering the whole page.
    pub fn whole_page(page_box: Rect, dpi: f64) -> Self {
        Self {
            area: page_box,
            dpi,
            zoom: 1.0,
        }
    }

    /// Resolution after zoom is applied.
    pub fn effective_dpi(&self) -> f64 {
        self.dpi * self.zoom
    }

    /// Output size in pixels.
    pub fn output_size(&self) -> (u32, u32) {
        self.area.size_px(self.effective_dpi())
    }

    /// Whether the viewport overlaps the page at all.
    pub fn intersects_page(&self, page_box: Rect) -> bool {
        self.area.intersects(page_box)
    }
}

/// Quality preset trading fidelity against speed and memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RenderQuality {
    /// Low resolution, text detail may be sacrificed.
    Thumbnail,
    /// Balanced quality favouring interactivity.
    #[default]
    Preview,
    /// Print resolution with accurate glyphs.
    Print,
    /// Archival resolution with strict glyph matching.
    Archive,
}

impl RenderQuality {
    /// Resolution appropriate for this preset.
    pub fn suggested_dpi(self) -> f64 {
        match self {
            Self::Thumbnail => 36.0,
            Self::Preview => 96.0,
            Self::Print => 300.0,
            Self::Archive => 600.0,
        }
    }

    /// Whether substituted system fonts are acceptable at this preset.
    pub fn allow_font_fallback(self) -> bool {
        matches!(self, Self::Thumbnail | Self::Preview)
    }
}

impl PageInfo {
    /// Page dimensions as a pair of millimetre values.
    pub fn size_mm(&self) -> (Millimeter, Millimeter) {
        (Millimeter(self.page_box.w), Millimeter(self.page_box.h))
    }
}
