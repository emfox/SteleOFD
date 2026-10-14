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
    /// No match found
    ReaderNoMatch,

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
            Self::ReaderPageOf => "reader-page-of",
            Self::ReaderZoom => "reader-zoom",
            Self::ReaderFitWidth => "reader-fit-width",
            Self::ReaderFitPage => "reader-fit-page",
            Self::ReaderLayoutSingle => "reader-layout-single",
            Self::ReaderLayoutDouble => "reader-layout-double",
            Self::ReaderLayoutContinuous => "reader-layout-continuous",
            Self::ReaderBlankPage => "reader-blank-page",
            Self::ReaderNoMatch => "reader-no-match",

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

            // Edit
            Self::EditConfirmDeletePages => "edit-confirm-delete-pages",
            Self::EditWarnInvalidateSignature => "edit-warn-invalidate-signature",
            Self::EditUndo => "edit-undo",
            Self::EditRedo => "edit-redo",

            // Search
            Self::SearchResultCount => "search-result-count",
            Self::SearchPlaceholder => "search-placeholder",

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

    /// Whether the message takes variables, read by the static checker
    pub fn is_parametric(self) -> bool {
        matches!(
            self,
            Self::AppLoading
                | Self::ReaderPageOf
                | Self::ReaderZoom
                | Self::ConvertProgress
                | Self::ConvertFailed
                | Self::SignCount
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
        )
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}
