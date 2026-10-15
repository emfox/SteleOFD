//! The document handle every capability hangs off.

use std::collections::{BTreeSet, HashMap};

use steleofd_display::{DisplayList, DrawOp, PageDisplay};
use steleofd_error::Result;
use steleofd_font::{FontId, FontManager};
use steleofd_io::ByteSource;
use steleofd_model::document::Document as ModelDocument;
use steleofd_package::Package;
use steleofd_parser::{ParsedDocument, Parser};

/// Font file extensions recognised when scanning a container.
const FONT_EXTENSIONS: [&str; 6] = ["ttf", "otf", "ttc", "otc", "woff", "woff2"];

/// An opened OFD document: pages flattened and container fonts registered,
/// so every capability works against the same prepared state.
pub struct Document {
    package: Package,
    parsed: ParsedDocument,
    list: DisplayList,
    fonts: FontManager,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("pages", &self.list.page_count)
            .field("fonts", &self.fonts)
            .finish()
    }
}

impl Document {
    /// Open a document from any byte source.
    pub fn open(source: &dyn ByteSource) -> Result<Self> {
        let mut package = Package::open(source)?;
        let mut parser = Parser::new(&mut package);
        let parsed = parser.open_document()?;
        let page_count = parsed.document.pages.len();
        let mut pages = Vec::with_capacity(page_count);
        for index in 0..page_count {
            pages.push(parser.page_display(&parsed, index)?);
        }
        let mut list = DisplayList { page_count, pages };
        let mut fonts = FontManager::new();
        let doc_dir = parsed.doc_dir.clone();
        let registered = register_container_fonts(&mut package, &doc_dir, &mut fonts, &list);
        remap_fonts(&mut list, &registered);
        Ok(Self {
            package,
            parsed,
            list,
            fonts,
        })
    }

    /// Open a document from bytes, the common case for uploads and tests.
    pub fn open_bytes(data: &[u8]) -> Result<Self> {
        let source = steleofd_io::SliceSource::new(data, "document");
        Self::open(&source)
    }

    /// Pages the document declares.
    pub fn page_count(&self) -> usize {
        self.list.page_count
    }

    /// One flattened page, or `None` when the index is out of range.
    pub fn page(&self, index: usize) -> Option<&PageDisplay> {
        self.list.page(index)
    }

    /// The whole flattened display list.
    pub fn display_list(&self) -> &DisplayList {
        &self.list
    }

    /// Faces registered from the container.
    pub fn fonts(&self) -> &FontManager {
        &self.fonts
    }

    /// Mutable access to the font manager, for callers that bring their own
    /// faces and want to register them under the ids the list already uses.
    pub fn fonts_mut(&mut self) -> &mut FontManager {
        &mut self.fonts
    }

    /// The parsed document model, for callers that need the raw structure.
    pub fn model(&self) -> &ModelDocument {
        &self.parsed.document
    }

    /// Version the document declares, such as `1.0`.
    /// The minor part is appended only when present.
    pub fn version(&self) -> String {
        let version = &self.parsed.ofd.version;
        match &version.sub {
            Some(sub) if !sub.is_empty() => format!("{}.{}", version.major, sub),
            _ => version.major.clone(),
        }
    }

    /// The container, for signature and archival work that reads entries.
    pub fn package(&mut self) -> &mut Package {
        &mut self.package
    }

    /// Rasterize one page at the given options.
    #[cfg(feature = "render")]
    pub fn render_page(
        &self,
        index: usize,
        options: &steleofd_render::RenderOptions,
    ) -> Result<steleofd_render::RasterOutput> {
        let page = self
            .page(index)
            .ok_or_else(|| out_of_range(index, self.page_count()))?;
        steleofd_render::render_page(page, &self.fonts, options)
    }

