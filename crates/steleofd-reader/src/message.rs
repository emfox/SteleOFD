//! The reader's own message ids. They live here, not in `steleofd-i18n`,
//! because they name things only a reader shows — a toolbar toggle, a paper
//! size, an export dialog title. The engine catalogue stays engine messages.

use steleofd_i18n::{I18n, I18nBuilder, Locale};

/// The reader's English catalogue, compiled in. It is merged into the engine
/// bundle at build time, so a reader key and an engine key resolve the same way.
const EN_US: &str = include_str!("../locales/en-US/reader.ftl");

/// The reader's Chinese catalogue, compiled in.
const ZH_CN: &str = include_str!("../locales/zh-CN/reader.ftl");

/// An [`I18n`] with both engine catalogues and the reader's own merged in.
/// The reader supplies its catalogue here rather than shipping it inside the
/// engine crate, which is what keeps `steleofd-i18n` free of reader keys.
pub fn bundle() -> I18n {
    build(I18n::builder())
}

/// The same, pinned to one locale.
pub fn bundle_for(locale: Locale) -> I18n {
    build(I18n::builder().locale(locale))
}

fn build(builder: I18nBuilder) -> I18n {
    builder
        .add_resource(Locale::en_us(), EN_US)
        .add_resource(Locale::zh_cn(), ZH_CN)
        .build()
}

/// What the reader says. Each variant maps to one Fluent key in this crate's
/// own `locales/` catalogue, which [`crate::window`] loads as a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderMessage {
    /// Open a document
    Open,
    /// Close the open document
    Close,
    /// Shown where a file can be dropped
    DropHint,
    /// The document description line
    Info,
    /// Page N of M
    PageOf,
    /// Zoom level
    Zoom,
    /// Fit to width
    FitWidth,
    /// Fit to page
    FitPage,
    /// Single page layout
    LayoutSingle,
    /// Facing pages layout
    LayoutDouble,
    /// Continuous scroll layout
    LayoutContinuous,
    /// Page has no visible content
    BlankPage,
    /// The canvas says a page failed; the status bar says why
    ErrorLabel,
    /// Label on the toolbar toggle that opens the search panel
    SearchTitle,
    /// Heading inside the search panel, distinct from the toggle
    SearchPanelTitle,
    /// No match found
    NoMatch,
    /// Menu heading for the export actions
    Export,
    /// Export the pages on screen as one PDF
    ExportPdf,
    /// Export the pages on screen as one PNG file each
    ExportPng,
    /// A PDF export finished, with its page count and path
    ExportPdfDone,
    /// A PNG export finished, with its file count and path
    ExportPngDone,
    /// Title of the platform dialog choosing where the PDF goes
    ExportPdfTitle,
    /// Title of the platform dialog choosing where the PNG files go
    ExportPngTitle,
    /// Print the pages on screen
    Print,
    /// A print job finished, with its page, sheet and paper count
    PrintDone,
    /// A3 paper
    PaperA3,
    /// A4 paper
    PaperA4,
    /// A5 paper
    PaperA5,
    /// B5 paper
    PaperB5,
    /// US Letter paper
    PaperLetter,
    /// US Legal paper
    PaperLegal,
    /// The paper the document itself declares
    PaperDocument,
}

impl ReaderMessage {
    /// The Fluent key for this message.
    pub fn key(self) -> &'static str {
        match self {
            Self::Open => "reader-open",
            Self::Close => "reader-close",
            Self::DropHint => "reader-drop-hint",
            Self::Info => "reader-info",
            Self::PageOf => "reader-page-of",
            Self::Zoom => "reader-zoom",
            Self::FitWidth => "reader-fit-width",
            Self::FitPage => "reader-fit-page",
            Self::LayoutSingle => "reader-layout-single",
            Self::LayoutDouble => "reader-layout-double",
            Self::LayoutContinuous => "reader-layout-continuous",
            Self::BlankPage => "reader-blank-page",
            Self::ErrorLabel => "reader-error-label",
            Self::SearchTitle => "reader-search-title",
            Self::SearchPanelTitle => "reader-search-panel-title",
            Self::NoMatch => "reader-no-match",
            Self::Export => "reader-export",
            Self::ExportPdf => "reader-export-pdf",
            Self::ExportPng => "reader-export-png",
            Self::ExportPdfDone => "reader-export-pdf-done",
            Self::ExportPngDone => "reader-export-png-done",
            Self::ExportPdfTitle => "reader-export-pdf-title",
            Self::ExportPngTitle => "reader-export-png-title",
            Self::Print => "reader-print",
            Self::PrintDone => "reader-print-done",
            Self::PaperA3 => "reader-paper-a3",
            Self::PaperA4 => "reader-paper-a4",
            Self::PaperA5 => "reader-paper-a5",
            Self::PaperB5 => "reader-paper-b5",
            Self::PaperLetter => "reader-paper-letter",
            Self::PaperLegal => "reader-paper-legal",
            Self::PaperDocument => "reader-paper-document",
        }
    }

    /// Every message, in the same order as the catalogue lists them.
    pub const ALL: &'static [Self] = &[
        Self::Open,
        Self::Close,
        Self::DropHint,
        Self::Info,
        Self::PageOf,
        Self::Zoom,
        Self::FitWidth,
        Self::FitPage,
        Self::LayoutSingle,
        Self::LayoutDouble,
        Self::LayoutContinuous,
        Self::BlankPage,
        Self::ErrorLabel,
        Self::SearchTitle,
        Self::SearchPanelTitle,
        Self::NoMatch,
        Self::Export,
        Self::ExportPdf,
        Self::ExportPng,
        Self::ExportPdfDone,
        Self::ExportPngDone,
        Self::ExportPdfTitle,
        Self::ExportPngTitle,
        Self::Print,
        Self::PrintDone,
        Self::PaperA3,
        Self::PaperA4,
        Self::PaperA5,
        Self::PaperB5,
        Self::PaperLetter,
        Self::PaperLegal,
        Self::PaperDocument,
    ];
}
