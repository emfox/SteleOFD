//! The document handle every capability hangs off.

use std::collections::{BTreeSet, HashMap};

use steleofd_display::{DisplayList, DrawOp, PageDisplay};
use steleofd_error::Result;
use steleofd_font::{FontId, FontManager};
use steleofd_io::ByteSource;
use steleofd_model::document::Document as ModelDocument;
use steleofd_package::Package;
use steleofd_parser::{ParsedDocument, Parser};

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
        let registered = register_container_fonts(&mut package, &doc_dir, &mut fonts, &list)?;
        remap_fonts(&mut list, &registered);
        // The chain is built last, once every face the document declares has
        // been registered, so it covers all of them rather than the first.
        fonts.build_fallback_chain();
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

    /// Compare this revision against another, page by page. Both sides are
    /// already flattened, so the comparison is over what each draws rather
    /// than over the markup it draws it from.
    pub fn compare(&self, other: &Document) -> crate::Comparison {
        crate::compare::compare(&self.list.pages, &other.list.pages)
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

    /// What each signature covers, and what the container holds outside every
    /// signature's scope. Clause 18.2.2: a signature protects the files its
    /// references name, so this is how a carrier sees what arrived afterwards.
    #[cfg(feature = "sign")]
    pub fn signature_coverage(&mut self) -> Result<Vec<steleofd_sign::Coverage>> {
        let loc = self
            .parsed
            .ofd
            .doc_bodies
            .first()
            .and_then(|body| body.signatures.as_ref())
            .map(|loc| loc.value.clone())
            .ok_or_else(no_signatures)?;
        steleofd_sign::read_coverage(&mut self.package, &loc)
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

    /// Whether each signature carries a trusted timestamp, and whether the
    /// token really binds that signature. Clause 18.2.4: the proof hangs off
    /// the signed value, so a token that covers other bytes proves nothing.
    #[cfg(feature = "sign")]
    pub fn verify_timestamps(&mut self) -> Result<Vec<steleofd_sign::TimestampStatus>> {
        let refs = self.signatures()?;
        refs.iter()
            .map(|sig| {
                let signature = steleofd_sign::read_signature(&mut self.package, &sig.base_loc)?;
                steleofd_sign::verify_timestamp(&signature)
            })
            .collect()
    }

    /// The seals a document draws, each resolved onto the page it sits on.
    /// Clause 18.2.3: a stamp names its page by id, so the id is joined to the
    /// page index here, and the seal image travels ready to draw.
    #[cfg(feature = "sign")]
    pub fn seals(&mut self) -> Result<Vec<Seal>> {
        let refs = self.signatures()?;
        let by_id = page_ids(&self.parsed);
        let mut out = Vec::new();
        for sign in &refs {
            let signature = steleofd_sign::read_signature(&mut self.package, &sign.base_loc)?;
            let placements =
                steleofd_sign::read_seals(&mut self.package, &sign.base_loc, &signature)?;
            for placement in placements {
                let Some(&page) = by_id.get(&placement.page_ref) else {
                    continue;
                };
                out.push(Seal {
                    page,
                    id: placement.id,
                    boundary: placement.boundary,
                    clip: placement.clip,
                    image: placement.image,
                });
            }
        }
        Ok(out)
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

    /// One annotation, as a reader needs it: which page it marks up, what kind
    /// of mark it is and where its appearance sits.
    #[cfg(feature = "annot")]
    pub fn annotations(&mut self) -> Result<Vec<Annotation>> {
        let resolved = steleofd_parser::marks::read_annotations(&mut self.package, &self.parsed)?;
        Ok(resolved
            .into_iter()
            .map(|entry| Annotation {
                page: entry.page,
                id: entry.annot.id,
                kind: entry.annot.kind,
                remark: entry.annot.remark,
                parameters: entry.annot.parameters,
                visible: entry.annot.visible,
                boundary: entry.annot.boundary,
            })
            .collect())
    }
}

/// A seal resolved onto the page it sits on, ready for a reader to draw.
#[cfg(feature = "sign")]
#[derive(Debug, Clone, PartialEq)]
pub struct Seal {
    /// Zero-based page index the seal is drawn on.
    pub page: usize,
    /// Identifier written in the document.
    pub id: String,
    /// Where on the page the seal sits, in page space.
    pub boundary: steleofd_common::Rect,
    /// Region the seal is clipped to, when one is declared.
    pub clip: Option<steleofd_common::Rect>,
    /// The seal image bytes, empty when none was named or found.
    pub image: Vec<u8>,
}

/// An annotation resolved onto the page it belongs to.
#[cfg(feature = "annot")]
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// Zero-based page index the annotation marks up.
    pub page: usize,
    /// Identifier written in the document.
    pub id: String,
    /// What kind of mark it is, as written.
    pub kind: String,
    /// Free-text note, when the document carries one.
    pub remark: Option<String>,
    /// Name/value pairs from `Parameters`.
    pub parameters: Vec<(String, String)>,
    /// Whether the document says the mark is drawn.
    pub visible: bool,
    /// Where the mark's appearance sits on the page, in page space.
    pub boundary: Option<steleofd_common::Rect>,
}

/// Page id to zero-based page index, in document order.
#[cfg(feature = "sign")]
fn page_ids(parsed: &steleofd_parser::ParsedDocument) -> std::collections::HashMap<String, usize> {
    parsed
        .document
        .pages
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| (page.id.clone(), index))
        .collect()
}

/// Register the fonts the document declares, keyed by the id the display list
/// references. Referenced but undeclared ids point at one system substitute,
/// so a document that names a font without shipping it still renders.
fn register_container_fonts(
    package: &mut Package,
    doc_dir: &str,
    fonts: &mut FontManager,
    list: &DisplayList,
) -> Result<HashMap<u32, FontId>> {
    let mut map: HashMap<u32, FontId> = HashMap::new();
    let used = used_font_ids(list);
    let glyphs = used_glyphs(list);
    let declared = steleofd_parser::fonts::load_fonts(package, doc_dir)?;
    for id in &used {
        let Some(resource) = declared.get(id) else {
            continue;
        };
        let family = if resource.family.is_empty() {
            "embedded".to_string()
        } else {
            resource.family.clone()
        };
        // Only the glyphs this document draws are worth carrying: a face can
        // be an order of magnitude smaller without them, and nothing outside
        // the display list can be asked for at render time.
        let data = match glyphs.get(id).and_then(|set| set.as_ref()) {
            Some(set) => {
                std::sync::Arc::new(steleofd_font::subset(&resource.data, set.iter().copied()))
            }
            None => resource.data.clone(),
        };
        let registered =
            fonts
                .registry_mut()
                .register_embedded(resource.path.clone(), family, data);
        map.insert(*id, registered);
    }

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
    Ok(map)
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

/// The glyphs each font id is asked to draw, so a face can be subset to them.
/// A run carrying code points cannot name its glyphs without the face, so that
/// font is marked `None` and left whole rather than guessed at.
fn used_glyphs(list: &DisplayList) -> HashMap<u32, Option<BTreeSet<u16>>> {
    let mut used: HashMap<u32, Option<BTreeSet<u16>>> = HashMap::new();
    for page in &list.pages {
        for op in &page.ops {
            let DrawOp::Text(run) = op else {
                continue;
            };
            let entry = used
                .entry(run.font.0)
                .or_insert_with(|| Some(BTreeSet::new()));
            match entry {
                // Code points: without the face the glyph ids are unknown.
                _ if !run.codes_are_glyphs => *entry = None,
                Some(set) => set.extend(run.glyphs.iter().copied()),
                None => {}
            }
        }
    }
    used
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
