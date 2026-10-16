//! The reader's state, and the widgets that draw it.

use std::path::{Path, PathBuf};

use steleofd_core::{Document, SearchOptions, VerifyStatus};
use steleofd_i18n::{I18n, MessageId};

use crate::export_worker::{ExportFormat, ExportQueue, ExportRequest};
use crate::render_worker::RenderQueue;
use crate::shortcuts::{self, ZOOM_STEP};
use crate::viewer::{self, DroppedFile, Layout, PageCache};

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
    /// The bytes the document came from, so the render worker can open its own
    /// handle rather than sharing this one across threads.
    bytes: Option<Vec<u8>>,
    queue: Option<RenderQueue>,
    path: Option<PathBuf>,
    cache: PageCache,
    current: usize,
    dpi: f32,
    fit: Option<Fit>,
    layout: Layout,
    /// The text being looked for, empty when nothing is.
    query: String,
    /// Every hit for `query`, in page order.
    hits: Vec<Hit>,
    /// Which hit the reader is on, if any.
    hit_index: Option<usize>,
    /// Whether the search panel is showing.
    search_open: bool,
    /// The verification outcome for each signature, filled on first ask.
    signatures: Option<Vec<SignatureReport>>,
    /// Whether the signature panel is showing.
    signatures_open: bool,
    /// The worker that converts and writes exported pages.
    exports: Option<ExportQueue>,
    /// The stamp the newest export was asked under.
    export_generation: u64,
    /// What the newest finished export wrote, for the status bar to report.
    export_report: Option<ExportReport>,
    /// The format the reader asked to export, waiting for a destination.
    want_export: Option<ExportFormat>,
    error: Option<AppError>,
    info: Option<String>,
    want_open: bool,
    want_quit: bool,
}

/// What one finished export produced, in the reader's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    /// What was produced.
    pub format: ExportFormat,
    /// How many pages went in.
    pub pages: usize,
    /// Where the files landed.
    pub written: Vec<PathBuf>,
}

/// What the reader needs to know about one signature. The verdict comes from
/// the engine; who signed and when come from the descriptor, read alongside
/// it, so the panel never reaches into the engine itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureReport {
    /// The signature's own identifier, as written in the document.
    pub id: Option<String>,
    /// How the signature stands, which is not a yes-or-no question.
    pub verdict: Verdict,
    /// The name of the signing provider, when the document records one.
    pub signer: Option<String>,
    /// When it was signed, as written.
    pub signed_at: Option<String>,
    /// The algorithm named in the descriptor.
    pub algorithm: Option<String>,
}

/// How one signature stands. Three states rather than two: a proof that fails
/// and a proof that cannot be checked call for different reactions, and
/// flattening them calls a document tampered with when it is only unconfirmable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The signature matches what it covers.
    Valid,
    /// The content no longer matches the signed digest.
    Tampered,
    /// The signature could not be checked at all, with the reason why.
    Unverified,
}

impl Verdict {
    /// Translate the engine's two-state answer into the reader's three.
    fn from_status(status: VerifyStatus) -> Self {
        match status {
            VerifyStatus::Valid => Self::Valid,
            VerifyStatus::Tampered => Self::Tampered,
        }
    }
}

/// A resolution the reader keeps chasing as the window or page changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Fill the available width with the page.
    Width,
    /// Show the whole page inside the available area.
    Page,
}

