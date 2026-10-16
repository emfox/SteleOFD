//! Message id constants.
//! Kebab-case English ids mapping one to one onto `.ftl` keys; never a sentence,
//! since rewording would then touch every catalogue.

use std::fmt;

/// A localizable message identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageId {
    // Application level
    /// Application name
    AppName,
    /// Ready
    AppReady,
    /// Loading, with a percentage
    AppLoading,
    /// Operation cancelled
    AppCancelled,
    /// Generic confirm
    CommonConfirm,
    /// Generic cancel
    CommonCancel,
    /// Generic save
    CommonSave,
    /// Generic close
    CommonClose,
    /// Generic retry
    CommonRetry,
    /// Generic delete
    CommonDelete,

    // Reader module
    /// Open a document
    ReaderOpen,
    /// Close the open document
    ReaderClose,
    /// Shown where a file can be dropped
    ReaderDropHint,
    /// The document description line
    ReaderInfo,
    /// Page N of M
    ReaderPageOf,
    /// Zoom level
    ReaderZoom,
    /// Fit to width
    ReaderFitWidth,
    /// Fit to page
    ReaderFitPage,
    /// Single page layout
    ReaderLayoutSingle,
    /// Facing pages layout
    ReaderLayoutDouble,
    /// Continuous scroll layout
    ReaderLayoutContinuous,
    /// Page has no visible content
    ReaderBlankPage,
    /// The canvas says a page failed; the status bar says why
    ReaderErrorLabel,
    /// Label on the toolbar toggle that opens the search panel
    ReaderSearchTitle,
    /// Heading inside the search panel, distinct from the toggle
    ReaderSearchPanelTitle,
    /// No match found
    ReaderNoMatch,
    /// Menu heading for the export actions
    ReaderExport,
    /// Export the pages on screen as one PDF
    ReaderExportPdf,
    /// Export the pages on screen as one PNG file each
    ReaderExportPng,
    /// A PDF export finished, with its page count and path
    ReaderExportPdfDone,
    /// A PNG export finished, with its file count and path
    ReaderExportPngDone,
    /// Title of the platform dialog choosing where the PDF goes
    ReaderExportPdfTitle,
    /// Title of the platform dialog choosing where the PNG files go
    ReaderExportPngTitle,
    /// Print the pages on screen
    ReaderPrint,
    /// A print job finished, with its page count, sheet count and paper
    ReaderPrintDone,
    /// Paper size A3, as a printer names it
    ReaderPaperA3,
    /// Paper size A4, as a printer names it
    ReaderPaperA4,
    /// Paper size A5, as a printer names it
    ReaderPaperA5,
    /// Paper size B5, as a printer names it
    ReaderPaperB5,
    /// Paper size Letter, as a printer names it
    ReaderPaperLetter,
    /// Paper size Legal, as a printer names it
    ReaderPaperLegal,
    /// The paper the document itself declares
    ReaderPaperDocument,
    /// Name of this language, written in itself
    LocaleNameEn,
    /// Name of the Chinese locale, written in itself
    LocaleNameZh,

    // Conversion module
    /// Conversion started
    ConvertStart,
    /// Conversion progress, completed of total
    ConvertProgress,
    /// Conversion finished
    ConvertDone,
    /// Conversion failed, with page and reason
    ConvertFailed,
    /// Output format
    ConvertOutputFormat,
    /// Page range
    ConvertPageRange,
    /// Output resolution in DPI
    ConvertDpi,
    /// Keep bookmarks and links
    ConvertKeepBookmarks,
    /// Keep seals as images
    ConvertKeepSignatures,

    // Signature module
    /// Signature is valid
    SignValid,
    /// Signature is invalid
    SignInvalid,
    /// Signature could not be verified
    SignUnverified,
    /// Certificate has expired
    SignExpired,
    /// Document integrity intact
    SignIntegrityOk,
    /// Document has been tampered with
    SignTampered,
    /// Signature count, plural
    SignCount,
    /// Signer
    SignSigner,
    /// Signed at
    SignSignedAt,
    /// Algorithm
    SignAlgorithm,
    /// Issuer
    SignIssuer,
    /// Export verification report
    SignExportReport,
    /// Heading on the signature panel in the reader
    SignPanelTitle,

    // Annotation module
    /// Heading on the annotation panel in the reader
    AnnotPanelTitle,
    /// Annotation count, plural
    AnnotCount,
    /// The document carries no annotations
    AnnotNone,

    // Comparison module
    /// Heading on the comparison panel in the reader
    ComparePanelTitle,
    /// Menu item that opens a second revision to compare against
    CompareOpenSecond,
    /// Whether every page matches
    CompareIdentical,
    /// Page count of a change summary, plural
    CompareChanged,
    /// One page row: its index and what changed
    ComparePage,

    // Edit module
    /// Confirm page deletion
    EditConfirmDeletePages,
    /// Warn that signatures will be invalidated, with a count
    EditWarnInvalidateSignature,
    /// Undo
    EditUndo,
    /// Redo
    EditRedo,

    // Search module
    /// Search result count, plural
    SearchResultCount,
    /// Search input placeholder
    SearchPlaceholder,

    // Command line output
    /// Page count line in document info
    CliPages,
    /// Page size line in document info
    CliPageSize,
    /// Version line in document info
    CliVersion,
    /// Embedded font count line in document info
    CliEmbeddedFonts,
    /// Signature count line in document info
    CliSignatures,
    /// The document declares no pages
    CliNoPages,
    /// Confirmation that a file was written
    CliWrote,
    /// One page rendered at a resolution
    CliRenderedPage,
    /// A batch of pages converted to a format
    CliConverted,
    /// One search hit with its snippet
    CliSearchHit,
    /// The audit found nothing
    CliAuditClean,
    /// One audit finding
    CliAuditFinding,
    /// Print job plan summary
    CliPrintPlan,
    /// One signature with its verification status
    CliSignEntry,
    /// The document declares no signatures
    CliSignNone,
    /// Prefix for a reported error
    CliError,

    // Error messages
    /// File is not a valid OFD document
    ErrNotOfd,
    /// Document requires a password
    ErrPasswordRequired,
    /// Page index out of range, with index and total
    ErrPageOutOfRange,
    /// Container is corrupted
    ErrPackageCorrupted,
    /// Font missing, named
    ErrFontMissing,
    /// Referenced resource does not exist
    ErrDanglingReference,
    /// Resource limit exceeded
    ErrResourceLimit,
    /// I/O failure
    ErrIo,
    /// Unsupported feature
    ErrUnsupported,
}