    /// Rasterize several pages, in parallel when the async feature is on.
    #[cfg(feature = "render")]
    pub fn render_pages(
        &self,
        indices: &[usize],
        options: &steleofd_render::RenderOptions,
    ) -> Vec<Result<steleofd_render::RasterOutput>> {
        #[cfg(feature = "async")]
        {
            use rayon::prelude::*;
            indices
                .par_iter()
                .map(|&index| self.render_page(index, options))
                .collect()
        }
        #[cfg(not(feature = "async"))]
        {
            indices
                .iter()
                .map(|&index| self.render_page(index, options))
                .collect()
        }
    }

    /// Plan a print job over the whole document.
    #[cfg(feature = "print")]
    pub fn print_job(
        &self,
        settings: &steleofd_print::PrintSettings,
    ) -> Result<steleofd_print::PrintJob> {
        steleofd_print::plan(&self.list, settings)
    }

    /// Plan and rasterize a print job, in print order.
    #[cfg(feature = "print")]
    pub fn print_render(
        &self,
        settings: &steleofd_print::PrintSettings,
    ) -> Result<Vec<steleofd_print::RasterPage>> {
        let job = self.print_job(settings)?;
        steleofd_print::render_job(&self.list, &self.fonts, &job)
    }

    /// Convert the whole document to one PDF file.
    #[cfg(feature = "pdf")]
    pub fn to_pdf(&self) -> Result<Vec<u8>> {
        steleofd_convert::document_to_pdf(&self.list, &steleofd_convert::PdfOptions::default())
    }

    /// Convert the named pages to one PDF file, in the order given. The
    /// selection is a list rather than a span because a reader's pages are not
    /// contiguous: facing mode shows a pair, and a search lands anywhere.
    #[cfg(feature = "pdf")]
    pub fn to_pdf_pages(&self, pages: &[usize]) -> Result<Vec<u8>> {
        steleofd_convert::document_to_pdf_pages(
            &self.list,
            pages,
            &steleofd_convert::PdfOptions::default(),
        )
    }

    /// Convert one page to SVG.
    #[cfg(feature = "convert")]
    pub fn to_svg(&self, index: usize) -> Result<String> {
        let page = self
            .page(index)
            .ok_or_else(|| out_of_range(index, self.page_count()))?;
        steleofd_convert::page_to_svg(page, &steleofd_convert::SvgOptions::default())
    }

    /// Rasterize one page and encode it as PNG.
    #[cfg(feature = "convert")]
    pub fn to_png(
        &self,
        index: usize,
        options: &steleofd_render::RenderOptions,
    ) -> Result<Vec<u8>> {
        let page = self
            .page(index)
            .ok_or_else(|| out_of_range(index, self.page_count()))?;
        steleofd_convert::png::page_to_png(page, &self.fonts, options)
    }

    /// Read the signature list the document points at.
    #[cfg(feature = "sign")]
    pub fn signatures(&mut self) -> Result<Vec<steleofd_sign::SignRef>> {
        let loc = self
            .parsed
            .ofd
            .doc_bodies
            .first()
            .and_then(|body| body.signatures.as_ref())
            .map(|loc| loc.value.clone())
            .ok_or_else(no_signatures)?;
        steleofd_sign::read_sign_list(&mut self.package, &loc)
    }

    /// Verify every signature in the list, in document order.
    #[cfg(feature = "sign")]
    pub fn verify_signatures(&mut self) -> Result<Vec<steleofd_sign::VerifyStatus>> {
        let refs = self.signatures()?;
        refs.iter()
            .map(|sig| {
                let signature = steleofd_sign::read_signature(&mut self.package, &sig.base_loc)?;
                steleofd_sign::verify_signature(&mut self.package, &signature)
            })
            .collect()
    }

    /// Start an editing session over a copy of the document model.
    #[cfg(feature = "edit")]
    pub fn edit(&self, options: steleofd_edit::EditOptions) -> steleofd_edit::EditSession {
        steleofd_edit::EditSession::new(self.parsed.document.clone(), options)
    }

