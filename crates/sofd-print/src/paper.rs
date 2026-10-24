//! Paper, orientation, margins and other physical print parameters.

use sofd_common::Rect;

/// A physical paper size in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PaperSize {
    /// Follow each page's own box, so mixed page sizes survive printing.
    Document,
    /// ISO A3, 297 by 420 millimetres.
    A3,
    /// ISO A4, 210 by 297 millimetres.
    #[default]
    A4,
    /// ISO A5, 148 by 210 millimetres.
    A5,
    /// ISO B5, 176 by 250 millimetres.
    B5,
    /// US Letter, 215.9 by 279.4 millimetres.
    Letter,
    /// US Legal, 215.9 by 355.6 millimetres.
    Legal,
    /// An explicit size in millimetres.
    Custom {
        /// Width in millimetres.
        w: f64,
        /// Height in millimetres.
        h: f64,
    },
}

impl PaperSize {
    /// Resolve to millimetres, using the page box for [`PaperSize::Document`].
    pub fn size_mm(self, page_box: Rect) -> (f64, f64) {
        match self {
            PaperSize::Document => (page_box.w, page_box.h),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::B5 => (176.0, 250.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Custom { w, h } => (w, h),
        }
    }

    /// Whether the size is fixed rather than following the document.
    pub fn is_fixed(self) -> bool {
        !matches!(self, PaperSize::Document)
    }
}

/// How the sheet is fed through the printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Long edge horizontal.
    #[default]
    Portrait,
    /// Long edge vertical.
    Landscape,
}

impl Orientation {
    /// Apply the orientation to a size, swapping the axes when needed.
    pub fn orient(self, w: f64, h: f64) -> (f64, f64) {
        match self {
            Orientation::Portrait if h < w => (h, w),
            Orientation::Landscape if w < h => (h, w),
            _ => (w, h),
        }
    }
}

/// Unprintable edges of the paper, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    /// Top edge.
    pub top: f64,
    /// Right edge.
    pub right: f64,
    /// Bottom edge.
    pub bottom: f64,
    /// Left edge.
    pub left: f64,
}

impl Margins {
    /// The same margin on every edge.
    pub fn uniform(m: f64) -> Self {
        Self {
            top: m,
            right: m,
            bottom: m,
            left: m,
        }
    }

    /// No margins at all.
    pub fn none() -> Self {
        Self::uniform(0.0)
    }

    /// The printable rectangle inside a sheet of the given size.
    pub fn printable(self, w: f64, h: f64) -> Rect {
        let (x, y) = (self.left, self.top);
        let width = (w - self.left - self.right).max(0.0);
        let height = (h - self.top - self.bottom).max(0.0);
        Rect::new(x, y, width, height)
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self::uniform(5.0)
    }
}

/// Whether the printer prints on both sides of the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Duplex {
    /// One page per sheet.
    #[default]
    None,
    /// Flip along the long edge, like a book.
    LongEdge,
    /// Flip along the short edge, like a notepad.
    ShortEdge,
}

impl Duplex {
    /// Whether two pages share one sheet.
    pub fn two_sided(self) -> bool {
        !matches!(self, Duplex::None)
    }
}

/// Colour or monochrome output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Full colour.
    #[default]
    Color,
    /// Greyscale, for mono printers and cheaper output.
    Mono,
}

impl ColorMode {
    /// Whether the output should be reduced to one channel.
    pub fn is_mono(self) -> bool {
        matches!(self, ColorMode::Mono)
    }
}