impl MessageId {
    /// The Fluent key for this message
    pub fn key(self) -> &'static str {
        match self {
            // Application level
            Self::AppName => "app-name",
            Self::AppReady => "app-ready",
            Self::AppLoading => "app-loading",
            Self::AppCancelled => "app-cancelled",
            Self::CommonConfirm => "common-confirm",
            Self::CommonCancel => "common-cancel",
            Self::CommonSave => "common-save",
            Self::CommonClose => "common-close",
            Self::CommonRetry => "common-retry",
            Self::CommonDelete => "common-delete",

            // Reader
            Self::ReaderOpen => "reader-open",
            Self::ReaderClose => "reader-close",
            Self::ReaderDropHint => "reader-drop-hint",
            Self::ReaderInfo => "reader-info",
            Self::ReaderPageOf => "reader-page-of",
            Self::ReaderZoom => "reader-zoom",
            Self::ReaderFitWidth => "reader-fit-width",
            Self::ReaderFitPage => "reader-fit-page",
            Self::ReaderLayoutSingle => "reader-layout-single",
            Self::ReaderLayoutDouble => "reader-layout-double",
            Self::ReaderLayoutContinuous => "reader-layout-continuous",
            Self::ReaderBlankPage => "reader-blank-page",
            Self::ReaderErrorLabel => "reader-error-label",
            Self::ReaderSearchTitle => "reader-search-title",
            Self::ReaderSearchPanelTitle => "reader-search-panel-title",
            Self::ReaderNoMatch => "reader-no-match",
            Self::ReaderExport => "reader-export",
            Self::ReaderExportPdf => "reader-export-pdf",
            Self::ReaderExportPng => "reader-export-png",
            Self::ReaderExportPdfDone => "reader-export-pdf-done",
            Self::ReaderExportPngDone => "reader-export-png-done",
            Self::ReaderExportPdfTitle => "reader-export-pdf-title",
            Self::ReaderExportPngTitle => "reader-export-png-title",
            Self::ReaderPrint => "reader-print",
            Self::ReaderPrintDone => "reader-print-done",
            Self::ReaderPaperA3 => "reader-paper-a3",
            Self::ReaderPaperA4 => "reader-paper-a4",
            Self::ReaderPaperA5 => "reader-paper-a5",
            Self::ReaderPaperB5 => "reader-paper-b5",
            Self::ReaderPaperLetter => "reader-paper-letter",
            Self::ReaderPaperLegal => "reader-paper-legal",
            Self::ReaderPaperDocument => "reader-paper-document",
            Self::LocaleNameEn => "locale-name-en",
            Self::LocaleNameZh => "locale-name-zh",

            // Conversion
            Self::ConvertStart => "convert-start",
            Self::ConvertProgress => "convert-progress",
            Self::ConvertDone => "convert-done",
            Self::ConvertFailed => "convert-failed",
            Self::ConvertOutputFormat => "convert-output-format",
            Self::ConvertPageRange => "convert-page-range",
            Self::ConvertDpi => "convert-dpi",
            Self::ConvertKeepBookmarks => "convert-keep-bookmarks",
            Self::ConvertKeepSignatures => "convert-keep-signatures",

            // Signature
            Self::SignValid => "sign-valid",
            Self::SignInvalid => "sign-invalid",
            Self::SignUnverified => "sign-unverified",
            Self::SignExpired => "sign-expired",
            Self::SignIntegrityOk => "sign-integrity-ok",
            Self::SignTampered => "sign-tampered",
            Self::SignCount => "sign-count",
            Self::SignSigner => "sign-signer",
            Self::SignSignedAt => "sign-signed-at",
            Self::SignAlgorithm => "sign-algorithm",
            Self::SignIssuer => "sign-issuer",
            Self::SignExportReport => "sign-export-report",
            Self::SignPanelTitle => "sign-panel-title",

            // Annotation
            Self::AnnotPanelTitle => "annot-panel-title",
            Self::AnnotCount => "annot-count",
            Self::AnnotNone => "annot-none",

            // Comparison
            Self::ComparePanelTitle => "compare-panel-title",
            Self::CompareOpenSecond => "compare-open-second",
            Self::CompareIdentical => "compare-identical",
            Self::CompareChanged => "compare-changed",
            Self::ComparePage => "compare-page",

            // Edit
            Self::EditConfirmDeletePages => "edit-confirm-delete-pages",
            Self::EditWarnInvalidateSignature => "edit-warn-invalidate-signature",
            Self::EditUndo => "edit-undo",
            Self::EditRedo => "edit-redo",

            // Search
            Self::SearchResultCount => "search-result-count",
            Self::SearchPlaceholder => "search-placeholder",

            // Command line output
            Self::CliPages => "cli-pages",
            Self::CliPageSize => "cli-page-size",
            Self::CliVersion => "cli-version",
            Self::CliEmbeddedFonts => "cli-embedded-fonts",
            Self::CliSignatures => "cli-signatures",
            Self::CliNoPages => "cli-no-pages",
            Self::CliWrote => "cli-wrote",
            Self::CliRenderedPage => "cli-rendered-page",
            Self::CliConverted => "cli-converted",
            Self::CliSearchHit => "cli-search-hit",
            Self::CliAuditClean => "cli-audit-clean",
            Self::CliAuditFinding => "cli-audit-finding",
            Self::CliPrintPlan => "cli-print-plan",
            Self::CliSignEntry => "cli-sign-entry",
            Self::CliSignNone => "cli-sign-none",
            Self::CliError => "cli-error",

            // Errors
            Self::ErrNotOfd => "err-not-ofd",
            Self::ErrPasswordRequired => "err-password-required",
            Self::ErrPageOutOfRange => "err-page-out-of-range",
            Self::ErrPackageCorrupted => "err-package-corrupted",
            Self::ErrFontMissing => "err-font-missing",
            Self::ErrDanglingReference => "err-dangling-reference",
            Self::ErrResourceLimit => "err-resource-limit",
            Self::ErrIo => "err-io",
            Self::ErrUnsupported => "err-unsupported",
        }
    }

    /// The message id for a Fluent key, or `None` when the key is unknown.
    /// The inverse of [`MessageId::key`]; used to prove the two never drift.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.key() == key)
    }

    /// Every message id, in the same order as `bundle::all_keys`.
    pub const ALL: &'static [Self] = &[
        // Application level
        Self::AppName,
        Self::AppReady,
        Self::AppLoading,
        Self::AppCancelled,
        Self::CommonConfirm,
        Self::CommonCancel,
        Self::CommonSave,
        Self::CommonClose,
        Self::CommonRetry,
        Self::CommonDelete,
        // Reader
        Self::ReaderOpen,
        Self::ReaderClose,
        Self::ReaderDropHint,
        Self::ReaderInfo,
        Self::ReaderPageOf,
        Self::ReaderZoom,
        Self::ReaderFitWidth,
        Self::ReaderFitPage,
        Self::ReaderLayoutSingle,
        Self::ReaderLayoutDouble,
        Self::ReaderLayoutContinuous,
        Self::ReaderBlankPage,
        Self::ReaderErrorLabel,
        Self::ReaderSearchTitle,
        Self::ReaderSearchPanelTitle,
        Self::ReaderNoMatch,
        Self::ReaderExport,
        Self::ReaderExportPdf,
        Self::ReaderExportPng,
        Self::ReaderExportPdfDone,
        Self::ReaderExportPngDone,
        Self::ReaderExportPdfTitle,
        Self::ReaderExportPngTitle,
        Self::ReaderPrint,
        Self::ReaderPrintDone,
        Self::ReaderPaperA3,
        Self::ReaderPaperA4,
        Self::ReaderPaperA5,
        Self::ReaderPaperB5,
        Self::ReaderPaperLetter,
        Self::ReaderPaperLegal,
        Self::ReaderPaperDocument,
        Self::LocaleNameEn,
        Self::LocaleNameZh,
        // Conversion
        Self::ConvertStart,
        Self::ConvertProgress,
        Self::ConvertDone,
        Self::ConvertFailed,
        Self::ConvertOutputFormat,
        Self::ConvertPageRange,
        Self::ConvertDpi,
        Self::ConvertKeepBookmarks,
        Self::ConvertKeepSignatures,
        // Signature
        Self::SignValid,
        Self::SignInvalid,
        Self::SignUnverified,
        Self::SignExpired,
        Self::SignIntegrityOk,
        Self::SignTampered,
        Self::SignCount,
        Self::SignSigner,
        Self::SignSignedAt,
        Self::SignAlgorithm,
        Self::SignIssuer,
        Self::SignExportReport,
        Self::SignPanelTitle,
        // Annotation
        Self::AnnotPanelTitle,
        Self::AnnotCount,
        Self::AnnotNone,
        // Comparison
        Self::ComparePanelTitle,
        Self::CompareOpenSecond,
        Self::CompareIdentical,
        Self::CompareChanged,
        Self::ComparePage,
        Self::EditConfirmDeletePages,
        Self::EditWarnInvalidateSignature,
        Self::EditUndo,
        Self::EditRedo,
        // Search
        Self::SearchResultCount,
        Self::SearchPlaceholder,
        // Command line output
        Self::CliPages,
        Self::CliPageSize,
        Self::CliVersion,
        Self::CliEmbeddedFonts,
        Self::CliSignatures,
        Self::CliNoPages,
        Self::CliWrote,
        Self::CliRenderedPage,
        Self::CliConverted,
        Self::CliSearchHit,
        Self::CliAuditClean,
        Self::CliAuditFinding,
        Self::CliPrintPlan,
        Self::CliSignEntry,
        Self::CliSignNone,
        Self::CliError,
        // Errors
        Self::ErrNotOfd,
        Self::ErrPasswordRequired,
        Self::ErrPageOutOfRange,
        Self::ErrPackageCorrupted,
        Self::ErrFontMissing,
        Self::ErrDanglingReference,
        Self::ErrResourceLimit,
        Self::ErrIo,
        Self::ErrUnsupported,
    ];

    /// Whether the message takes variables, read by the static checker
    pub fn is_parametric(self) -> bool {
        matches!(
            self,
            Self::AppLoading
                | Self::ReaderInfo
                | Self::ReaderPageOf
                | Self::ReaderZoom
                | Self::ReaderPrintDone
                | Self::ConvertProgress
                | Self::ConvertFailed
                | Self::SignCount
                | Self::AnnotCount
                | Self::CompareChanged
                | Self::SignSigner
                | Self::SignSignedAt
                | Self::SignAlgorithm
                | Self::SignIssuer
                | Self::EditWarnInvalidateSignature
                | Self::SearchResultCount
                | Self::ErrPageOutOfRange
                | Self::ErrFontMissing
                | Self::ErrDanglingReference
                | Self::ErrResourceLimit
                | Self::CliPages
                | Self::CliPageSize
                | Self::CliVersion
                | Self::CliEmbeddedFonts
                | Self::CliSignatures
                | Self::CliWrote
                | Self::CliRenderedPage
                | Self::CliConverted
                | Self::CliSearchHit
                | Self::CliAuditFinding
                | Self::CliPrintPlan
                | Self::CliSignEntry
                | Self::CliError
        )
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}
