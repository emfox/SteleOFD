//! Page rasterization and the small cache the viewer keeps.

use std::collections::HashMap;

use steleofd_core::Document;
use steleofd_render::RenderOptions;

/// The resolution bounds the reader will rasterize at.
pub const MIN_DPI: f32 = 24.0;
/// The resolution bounds the reader will rasterize at.
pub const MAX_DPI: f32 = 300.0;
/// The resolution a freshly opened document is shown at.
pub const DEFAULT_DPI: f32 = 96.0;

/// A rasterized page, decoded and ready to paint.
pub struct RenderedPage {
    /// The decoded image, in `egui` colour order.
    pub image: egui::ColorImage,
    /// The page's physical size in millimetres.
    pub size_mm: (f32, f32),
}

/// How pages are laid out in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// One page at a time.
    #[default]
    Single,
    /// Two pages side by side, as a spread.
    Facing,
    /// Every page in one scrolling column.
    Continuous,
}

impl Layout {
    /// The label shown in the menu, in the reader's own words.
    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "reader-layout-single",
            Self::Facing => "reader-layout-double",
            Self::Continuous => "reader-layout-continuous",
        }
    }
}

/// Keeps the pages a reader has already rasterized, because rendering is the
/// expensive part of reading and a reader revisits the same page constantly.
/// The key carries the dpi: one page at two zooms is two entries.
pub struct PageCache {
    entries: HashMap<(usize, u32), RenderedPage>,
    order: Vec<(usize, u32)>,
    limit: usize,
}

impl PageCache {
    /// A cache holding at most `limit` rendered pages.
    pub fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// The cached page for this index and dpi, if it was rendered already.
    pub fn get(&self, page: usize, dpi: f32) -> Option<&RenderedPage> {
        self.entries.get(&(page, dpi.to_bits()))
    }

    /// Store a rendered page, evicting the oldest entry when full.
    pub fn insert(&mut self, page: usize, dpi: f32, rendered: RenderedPage) {
        let key = (page, dpi.to_bits());
        if self.entries.insert(key, rendered).is_none() {
            self.order.push(key);
        }
        while self.order.len() > self.limit {
            let oldest = self.order.remove(0);
            self.entries.remove(&oldest);
        }
    }

    /// Drop every entry; used when a different document is opened.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    /// How many pages are currently held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// How close to white every pixel has to be before a page counts as blank.
const BLANK_THRESHOLD: u8 = 8;

/// A file the platform handed to the window, as a drop or a paste.
///
/// Desktops differ: some deliver a path, others only the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DroppedFile {
    /// A path on disk the reader may read.
    Path(std::path::PathBuf),
    /// The file contents, with a name for display.
    Bytes {
        /// The file name, if the platform reported one.
        name: String,
        /// The whole file.
        bytes: Vec<u8>,
    },
}

impl DroppedFile {
    /// The file name, if there is one to show.
    pub fn name(&self) -> Option<String> {
        match self {
            Self::Path(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()),
            Self::Bytes { name, .. } => Some(name.clone()),
        }
    }
}

/// Whether a rendered page carries no visible ink.
///
/// It reads the raster, because a page may hold nothing but a background.
pub fn looks_blank(image: &egui::ColorImage) -> bool {
    let pixels = image.pixels.as_slice();
    !pixels.is_empty()
        && pixels.iter().all(|p| {
            p.r() >= 255 - BLANK_THRESHOLD
                && p.g() >= 255 - BLANK_THRESHOLD
                && p.b() >= 255 - BLANK_THRESHOLD
        })
}

/// Rasterize one page at the given resolution.
pub fn render_page(
    doc: &Document,
    page: usize,
    dpi: f32,
) -> Result<RenderedPage, steleofd_error::OfdError> {
    let options = RenderOptions {
        dpi: f64::from(dpi.clamp(MIN_DPI, MAX_DPI)),
        ..RenderOptions::default()
    };
    let png = doc.to_png(page, &options)?;
    let decoded =
        image::load_from_memory_with_format(&png, image::ImageFormat::Png).map_err(|reason| {
            steleofd_error::OfdError::Render(steleofd_error::RenderError::ImageEncode {
                page,
                reason: reason.to_string(),
            })
        })?;
    let size_mm = doc
        .page(page)
        .map(|p| (p.page_box.w as f32, p.page_box.h as f32))
        .unwrap_or((210.0, 297.0));
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [decoded.width() as usize, decoded.height() as usize],
        decoded.to_rgba8().as_raw(),
    );
    Ok(RenderedPage { image, size_mm })
}
