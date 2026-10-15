//! The reader's state, and the widgets that draw it.

use std::path::{Path, PathBuf};

use steleofd_core::Document;
use steleofd_i18n::{I18n, MessageId};

use crate::viewer::{self, Layout, PageCache};

/// A failure the reader shows in its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    /// The message shown to the reader, already localized where possible.
    pub message: String,
}

/// Everything the window draws is derived from this.
pub struct App {
    i18n: I18n,
    doc: Option<Document>,
    path: Option<PathBuf>,
    cache: PageCache,
    current: usize,
    dpi: f32,
    layout: Layout,
    error: Option<AppError>,
    info: Option<String>,
    want_open: bool,
    want_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// A reader with no document open.
    pub fn new() -> Self {
        Self {
            i18n: I18n::with_system_locale(),
            doc: None,
            path: None,
            cache: PageCache::new(12),
            current: 0,
            dpi: viewer::DEFAULT_DPI,
            layout: Layout::Single,
            error: None,
            info: None,
            want_open: false,
            want_quit: false,
        }
    }

    /// The open document, if there is one.
    pub fn document(&self) -> Option<&Document> {
        self.doc.as_ref()
    }

    /// The file the open document came from.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The page the reader is showing.
    pub fn current_page(&self) -> usize {
        self.current
    }

    /// How many pages the open document has.
    pub fn page_count(&self) -> usize {
        self.doc.as_ref().map_or(0, Document::page_count)
    }

    /// The resolution pages are rasterized at.
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// How pages are laid out.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// The last failure, if the reader has one to show.
    pub fn error(&self) -> Option<&AppError> {
        self.error.as_ref()
    }

    /// Ask for a file to open; the window turns this into a dialog.
    pub fn request_open(&mut self) {
        self.want_open = true;
    }

    /// Whether the window should open a file dialog this frame.
    pub fn wants_open(&self) -> bool {
        self.want_open
    }

    /// Consume the open request.
    pub fn take_open_request(&mut self) -> bool {
        std::mem::take(&mut self.want_open)
    }

    /// Whether the window should close.
    pub fn wants_quit(&self) -> bool {
        self.want_quit
    }

    /// Ask the window to close.
    pub fn quit(&mut self) {
        self.want_quit = true;
    }

    /// Open a document from disk, replacing whatever was open.
    pub fn open_path(&mut self, path: &Path) -> Result<(), AppError> {
        let bytes = std::fs::read(path).map_err(|err| AppError {
            message: format!("{}: {err}", path.display()),
        })?;
        let doc = Document::open_bytes(&bytes).map_err(|err| AppError {
            message: err.to_string(),
        })?;
        self.cache.clear();
        self.current = 0;
        self.doc = Some(doc);
        self.path = Some(path.to_path_buf());
        self.error = None;
        self.info = Some(self.describe());
        Ok(())
    }

    /// Open a document already in memory, as a dropped file arrives.
    pub fn open_bytes(&mut self, bytes: &[u8], name: Option<&str>) -> Result<(), AppError> {
        let doc = Document::open_bytes(bytes).map_err(|err| AppError {
            message: err.to_string(),
        })?;
        self.cache.clear();
        self.current = 0;
        self.doc = Some(doc);
        self.path = name.map(PathBuf::from);
        self.error = None;
        self.info = Some(self.describe());
        Ok(())
    }

    /// Close the open document and drop its pages.
    pub fn close(&mut self) {
        self.doc = None;
        self.path = None;
        self.cache.clear();
        self.current = 0;
        self.info = None;
    }

    /// The one-line description of the open document, for the status bar.
    pub fn describe(&self) -> String {
        let Some(doc) = self.doc.as_ref() else {
            return String::new();
        };
        let pages = self.i18n.text_with(
            MessageId::ReaderPageOf,
            [
                ("current", "0".into()),
                ("total", doc.page_count().to_string().into()),
            ],
        );
        format!("{} · {pages}", doc.version())
    }

    /// Show a page, clamped to the pages that exist.
    pub fn go_to(&mut self, page: usize) {
        if self.page_count() == 0 {
            self.current = 0;
            return;
        }
        self.current = page.min(self.page_count() - 1);
    }

    /// Step forward one page.
    pub fn next_page(&mut self) {
        self.go_to(self.current + 1);
    }

    /// Step back one page.
    pub fn previous_page(&mut self) {
        self.current = self.current.saturating_sub(1);
    }

    /// Whether there is a page after the current one.
    pub fn has_next(&self) -> bool {
        self.current + 1 < self.page_count()
    }

    /// Whether there is a page before the current one.
    pub fn has_previous(&self) -> bool {
        self.current > 0
    }

    /// Set the resolution, clamped to the range a reader can use.
    pub fn set_dpi(&mut self, dpi: f32) {
        self.dpi = dpi.clamp(viewer::MIN_DPI, viewer::MAX_DPI);
    }

    /// Choose how pages are laid out.
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
    }

    /// Record a failure to show in the status bar.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(AppError {
            message: message.into(),
        });
    }

    /// Clear the last failure.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// The reader's language bundle, for widgets that need a string.
    pub fn i18n(&self) -> &I18n {
        &self.i18n
    }

    /// The rendered page if it is cached at the current resolution.
    pub fn cached(&self, page: usize) -> Option<&viewer::RenderedPage> {
        self.cache.get(page, self.dpi)
    }

    /// Rasterize a page, or return the cached one.
    pub fn page_image(&mut self, page: usize) -> Result<(), AppError> {
        if self.cache.get(page, self.dpi).is_some() {
            return Ok(());
        }
        let Some(doc) = self.doc.as_ref() else {
            return Ok(());
        };
        match viewer::render_page(doc, page, self.dpi) {
            Ok(rendered) => {
                self.cache.insert(page, self.dpi, rendered);
                Ok(())
            }
            Err(err) => {
                self.set_error(err.to_string());
                Err(AppError {
                    message: err.to_string(),
                })
            }
        }
    }

    /// The page label shown under the window.
    pub fn page_label(&self) -> String {
        self.i18n.text_with(
            MessageId::ReaderPageOf,
            [
                ("current", (self.current + 1).to_string().into()),
                ("total", self.page_count().to_string().into()),
            ],
        )
    }

    /// The zoom label shown in the toolbar.
    pub fn zoom_label(&self) -> String {
        self.i18n.text_with(
            MessageId::ReaderZoom,
            [("percent", (self.dpi as i32).to_string().into())],
        )
    }
}
