//! PNG backend.
//! Rasterization belongs to the render crate; this only encodes its output,
//! so the two never disagree about what a page looks like.

use steleofd_display::PageDisplay;
use steleofd_error::{OfdError, RenderError, Result};
use steleofd_font::FontManager;
use steleofd_render::{RasterOutput, RenderOptions, render_page};

/// Rasterize one page and encode it as PNG bytes.
pub fn page_to_png(
    page: &PageDisplay,
    fonts: &FontManager,
    options: &RenderOptions,
) -> Result<Vec<u8>> {
    let raster = render_page(page, fonts, options)?;
    encode_png(&raster, page.index)
}

/// Encode a finished rasterization as PNG bytes.
pub fn encode_png(raster: &RasterOutput, page: usize) -> Result<Vec<u8>> {
    let image = image::RgbaImage::from_raw(raster.width, raster.height, raster.rgba_bytes())
        .ok_or_else(|| {
            OfdError::Render(RenderError::ImageEncode {
                page,
                reason: format!(
                    "{}x{} does not match the raster byte count",
                    raster.width, raster.height
                ),
            })
        })?;
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| {
            OfdError::Render(RenderError::ImageEncode {
                page,
                reason: e.to_string(),
            })
        })?;
    Ok(out.into_inner())
}