    /// Full-text search over the flattened pages.
    #[cfg(feature = "search")]
    pub fn search(
        &self,
        needle: &str,
        options: steleofd_search::SearchOptions,
    ) -> Vec<steleofd_search::SearchHit> {
        let index = steleofd_search::SearchIndex::build(&self.list);
        index.search(needle, options)
    }

    /// Audit the container for archival readiness.
    #[cfg(feature = "archive")]
    pub fn audit(&mut self) -> Result<steleofd_archive::Report> {
        steleofd_archive::audit(&mut self.package)
    }
}

/// Register every font file under the document directory.
/// Returns the document id to manager id pairing, sorted on both sides.
fn register_container_fonts(
    package: &mut Package,
    doc_dir: &str,
    fonts: &mut FontManager,
    list: &DisplayList,
) -> HashMap<u32, FontId> {
    let prefix = if doc_dir.is_empty() {
        String::new()
    } else {
        format!("{doc_dir}/")
    };
    let mut paths: Vec<String> = package
        .entries()
        .iter()
        .filter(|path| path.starts_with(&prefix) && is_font_path(path))
        .cloned()
        .collect();
    paths.sort();

    let mut registered = Vec::with_capacity(paths.len());
    for path in &paths {
        let Ok(data) = package.read(path) else {
            continue;
        };
        let family = path
            .rsplit('/')
            .next()
            .and_then(|name| name.split('.').next())
            .unwrap_or("embedded")
            .to_string();
        let id =
            fonts
                .registry_mut()
                .register_embedded(path.clone(), family, std::sync::Arc::new(data));
        registered.push(id);
    }
    let used = used_font_ids(list);
    let mut map: HashMap<u32, FontId> = used.iter().copied().zip(registered).collect();

    // Referenced but not embedded: point those ids at one system substitute, so
    // a document that names a font without shipping it still renders.
    let unembedded: Vec<u32> = used
        .into_iter()
        .filter(|id| !map.contains_key(id))
        .collect();
    if !unembedded.is_empty() {
        let substitute = fonts.registry_mut().register_system(SUBSTITUTE_FAMILY);
        for id in unembedded {
            map.insert(id, substitute);
        }
    }
    map
}

/// Family used when a document references a font it does not embed.
/// A generic alias every system resolver knows, so the substitute degrades to
/// whatever sans face the machine has.
const SUBSTITUTE_FAMILY: &str = "sans-serif";

/// Every font id the display list actually references, sorted.
fn used_font_ids(list: &DisplayList) -> Vec<u32> {
    let mut used = BTreeSet::new();
    for page in &list.pages {
        for op in &page.ops {
            if let DrawOp::Text(run) = op {
                used.insert(run.font.0);
            }
        }
    }
    used.into_iter().collect()
}

/// Whether a container path looks like a font file.
fn is_font_path(path: &str) -> bool {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    FONT_EXTENSIONS.contains(&extension.as_str())
}

/// Rewrite font ids in the display list through the pairing.
fn remap_fonts(list: &mut DisplayList, map: &HashMap<u32, FontId>) {
    for page in &mut list.pages {
        for op in &mut page.ops {
            if let DrawOp::Text(run) = op
                && let Some(&id) = map.get(&run.font.0)
            {
                run.font = id;
            }
        }
    }
}

/// The page index was out of range.
#[cfg(any(feature = "render", feature = "convert", feature = "pdf"))]
fn out_of_range(index: usize, total: usize) -> steleofd_error::OfdError {
    steleofd_error::OfdError::Format(steleofd_error::FormatError::PageOutOfRange { index, total })
}

/// The document declares no signatures at all.
#[cfg(feature = "sign")]
fn no_signatures() -> steleofd_error::OfdError {
    steleofd_error::OfdError::Format(steleofd_error::FormatError::MissingSignatures)
}
