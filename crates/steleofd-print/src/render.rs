//! Rasterizing a planned job at print resolution.

use steleofd_common::Rect;
use steleofd_display::{DrawOp, PageDisplay};
use steleofd_error::Result;
use steleofd_font::FontManager;
use steleofd_geometry::Affine;
use steleofd_render::{RasterOutput, RenderOptions, render_page};

use crate::job::{PlannedPage, PrintJob};

/// One rasterized sheet face, straight RGBA8, top row first.
#[derive(Debug, Clone)]
pub struct RasterPage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Resolution the raster was produced at.
    pub dpi: f64,
    /// Straight RGBA8 bytes, four per pixel.
    pub rgba: Vec<u8>,
}

/// Rasterize one planned page onto its sheet.
/// The placement wraps the page ops in a save/transform pair, so the display
/// list itself stays object-local and the sheet carries the page box.
pub fn render_planned(
    page: &PageDisplay,
    fonts: &FontManager,
    planned: &PlannedPage,
    job: &PrintJob,
) -> Result<RasterPage> {
    let sheet = PageDisplay {
        index: planned.page_index,
        page_box: planned.paper,
        ops: wrapped(planned.placement, &page.ops),
        content_bbox: Rect::EMPTY,
    };
    let options = RenderOptions {
        dpi: job.dpi,
        background: job.background,
        ..RenderOptions::default()
    };
    let raster = render_page(&sheet, fonts, &options)?;
    Ok(finish(raster, job))
}

/// Rasterize every page of a job, in print order.
pub fn render_job(
    list: &steleofd_display::DisplayList,
    fonts: &FontManager,
    job: &PrintJob,
) -> Result<Vec<RasterPage>> {
    job.pages
        .iter()
        .map(|planned| {
            let page = list.pages.get(planned.page_index).ok_or_else(|| {
                steleofd_error::OfdError::Print(steleofd_error::PrintError::PageOutOfRange {
                    page: planned.page_index + 1,
                    count: list.pages.len(),
                })
            })?;
            render_planned(page, fonts, planned, job)
        })
        .collect()
}

/// Wrap page ops so they land on the sheet through the placement.
fn wrapped(placement: Affine, ops: &[DrawOp]) -> Vec<DrawOp> {
    let mut out = Vec::with_capacity(ops.len() + 2);
    out.push(DrawOp::Save);
    out.push(DrawOp::Transform(placement));
    out.extend_from_slice(ops);
    out.push(DrawOp::Restore);
    out
}

/// Take the raster bytes and reduce them when the job is monochrome.
fn finish(raster: RasterOutput, job: &PrintJob) -> RasterPage {
    let mut rgba = raster.rgba_bytes();
    if job.color.is_mono() {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            let luma = (0.299 * pixel[0] as f64 + 0.587 * pixel[1] as f64 + 0.114 * pixel[2] as f64)
                .round() as u8;
            pixel[0] = luma;
            pixel[1] = luma;
            pixel[2] = luma;
        }
    }
    RasterPage {
        width: raster.width,
        height: raster.height,
        dpi: job.dpi,
        rgba,
    }
}