/// One search result, reduced to what the reader needs to show and jump to.
/// The engine's hit also carries run and op indices and byte offsets, which
/// matter to whoever edits the text rather than to a highlight.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Page the match is on.
    pub page: usize,
    /// The text around the match, for the result list.
    pub snippet: String,
    /// Where the match is on the page, in page space.
    pub rect: steleofd_common::Rect,
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
            bytes: None,
            queue: None,
            path: None,
            cache: PageCache::new(12),
            current: 0,
            dpi: viewer::DEFAULT_DPI,
            fit: None,
            layout: Layout::Single,
            query: String::new(),
            hits: Vec::new(),
            hit_index: None,
            search_open: false,
            signatures: None,
            signatures_open: false,
            exports: None,
            export_generation: 0,
            export_report: None,
            want_export: None,
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

    /// Whether a document is open.
    pub fn is_open(&self) -> bool {
        self.doc.is_some()
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
        self.install(&bytes, Some(path.to_path_buf()))?;
        self.info = Some(self.describe());
        Ok(())
    }

    /// Open a document already in memory, as a dropped file arrives.
    pub fn open_bytes(&mut self, bytes: &[u8], name: Option<&str>) -> Result<(), AppError> {
        self.install(bytes, name.map(PathBuf::from))?;
        self.info = Some(self.describe());
        Ok(())
    }

    /// Replace the open document, keeping the bytes for the render worker.
    /// A failed open changes nothing, so a good document is never lost.
    fn install(&mut self, bytes: &[u8], path: Option<PathBuf>) -> Result<(), AppError> {
        let doc = Document::open_bytes(bytes).map_err(|err| AppError {
            message: err.to_string(),
        })?;
        self.cache.clear();
        self.current = 0;
        self.fit = None;
        self.doc = Some(doc);
        self.bytes = Some(bytes.to_vec());
        self.queue = Some(RenderQueue::spawn(bytes.to_vec()));
        self.path = path;
        self.error = None;
        Ok(())
    }

    /// Close the open document and drop its pages.
    pub fn close(&mut self) {
        self.doc = None;
        self.bytes = None;
        self.queue = None;
        self.path = None;
        self.cache.clear();
        self.current = 0;
        self.fit = None;
        self.info = None;
        self.query.clear();
        self.hits.clear();
        self.hit_index = None;
        self.search_open = false;
        self.signatures = None;
        self.signatures_open = false;
        self.exports = None;
        self.export_report = None;
        self.want_export = None;
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

    /// The description captured when the document was opened, if one is open.
    pub fn info(&self) -> Option<&str> {
        self.info.as_deref()
    }

    /// The file name of the open document, for the window title.
    pub fn file_name(&self) -> Option<String> {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
    }

    /// Show a page, clamped to the pages that exist.
    pub fn go_to(&mut self, page: usize) {
        if self.page_count() == 0 {
            self.current = 0;
            return;
        }
        self.current = page.min(self.page_count() - 1);
    }

    /// Step forward one screen, by the layout's stride.
    pub fn next_page(&mut self) {
        self.go_to(self.current + self.layout.stride());
    }

    /// Step back one screen, by the layout's stride.
    pub fn previous_page(&mut self) {
        self.current = self.current.saturating_sub(self.layout.stride());
    }

    /// The pages the canvas should show, given the layout. This is zone Z3's
    /// input: the window paints these and forms no opinion about which they are.
    pub fn visible(&self) -> Vec<usize> {
        viewer::visible_pages(self.layout, self.current, self.page_count())
    }

    /// Whether there is a screen after the current one.
    pub fn has_next(&self) -> bool {
        self.current + self.layout.stride() < self.page_count()
    }

    /// Whether there is a screen before the current one.
    pub fn has_previous(&self) -> bool {
        self.current >= self.layout.stride()
    }

    /// Set the resolution, clamped to the range a reader can use.
    pub fn set_dpi(&mut self, dpi: f32) {
        let dpi = dpi.clamp(viewer::MIN_DPI, viewer::MAX_DPI);
        if dpi != self.dpi {
            self.dpi = dpi;
            if let Some(queue) = self.queue.as_mut() {
                queue.rezoom();
            }
        }
        self.fit = None;
    }

    /// Keep the page fitted to the space it is drawn in.
    pub fn set_fit(&mut self, fit: Fit) {
        self.fit = Some(fit);
    }

    /// The fit the reader is holding, if any.
    pub fn fit(&self) -> Option<Fit> {
        self.fit
    }

    /// Recompute the resolution for the fit in force, given the space the
    /// canvas has. Called by the window each frame, so a resize re-fits.
    pub fn apply_fit(&mut self, available: (f32, f32)) {
        let Some(fit) = self.fit else {
            return;
        };
        let Some(size_mm) = self.page_size_mm(self.current) else {
            return;
        };
        let dpi = viewer::fit_dpi(fit, size_mm, available);
        if dpi != self.dpi {
            self.dpi = dpi;
            if let Some(queue) = self.queue.as_mut() {
                queue.rezoom();
            }
        }
    }

    /// The physical size of one page in millimetres, if it has one. The canvas
    /// needs it to turn a hit rectangle in page space into one on screen.
    pub fn page_size_mm(&self, page: usize) -> Option<(f32, f32)> {
        self.doc
            .as_ref()
            .and_then(|doc| doc.page(page))
            .map(|page| (page.page_box.w as f32, page.page_box.h as f32))
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

    /// The sentence the canvas shows when a page would not rasterize.
    /// The status bar carries the engine's own message, so the canvas says
    /// only that something failed rather than repeating it verbatim.
    pub fn error_label(&self) -> String {
        self.i18n.text(MessageId::ReaderErrorLabel)
    }

    /// The reader's language bundle, for widgets that need a string.
    pub fn i18n(&self) -> &I18n {
        &self.i18n
    }

    /// The locale every label is currently drawn in.
    pub fn locale(&self) -> steleofd_i18n::Locale {
        self.i18n.locale().clone()
    }

    /// The locales this build can switch to, from the bundled catalogues.
    pub fn locales(&self) -> Vec<steleofd_i18n::Locale> {
        self.i18n.available_locales().to_vec()
    }

    /// Switch every label to `locale`. The page cache survives, because the
    /// pages are pixels and a label is a word: a translated interface does not
    /// make a rendered page wrong.
    pub fn set_locale(&mut self, locale: steleofd_i18n::Locale) {
        if self.i18n.locale() == &locale {
            return;
        }
        self.i18n.set_locale(locale);
        // The description is assembled from labels, so it is rebuilt — but only
        // while a document is open, because with none there is nothing to say.
        if self.doc.is_some() {
            self.info = Some(self.describe());
        }
    }

    /// The name of `locale` written in that locale, so a reader who cannot
    /// read the current language can still find theirs. It comes from the
    /// catalogue because it is text, and all the reader's text lives there.
    pub fn locale_name(&self, locale: &steleofd_i18n::Locale) -> String {
        let key = match locale.tag() {
            "zh-CN" => MessageId::LocaleNameZh,
            _ => MessageId::LocaleNameEn,
        };
        self.i18n.text(key)
    }

    /// The rendered page if it is cached at the current resolution.
    pub fn cached(&self, page: usize) -> Option<&viewer::RenderedPage> {
        self.cache.get(page, self.dpi)
    }

    /// Rasterize a page, or return the cached one.
    /// Rendering happens off the draw path, so this asks the worker and does
    /// not wait; the page arrives through `poll` on a later frame.
    pub fn page_image(&mut self, page: usize) -> Result<(), AppError> {
        if self.cache.get(page, self.dpi).is_some() {
            return Ok(());
        }
        if self.doc.is_none() {
            return Ok(());
        }
        if let Some(queue) = self.queue.as_mut() {
            queue.request(page, self.dpi);
        }
        Ok(())
    }

    /// Take whatever the worker has finished and put it in the cache.
    /// Called once per frame before anything is drawn, so a page that landed
    /// during the last frame is on screen in this one.
    pub fn poll(&mut self) {
        self.collect_exports();
        let Some(queue) = self.queue.as_mut() else {
            return;
        };
        for result in queue.drain() {
            match result.outcome {
                Ok(rendered) => self.cache.insert(result.page, result.dpi, rendered),
                Err(message) => self.set_error(message),
            }
        }
    }

    /// Whether the worker still owes the reader a page at the current zoom.
    pub fn is_rendering(&self) -> bool {
        self.queue.as_ref().is_some_and(RenderQueue::is_busy)
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

    /// Act on a keyboard shortcut. Paging goes through the same methods the
    /// toolbar buttons call, so a key and a click cannot drift apart, and it
    /// is all testable without a keyboard.
    pub fn apply(&mut self, action: shortcuts::Action) {
        use shortcuts::Action;
        match action {
            Action::NextPage => self.next_page(),
            Action::PreviousPage => self.previous_page(),
            Action::FirstPage => self.go_to(0),
            Action::LastPage => self.go_to(self.page_count().saturating_sub(1)),
            Action::ZoomIn => self.step_zoom(ZOOM_STEP),
            Action::ZoomOut => self.step_zoom(-ZOOM_STEP),
            Action::ZoomReset => self.set_dpi(viewer::DEFAULT_DPI),
            Action::NextHit => self.next_hit(),
            Action::PreviousHit => self.previous_hit(),
            Action::ToggleSearch => self.set_search_open(!self.search_open),
            Action::ToggleSignatures => self.set_signatures_open(!self.signatures_open),
            // Focus and Escape are the window's business: they are about where
            // the caret is, which the state does not track.
            Action::FocusSearch | Action::Escape => {}
            Action::Open => self.request_open(),
            Action::ExportPdf => self.request_export(crate::export_worker::ExportFormat::Pdf),
        }
    }

    /// Move the resolution by `delta`, staying inside the permitted range.
    fn step_zoom(&mut self, delta: f32) {
        self.set_dpi(self.dpi + delta);
    }

    /// The zoom label shown in the toolbar.
    pub fn zoom_label(&self) -> String {
        self.i18n.text_with(
            MessageId::ReaderZoom,
            [("percent", (self.dpi as i32).to_string().into())],
        )
    }

    /// The document line shown in the toolbar, localized.
    pub fn info_label(&self) -> String {
        match self.file_name() {
            Some(name) => self
                .i18n
                .text_with(MessageId::ReaderInfo, [("name", name.into())]),
            None => String::new(),
        }
    }

    /// Whether the current page holds no visible ink.
    /// It asks for the page first, but the answer is only known once the
    /// raster has landed; an uncached page is not blank, merely unknown.
    pub fn current_page_is_blank(&mut self) -> bool {
        if self.page_count() == 0 {
            return true;
        }
        let page = self.current;
        let _ = self.page_image(page);
        self.cache
            .get(page, self.dpi)
            .is_some_and(|rendered| viewer::looks_blank(&rendered.image))
    }

    /// Whether the page at `page` is cached and holds no visible ink.
    pub fn page_is_blank(&self, page: usize) -> bool {
        self.cache
            .get(page, self.dpi)
            .is_some_and(|rendered| viewer::looks_blank(&rendered.image))
    }

    /// The notice shown when the current page has nothing on it.
    pub fn blank_notice(&self) -> String {
        self.i18n.text(MessageId::ReaderBlankPage)
    }

    /// The hint shown on the empty window, where a file can be dropped.
    pub fn drop_hint(&self) -> String {
        self.i18n.text(MessageId::ReaderDropHint)
    }

    /// The label on the button that closes the document.
    pub fn close_label(&self) -> String {
        self.i18n.text(MessageId::ReaderClose)
    }

    /// Whether the search panel is showing.
    pub fn search_open(&self) -> bool {
        self.search_open
    }

    /// Show or hide the search panel. Hiding it keeps the query and the hits,
    /// so reopening the panel does not make the reader type it all again.
    pub fn set_search_open(&mut self, open: bool) {
        self.search_open = open;
    }

    /// The text being looked for.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replace the query and search for it. An empty query clears the results
    /// rather than matching everything: the reader who emptied the box is done
    /// searching, not asking for the whole document.
    pub fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query = query.to_string();
        self.run_search();
    }

    /// Search for the current query, replacing the hits.
    pub fn run_search(&mut self) {
        self.hits.clear();
        self.hit_index = None;
        if self.query.is_empty() {
            return;
        }
        let Some(doc) = self.doc.as_ref() else {
            return;
        };
        let options = SearchOptions::default();
        self.hits = doc
            .search(&self.query, options)
            .into_iter()
            .map(|hit| Hit {
                page: hit.page,
                snippet: hit.snippet,
                rect: hit.rect,
            })
            .collect();
        if !self.hits.is_empty() {
            self.hit_index = Some(0);
        }
    }

    /// Every hit, in page order.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// Which hit the reader is on, if any.
    pub fn hit_index(&self) -> Option<usize> {
        self.hit_index
    }

    /// The hit the reader is on, if any.
    pub fn current_hit(&self) -> Option<&Hit> {
        self.hit_index.and_then(|index| self.hits.get(index))
    }

    /// Whether a real search ran and found nothing. An empty query is not a
    /// search, and neither is a query made before a document was open, so
    /// neither reports "no matches".
    pub fn no_match(&self) -> bool {
        !self.query.is_empty() && self.doc.is_some() && self.hits.is_empty()
    }

    /// Move to the next hit, wrapping to the first.
    pub fn next_hit(&mut self) {
        if self.hits.is_empty() {
            return;
        }
        let next = match self.hit_index {
            Some(index) => (index + 1) % self.hits.len(),
            None => 0,
        };
        self.show_hit(next);
    }

    /// Move to the previous hit, wrapping to the last.
    pub fn previous_hit(&mut self) {
        if self.hits.is_empty() {
            return;
        }
        let previous = match self.hit_index {
            Some(0) | None => self.hits.len() - 1,
            Some(index) => index - 1,
        };
        self.show_hit(previous);
    }

    /// Jump to a hit, turning to its page: a hit the reader cannot see is not
    /// a hit they found, so this moves the document as well as the selection.
    pub fn show_hit(&mut self, index: usize) {
        let Some(hit) = self.hits.get(index) else {
            return;
        };
        self.hit_index = Some(index);
        self.go_to(hit.page);
    }

    /// The hits on one page, and which of them is the current one. The canvas
    /// paints rectangles from this, so it is derived state rather than
    /// something the window works out for itself.
    pub fn hit_rects_on(&self, page: usize) -> Vec<(steleofd_common::Rect, bool)> {
        self.hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.page == page)
            .map(|(index, hit)| (hit.rect, Some(index) == self.hit_index))
            .collect()
    }

    /// The label on the toolbar toggle that opens the search panel.
    pub fn search_heading(&self) -> String {
        self.i18n.text(MessageId::ReaderSearchTitle)
    }

    /// The heading inside the search panel. Distinct from the toggle: two
    /// controls wearing the same word on one screen make the reader guess
    /// which one they are looking at.
    pub fn search_panel_title(&self) -> String {
        self.i18n.text(MessageId::ReaderSearchPanelTitle)
    }

    /// The placeholder in the search box.
    pub fn search_placeholder(&self) -> String {
        self.i18n.text(MessageId::SearchPlaceholder)
    }

    /// How many results the current query found, in the reader's language.
    pub fn search_count_label(&self) -> String {
        self.i18n.text_with(
            MessageId::SearchResultCount,
            [("count", self.hits.len().to_string().into())],
        )
    }

    /// The notice shown when a query found nothing.
    pub fn no_match_label(&self) -> String {
        self.i18n.text(MessageId::ReaderNoMatch)
    }

    /// One row of the hit list: its page and the text around the match.
    /// The row says which page it is on because the list spans the document,
    /// and a row without a page number is a hit the reader must go find.
    pub fn hit_label(&self, index: usize, snippet: &str) -> String {
        let page = self.hits.get(index).map_or(0, |hit| hit.page + 1);
        format!("{page}  {snippet}")
    }

    /// Whether the signature panel is showing.
    pub fn signatures_open(&self) -> bool {
        self.signatures_open
    }

    /// Show or hide the signature panel, verifying when it first opens.
    /// Verification reads and hashes the whole package, so it is done once and
    /// kept rather than on every frame; closing it keeps the answer.
    pub fn set_signatures_open(&mut self, open: bool) {
        self.signatures_open = open;
        if open && self.signatures.is_none() {
            self.verify_signatures();
        }
    }

    /// Verify every signature in the document and remember the results.
    /// A document with no signatures is unsigned, not broken, so the absence
    /// is an empty list; one that cannot be checked is a third verdict.
    pub fn verify_signatures(&mut self) {
        let Some(doc) = self.doc.as_mut() else {
            self.signatures = Some(Vec::new());
            return;
        };
        let declared = match doc.signatures() {
            Ok(refs) => refs,
            Err(_) => {
                self.signatures = Some(Vec::new());
                return;
            }
        };
        // The verdicts come back in declaration order, so a failure is matched
        // to the signature it belongs to; and one pass yields them all, where
        // asking per signature would re-read the package each time.
        let verdicts = doc.verify_signatures();
        self.signatures = Some(
            declared
                .iter()
                .enumerate()
                .map(|(index, sig)| {
                    let verdict = verdicts
                        .as_ref()
                        .ok()
                        .and_then(|statuses| statuses.get(index).copied())
                        .map_or(Verdict::Unverified, Verdict::from_status);
                    SignatureReport {
                        id: sig.id.clone(),
                        verdict,
                        signer: None,
                        signed_at: None,
                        algorithm: None,
                    }
                })
                .collect(),
        );
    }

    /// The verification results, or an empty list if none has been asked for.
    pub fn signatures(&self) -> &[SignatureReport] {
        self.signatures.as_deref().unwrap_or(&[])
    }

    /// How many signatures the open document carries, whether or not verified.
    pub fn signature_count(&self) -> usize {
        self.signatures().len()
    }

    /// The verdict text for one signature.
    pub fn signature_status_label(&self, report: &SignatureReport) -> String {
        let key = match report.verdict {
            Verdict::Valid => MessageId::SignValid,
            Verdict::Tampered => MessageId::SignTampered,
            Verdict::Unverified => MessageId::SignUnverified,
        };
        self.i18n.text(key)
    }

    /// The heading on the signature panel.
    pub fn signatures_heading(&self) -> String {
        self.i18n.text(MessageId::SignPanelTitle)
    }

    /// How many signatures the document has, in the reader's language.
    pub fn signature_count_label(&self) -> String {
        self.i18n.text_with(
            MessageId::SignCount,
            [("count", self.signature_count().to_string().into())],
        )
    }

    /// The signer line for one signature, when the document names one.
    pub fn signature_signer_label(&self, report: &SignatureReport) -> Option<String> {
        report.signer.as_ref().map(|name| {
            self.i18n
                .text_with(MessageId::SignSigner, [("name", name.clone().into())])
        })
    }

    /// The signing-time line for one signature, when the document records one.
    pub fn signature_time_label(&self, report: &SignatureReport) -> Option<String> {
        report.signed_at.as_ref().map(|time| {
            self.i18n
                .text_with(MessageId::SignSignedAt, [("time", time.clone().into())])
        })
    }

    /// The algorithm line for one signature, when the descriptor names one.
    pub fn signature_algorithm_label(&self, report: &SignatureReport) -> Option<String> {
        report.algorithm.as_ref().map(|algo| {
            self.i18n
                .text_with(MessageId::SignAlgorithm, [("algo", algo.clone().into())])
        })
    }

    /// Ask to export the pages on screen. The host answers with a
    /// destination, because only the platform knows where a file should land.
    pub fn request_export(&mut self, format: ExportFormat) {
        if !self.is_open() {
            return;
        }
        self.want_export = Some(format);
    }

    /// Whether the window should ask the platform for a destination.
    pub fn wants_export(&self) -> Option<ExportFormat> {
        self.want_export
    }

    /// Consume the export request, yielding the format and the pages to write.
    pub fn take_export_request(&mut self) -> Option<(ExportFormat, Vec<usize>)> {
        let format = self.want_export.take()?;
        let pages = self.export_pages();
        Some((format, pages))
    }

    /// The pages an export covers: the reader's selection rather than the whole
    /// document, so a spread exports as the two pages shown and a search result
    /// exports the page it found.
    pub fn export_pages(&self) -> Vec<usize> {
        if self.page_count() == 0 {
            return Vec::new();
        }
        self.visible()
    }

    /// Export `pages` as `format` into `destination`, off the draw path. The
    /// destination is the file for PDF and the directory for PNG. Starting an
    /// export drops the previous report, so the bar never describes a stale file.
    pub fn export_to(&mut self, format: ExportFormat, pages: Vec<usize>, destination: PathBuf) {
        let Some(bytes) = self.bytes.clone() else {
            return;
        };
        if pages.is_empty() {
            return;
        }
        // Read the stem before the queue is borrowed, since it needs `self`.
        let stem = self.export_stem();
        let dpi = self.dpi;
        self.export_generation += 1;
        let generation = self.export_generation;
        self.export_report = None;
        let queue = self
            .exports
            .get_or_insert_with(|| ExportQueue::spawn(bytes));
        queue.submit(ExportRequest {
            generation,
            format,
            pages,
            dpi,
            destination,
            stem,
        });
    }

    /// Whether an export is still being written.
    pub fn is_exporting(&self) -> bool {
        self.exports.as_ref().is_some_and(ExportQueue::is_busy)
    }

    /// What the newest finished export wrote, if it has finished.
    pub fn export_report(&self) -> Option<&ExportReport> {
        self.export_report.as_ref()
    }

    /// The filename stem exported files are named from. A dropped file has no
    /// path, so it falls back to a fixed name rather than files called nothing.
    fn export_stem(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".to_string())
    }

    /// Collect finished exports into the report and the error line. Files
    /// written before a failure are kept in the report, because they exist.
    pub fn collect_exports(&mut self) {
        let Some(queue) = self.exports.as_mut() else {
            return;
        };
        for result in queue.drain() {
            match result.outcome {
                Ok(()) => {
                    self.export_report = Some(ExportReport {
                        format: result.format,
                        pages: result.pages,
                        written: result.written,
                    });
                    self.info = None;
                }
                // Files written before the failure still exist, so the report
                // is kept and the failure is stated rather than replacing it.
                Err(message) => self.set_error(message),
            }
        }
    }

    /// What the status bar says about the last export. The two formats count
    /// differently because they are different things: one PDF holding two pages
    /// is one file but two pages, and reporting files would halve a spread.
    pub fn export_label(&self) -> Option<String> {
        let report = self.export_report.as_ref()?;
        let path = report.written.first().map(|p| p.display().to_string());
        Some(match report.format {
            ExportFormat::Pdf => self.i18n.text_with(
                MessageId::ReaderExportPdfDone,
                [
                    ("count", report.pages.to_string().into()),
                    ("path", path.unwrap_or_default().into()),
                ],
            ),
            ExportFormat::Png => self.i18n.text_with(
                MessageId::ReaderExportPngDone,
                [
                    ("count", report.written.len().to_string().into()),
                    ("path", path.unwrap_or_default().into()),
                ],
            ),
        })
    }

    /// Open a file the platform handed over, choosing bytes or path.
    ///
    /// Some desktops send bytes and others a path; the path is cheaper.
    pub fn open_dropped(&mut self, dropped: DroppedFile) -> Result<(), AppError> {
        match dropped {
            DroppedFile::Path(path) => self.open_path(&path),
            DroppedFile::Bytes { name, bytes } => self.open_bytes(&bytes, Some(&name)),
        }
    }

    /// What the canvas should draw, decided here so it can be tested.
    /// This is zone Z3: the window paints the answer and forms no opinion.
    /// It reports `Pending` rather than rendering on the draw path.
    pub fn canvas(&mut self) -> Canvas {
        if self.page_count() == 0 {
            return Canvas::Empty;
        }
        for page in self.visible() {
            let _ = self.page_image(page);
        }
        let page = self.current;
        match self.cache.get(page, self.dpi) {
            Some(rendered) if viewer::looks_blank(&rendered.image) => Canvas::Blank,
            Some(_) => Canvas::Page,
            None if self.error.is_some() => Canvas::Failed,
            None => Canvas::Pending,
        }
    }

    /// The window title for the current document.
    pub fn title(&self) -> String {
        match self.file_name() {
            Some(name) => format!("{name} — SteleOFD Reader"),
            None => "SteleOFD Reader".to_string(),
        }
    }
}

/// The four states of the canvas, in the order the window checks them.
///
/// A blank page is content, so it is painted like a page and only annotated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Canvas {
    /// No document is open, so the drop hint is shown instead.
    Empty,
    /// The page would not rasterize. The wording lives in Z4.
    Failed,
    /// The page is rasterizing and has not landed in the cache yet.
    Pending,
    /// The page rasterized to nothing but white.
    Blank,
    /// The page is ready to paint.
    Page,
}
