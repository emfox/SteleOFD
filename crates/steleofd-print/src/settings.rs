//! What to print and how it should be laid out.

use steleofd_common::Rect;
use steleofd_display::ops::Color;
use steleofd_error::{OfdError, PrintError, Result};

use crate::paper::{ColorMode, Duplex, Margins, Orientation, PaperSize};

/// Which pages of the document take part in the job.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PageSelection {
    /// Every page.
    #[default]
    All,
    /// Explicit one-based page numbers, in the order given.
    List(Vec<usize>),
}

impl PageSelection {
    /// Parse a selection such as `all`, `3`, `2-5` or `1,3,7-9`.
    pub fn parse(spec: &str) -> Result<Self> {
        let trimmed = spec.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
            return Ok(PageSelection::All);
        }
        let mut pages = Vec::new();
        for part in trimmed.split(',') {
            let part = part.trim();
            let (start, end) = match part.split_once('-') {
                Some((a, b)) => {
                    let start = parse_page(a, trimmed)?;
                    let end = parse_page(b, trimmed)?;
                    if start > end {
                        return Err(OfdError::Print(PrintError::InvalidRange { start, end }));
                    }
                    (start, end)
                }
                None => (parse_page(part, trimmed)?, parse_page(part, trimmed)?),
            };
            pages.extend(start..=end);
        }
        Ok(PageSelection::List(pages))
    }

    /// Resolve against a page count into zero-based indices, rejecting gaps.
    pub fn resolve(&self, page_count: usize) -> Result<Vec<usize>> {
        let one_based: Vec<usize> = match self {
            PageSelection::All => (1..=page_count).collect(),
            PageSelection::List(pages) => {
                let mut sorted = pages.clone();
                sorted.sort_unstable();
                sorted.dedup();
                sorted
            }
        };
        for page in &one_based {
            if *page == 0 || *page > page_count {
                return Err(OfdError::Print(PrintError::PageOutOfRange {
                    page: *page,
                    count: page_count,
                }));
            }
        }
        Ok(one_based.into_iter().map(|p| p - 1).collect())
    }
}

fn parse_page(text: &str, spec: &str) -> Result<usize> {
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| {
            OfdError::Print(PrintError::InvalidSelection {
                spec: spec.to_string(),
            })
        })
}

/// How page content is sized onto the paper.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScaleMode {
    /// One to one: one millimetre of page becomes one millimetre of paper.
    #[default]
    None,
    /// Shrink or grow until the whole page fits the sheet, margins ignored.
    FitPaper,
    /// Shrink or grow until the whole page fits inside the margins.
    FitPrintable,
    /// A fixed percentage of the original size.
    Custom(f64),
}

impl ScaleMode {
    /// The scale factor for a content size inside a target size.
    pub fn factor(self, content: (f64, f64), target: (f64, f64)) -> f64 {
        match self {
            ScaleMode::None => 1.0,
            ScaleMode::Custom(pct) => pct / 100.0,
            ScaleMode::FitPaper | ScaleMode::FitPrintable => {
                let (cw, ch) = content;
                let (tw, th) = target;
                if cw <= 0.0 || ch <= 0.0 || tw <= 0.0 || th <= 0.0 {
                    return 1.0;
                }
                (tw / cw).min(th / ch)
            }
        }
    }

    /// Whether the target area ignores the margins.
    pub fn ignores_margins(self) -> bool {
        matches!(self, ScaleMode::FitPaper)
    }
}

/// The order pages travel through the printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageOrder {
    /// First page first, the usual case for face-up output.
    #[default]
    Normal,
    /// Last page first, for trays that stack face down.
    Reverse,
}

/// Everything that describes one print job.
#[derive(Debug, Clone)]
pub struct PrintSettings {
    /// The sheet to print on.
    pub paper: PaperSize,
    /// Portrait or landscape feeding.
    pub orientation: Orientation,
    /// Unprintable edges of the sheet.
    pub margins: Margins,
    /// Which pages to print.
    pub selection: PageSelection,
    /// How content is sized onto the sheet.
    pub scale: ScaleMode,
    /// How many times the whole selection repeats.
    pub copies: usize,
    /// Whether copies stay in page order instead of grouping per page.
    pub collate: bool,
    /// The order pages travel through the printer.
    pub order: PageOrder,
    /// One or two sided output.
    pub duplex: Duplex,
    /// Colour or monochrome.
    pub color: ColorMode,
    /// Output resolution in dots per inch.
    pub dpi: f64,
    /// Colour painted behind the page; paper is white by default.
    pub background: Option<Color>,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper: PaperSize::default(),
            orientation: Orientation::default(),
            margins: Margins::default(),
            selection: PageSelection::default(),
            scale: ScaleMode::default(),
            copies: 1,
            collate: true,
            order: PageOrder::default(),
            duplex: Duplex::default(),
            color: ColorMode::default(),
            dpi: 300.0,
            background: Some(Color::WHITE),
        }
    }
}

impl PrintSettings {
    /// Check the settings that can be rejected before any page is looked at.
    pub fn validate(&self) -> Result<()> {
        if self.copies == 0 {
            return Err(OfdError::Print(PrintError::InvalidCopies { copies: 0 }));
        }
        if let ScaleMode::Custom(pct) = self.scale
            && (!pct.is_finite() || pct <= 0.0)
        {
            return Err(OfdError::Print(PrintError::InvalidScale { scale: pct }));
        }
        if !self.dpi.is_finite() || self.dpi <= 0.0 {
            return Err(OfdError::Print(PrintError::InvalidScale {
                scale: self.dpi,
            }));
        }
        Ok(())
    }

    /// The paper rectangle a page of the given box prints on, margins applied.
    pub fn sheet_for(&self, page_box: Rect) -> Result<Rect> {
        let (w, h) = self.paper.size_mm(page_box);
        let (w, h) = self.orientation.orient(w, h);
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err(OfdError::Print(PrintError::InvalidPaper { w, h }));
        }
        Ok(Rect::new(0.0, 0.0, w, h))
    }
}
