//! steleofd-convert
//! Reads one display list and writes another format: SVG, PDF or PNG.
//! A new target format means one [`replay::PageSink`] implementation and nothing else.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod base64;
pub mod png;
pub mod replay;
pub mod svg;

#[cfg(feature = "pdf")]
pub mod pdf;

pub use replay::{GradientShape, PageSink, PathShape, replay};
pub use svg::{SvgOptions, SvgSink, page_to_svg, pages_to_svg};

#[cfg(feature = "pdf")]
pub use pdf::{PdfOptions, PdfSink, document_to_pdf, document_to_pdf_pages, page_to_pdf};

/// How glyph runs become target-format content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextExport {
    /// Outline each glyph into vector paths, which needs a font source.
    #[default]
    Outline,
    /// Emit the run's Unicode text and let the viewer choose a font.
    LiveText,
}

/// What a conversion actually produced.
/// A converter can silently degrade, so callers that care about fidelity read
/// this instead of guessing from the output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversionStats {
    /// Glyphs turned into vector outlines.
    pub outlined_glyphs: usize,
    /// Runs emitted as live text rather than outlines.
    pub live_text_runs: usize,
    /// Runs dropped: no font data and no Unicode text to fall back to.
    pub dropped_runs: usize,
    /// Runs that reached the output with at least one glyph missing from the
    /// face, so the page shows fewer marks than it should. Kept apart from
    /// `dropped_runs`, which counts whole runs that never appear at all.
    pub missing_glyph_runs: usize,
    /// Images embedded in the output.
    pub images: usize,
    /// Images skipped because the target cannot carry their encoding.
    pub images_skipped: usize,
    /// Gradients emitted.
    pub gradients: usize,
}

impl ConversionStats {
    /// Merge another page's counts into this one.
    pub fn absorb(&mut self, other: &Self) {
        self.outlined_glyphs += other.outlined_glyphs;
        self.live_text_runs += other.live_text_runs;
        self.dropped_runs += other.dropped_runs;
        self.missing_glyph_runs += other.missing_glyph_runs;
        self.images += other.images;
        self.images_skipped += other.images_skipped;
        self.gradients += other.gradients;
    }

    /// Whether nothing had to be dropped or skipped.
    pub fn is_lossless(&self) -> bool {
        self.dropped_runs == 0 && self.images_skipped == 0 && self.missing_glyph_runs == 0
    }
}
