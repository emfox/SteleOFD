//! Print job planning: which pages, on which sheet, placed where.

use steleofd_common::Rect;
use steleofd_display::DisplayList;
use steleofd_display::ops::Color;
use steleofd_error::{OfdError, PrintError, Result};
use steleofd_geometry::Affine;

use crate::paper::{ColorMode, Duplex, Orientation};
use crate::settings::{PageOrder, PrintSettings};

/// Which face of a sheet a page lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSide {
    /// The front face.
    Front,
    /// The back face, only used with two-sided printing.
    Back,
}

/// One page resolved onto a sheet, ready to rasterize.
#[derive(Debug, Clone)]
pub struct PlannedPage {
    /// Zero-based index into the display list's pages.
    pub page_index: usize,
    /// Zero-based copy this page belongs to.
    pub copy: usize,
    /// Zero-based sheet number within the job.
    pub sheet: usize,
    /// The face of the sheet the page prints on.
    pub side: PageSide,
    /// The sheet rectangle in millimetres, origin at the top left.
    pub paper: Rect,
    /// Content to sheet transform in millimetres.
    pub placement: Affine,
}

/// A whole print job: an ordered list of placed pages plus shared parameters.
#[derive(Debug, Clone)]
pub struct PrintJob {
    /// Pages in the order they travel through the printer.
    pub pages: Vec<PlannedPage>,
    /// Whether pages share sheets.
    pub duplex: Duplex,
    /// Colour or monochrome output.
    pub color: ColorMode,
    /// Output resolution in dots per inch.
    pub dpi: f64,
    /// Colour painted behind each page.
    pub background: Option<Color>,
}

impl PrintJob {
    /// How many sheets the job consumes.
    pub fn sheet_count(&self) -> usize {
        if self.duplex.two_sided() {
            self.pages.len().div_ceil(2)
        } else {
            self.pages.len()
        }
    }

    /// Whether the job has nothing to print.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

/// Turn a display list and its settings into a printable job.
/// Positions in the selection index the materialized pages.
pub fn plan(list: &DisplayList, settings: &PrintSettings) -> Result<PrintJob> {
    settings.validate()?;
    let mut sequence = settings.selection.resolve(list.pages.len())?;
    if settings.order == PageOrder::Reverse {
        sequence.reverse();
    }

    let mut ordered: Vec<(usize, usize)> = Vec::with_capacity(sequence.len() * settings.copies);
    if settings.collate {
        for copy in 0..settings.copies {
            ordered.extend(sequence.iter().map(|index| (*index, copy)));
        }
    } else {
        for index in &sequence {
            for copy in 0..settings.copies {
                ordered.push((*index, copy));
            }
        }
    }

    let mut pages = Vec::with_capacity(ordered.len());
    for (n, (page_index, copy)) in ordered.into_iter().enumerate() {
        let page = list.pages.get(page_index).ok_or_else(|| {
            OfdError::Print(PrintError::PageOutOfRange {
                page: page_index + 1,
                count: list.pages.len(),
            })
        })?;
        let paper = settings.sheet_for(page.page_box)?;
        let placement = placement_for(page.page_box, paper, settings)?;
        let (sheet, side) = if settings.duplex.two_sided() {
            (
                n / 2,
                if n % 2 == 0 {
                    PageSide::Front
                } else {
                    PageSide::Back
                },
            )
        } else {
            (n, PageSide::Front)
        };
        pages.push(PlannedPage {
            page_index,
            copy,
            sheet,
            side,
            paper,
            placement,
        });
    }

    Ok(PrintJob {
        pages,
        duplex: settings.duplex,
        color: settings.color,
        dpi: settings.dpi,
        background: settings.background,
    })
}

/// Place one page's content box onto its sheet, centred in the target area.
fn placement_for(content: Rect, sheet: Rect, settings: &PrintSettings) -> Result<Affine> {
    let (cw, ch) = (content.w, content.h);
    let rotate = needs_rotation(cw, ch, settings.orientation);
    let (rw, rh) = if rotate { (ch, cw) } else { (cw, ch) };

    let target = if settings.scale.ignores_margins() {
        sheet
    } else {
        settings.margins.printable(sheet.w, sheet.h)
    };
    let scale = settings.scale.factor((rw, rh), (target.w, target.h));
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OfdError::Print(PrintError::InvalidScale { scale }));
    }

    // Rotation about the content centre keeps that centre fixed, so scaling
    // about the origin only moves it to `scale * centre`. The leftmost factor
    // acts first, keeping the translation in sheet space.
    let (cx, cy) = (content.x + cw / 2.0, content.y + ch / 2.0);
    let (tx, ty) = (
        target.x + target.w / 2.0 - scale * cx,
        target.y + target.h / 2.0 - scale * cy,
    );
    let mut placement = Affine::scale(scale, scale).then(Affine::translate(tx, ty));
    if rotate {
        let spin = Affine::translate(-cx, -cy)
            .then(Affine::rotate_deg(90.0))
            .then(Affine::translate(cx, cy));
        placement = spin.then(placement);
    }
    Ok(placement)
}

/// Whether the orientation forces a quarter turn of the content.
fn needs_rotation(w: f64, h: f64, orientation: Orientation) -> bool {
    match orientation {
        Orientation::Portrait => w > h,
        Orientation::Landscape => h > w,
    }
}

