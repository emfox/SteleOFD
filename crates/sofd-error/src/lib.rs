//! Unified error taxonomy.
//! One enum per subsystem, each converting losslessly into [`OfdError`] and
//! carrying context such as path, page and object id.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use thiserror::Error;

/// Top-level error that every failure converges to.
#[derive(Debug, Error)]
pub enum OfdError {
    /// Filesystem or stream failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failure inside the ZIP container layer.
    #[error("container error: {0}")]
    Package(#[from] PackageError),

    /// XML parsing or serialization failure.
    #[error("XML error: {0}")]
    Xml(#[from] XmlError),

    /// Document violates the format specification.
    #[error("format error: {0}")]
    Format(#[from] FormatError),

    /// Rendering failure.
    #[error("render error: {0}")]
    Render(#[from] RenderError),

    /// Font handling failure.
    #[error("font error: {0}")]
    Font(#[from] FontError),

    /// Signature creation or verification failure.
    #[error("signature error: {0}")]
    Signature(#[from] SignatureError),

    /// Cryptographic operation failure.
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),

    /// Format conversion failure.
    #[error("conversion error: {0}")]
    Convert(#[from] ConvertError),

    /// Document editing failure.
    #[error("edit error: {0}")]
    Edit(#[from] EditError),

    /// Printing failure.
    #[error("print error: {0}")]
    Print(#[from] PrintError),

    /// Document is encrypted and needs a password.
    #[error("document is encrypted, password required")]
    PasswordRequired,

    /// Compliance validation failed.
    #[error("compliance check failed: {0}")]
    Compliance(String),

    /// A resource budget was exceeded while guarding against hostile documents.
    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
}

impl OfdError {
    /// Whether this means the input document is damaged, so callers may degrade.
    pub fn is_corruption(&self) -> bool {
        matches!(
            self,
            OfdError::Format(_) | OfdError::Package(PackageError::Corrupted { .. })
        )
    }
}

/// Convenience result alias.
pub type Result<T, E = OfdError> = std::result::Result<T, E>;

/// Container layer errors.
#[derive(Debug, Error)]
pub enum PackageError {
    /// Archive structure is damaged or malformed.
    #[error("container structure is corrupted: {reason}")]
    Corrupted {
        /// What went wrong.
        reason: String,
    },
    /// A required entry is absent.
    #[error("container is missing required entry: {entry}")]
    MissingEntry {
        /// Entry path.
        entry: String,
    },
    /// An entry path escaped the archive root, indicating Zip Slip.
    #[error("illegal entry path (possible path traversal): {path}")]
    IllegalPath {
        /// Offending entry path.
        path: String,
    },
    /// Decompressed size exceeded the configured ceiling.
    #[error("decompressed size limit exceeded: {actual} bytes, limit {limit} bytes")]
    DecompressionLimit {
        /// Bytes actually produced.
        actual: u64,
        /// Configured ceiling.
        limit: u64,
    },
    /// Entry count exceeded the configured ceiling.
    #[error("entry count limit exceeded: {actual} > {limit}")]
    EntryCountLimit {
        /// Entries found.
        actual: usize,
        /// Configured ceiling.
        limit: usize,
    },
}

/// XML layer errors.
#[derive(Debug, Error)]
pub enum XmlError {
    /// Well-formedness or syntax violation.
    #[error("XML syntax error at line {line}, column {col}: {msg}")]
    Syntax {
        /// One-based line number.
        line: u64,
        /// One-based column number.
        col: u64,
        /// Description of the problem.
        msg: String,
    },
    /// A required attribute is absent.
    #[error("element <{element}> is missing required attribute {attr}")]
    MissingAttribute {
        /// Element name.
        element: String,
        /// Attribute name.
        attr: String,
    },
    /// An attribute value could not be parsed into the expected type.
    #[error("element <{element}> attribute {attr}={value:?} could not be parsed")]
    InvalidAttribute {
        /// Element name.
        element: String,
        /// Attribute name.
        attr: String,
        /// Raw value as written.
        value: String,
    },
    /// An extension element is not supported.
    #[error("unsupported extension element: {0}")]
    UnsupportedExtension(String),
}

/// Specification conformance errors.
#[derive(Debug, Error)]
pub enum FormatError {
    /// Not a valid OFD file; the entry point is absent.
    #[error("not a valid OFD file (missing OFD.xml entry point)")]
    NotOfd,
    /// The document declares no root path to its own XML.
    #[error("document body declares no root path")]
    MissingRoot,
    /// A path object carries no drawing commands.
    #[error("path object declares no drawing data")]
    MissingPathData,
    /// Document version is not supported.
    #[error("unsupported OFD version: {0}")]
    UnsupportedVersion(String),
    /// A referenced object or resource does not exist.
    #[error("dangling reference: {kind} with id {id}")]
    DanglingReference {
        /// Kind of resource referenced.
        kind: &'static str,
        /// Resource id.
        id: String,
    },
    /// References form a cycle.
    #[error("circular reference detected: {chain}")]
    CircularReference {
        /// The cycle as a readable chain.
        chain: String,
    },
    /// Requested page does not exist.
    #[error("page index out of range: {index} (document has {total} pages)")]
    PageOutOfRange {
        /// Requested index.
        index: usize,
        /// Total pages available.
        total: usize,
    },
    /// The document declares no signature list at all.
    #[error("document declares no signatures")]
    MissingSignatures,
    /// Recursion went deeper than allowed.
    #[error("parse recursion depth exceeded: {0}")]
    MaxDepthExceeded(usize),
}

/// Rendering errors.
#[derive(Debug, Error)]
pub enum RenderError {
    /// Backend cannot express this command.
    #[error("backend {backend} does not support operation: {op}")]
    UnsupportedOp {
        /// Backend name.
        backend: &'static str,
        /// Command name.
        op: &'static str,
    },
    /// Requested output size is invalid.
    #[error("invalid output size: {w}x{h}")]
    InvalidSize {
        /// Width in pixels.
        w: u32,
        /// Height in pixels.
        h: u32,
    },
    /// Image decoding failed.
    #[error("image decode failed on page {page}: {reason}")]
    ImageDecode {
        /// Page index.
        page: usize,
        /// What went wrong.
        reason: String,
    },
    /// Image encoding failed.
    #[error("image encode failed on page {page}: {reason}")]
    ImageEncode {
        /// Page index.
        page: usize,
        /// What went wrong.
        reason: String,
    },
    /// Render was aborted after exceeding its budget, degrading to partial output.
    #[error("render aborted after exceeding budget: {0}")]
    BudgetExceeded(String),
}

/// Font errors.
#[derive(Debug, Error)]
pub enum FontError {
    /// Font data could not be parsed.
    #[error("font parse failed: {0}")]
    Parse(String),
    /// Font is referenced but neither embedded nor substitutable.
    #[error("font missing with no substitute available: {family}")]
    Missing {
        /// Font family name.
        family: String,
    },
    /// The face lacks a glyph for this code point.
    #[error("font {family} is missing glyph U+{codepoint:04X}")]
    GlyphMissing {
        /// Font family name.
        family: String,
        /// Unicode code point.
        codepoint: u32,
    },
}

/// Signature errors.
#[derive(Debug, Error)]
pub enum SignatureError {
    /// Signature did not verify.
    #[error("signature verification failed: {0}")]
    Invalid(String),
    /// Content changed after signing.
    #[error("document content has been tampered with; signed range mismatch")]
    Tampered,
    /// Certificate chain could not be validated.
    #[error("certificate chain validation failed: {0}")]
    CertificateChain(String),
    /// Certificate is past its validity period.
    #[error("certificate has expired: {subject}")]
    CertificateExpired {
        /// Certificate subject.
        subject: String,
    },
    /// Signature encoding is not recognized.
    #[error("unsupported signature format: {0}")]
    UnsupportedFormat(String),
    /// Seal impression does not match the registered one.
    #[error("seal impression verification failed")]
    SealMismatch,
}

/// Cryptographic errors.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// A primitive failed to compute.
    #[error("cryptographic operation failed: {0}")]
    Operation(String),
    /// Key material is malformed.
    #[error("invalid key material: {0}")]
    InvalidKey(String),
    /// Decryption failed, meaning a wrong password or damaged data.
    #[error("decryption failed: wrong password or corrupted data")]
    DecryptFailed,
    /// Algorithm is not implemented.
    #[error("unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
}

/// Conversion errors.
#[derive(Debug, Error)]
pub enum ConvertError {
    /// Source format is not handled.
    #[error("unsupported source format: {0}")]
    UnsupportedSource(String),
    /// Target format is not handled.
    #[error("unsupported target format: {0}")]
    UnsupportedTarget(String),
    /// A structure could not be mapped into the target format.
    #[error("structure mapping failed on page {page}, object {object}: {reason}")]
    MappingFailed {
        /// Page index.
        page: usize,
        /// Object identifier.
        object: String,
        /// What went wrong.
        reason: String,
    },
    /// Result quality fell below the accepted threshold.
    #[error(
        "conversion quality below threshold: difference ratio {diff_ratio:.4} exceeds {threshold:.4}"
    )]
    QualityBelowThreshold {
        /// Measured difference ratio.
        diff_ratio: f64,
        /// Maximum accepted ratio.
        threshold: f64,
    },
    /// Conversion was cancelled by the caller.
    #[error("conversion was cancelled")]
    Cancelled,
}

/// Editing errors.
#[derive(Debug, Error)]
pub enum EditError {
    /// Target of the edit does not exist.
    #[error("edit target not found: {0}")]
    TargetNotFound(String),
    /// Permission policy rejected the operation.
    #[error("operation denied by permission policy: {0}")]
    PermissionDenied(String),
    /// Operation would break existing signatures.
    #[error(
        "this operation would invalidate {count} existing signature(s) and needs explicit confirmation"
    )]
    WouldInvalidateSignature {
        /// Number of signatures affected.
        count: usize,
    },
    /// Undo stack is empty.
    #[error("nothing to undo")]
    NothingToUndo,
}

/// Printing errors.
#[derive(Debug, Error)]
pub enum PrintError {
    /// The page selection string could not be understood.
    #[error("cannot parse page selection: {spec}")]
    InvalidSelection {
        /// The offending selection string.
        spec: String,
    },
    /// A page range runs backwards or is empty.
    #[error("invalid page range {start}-{end}")]
    InvalidRange {
        /// First page of the range.
        start: usize,
        /// Last page of the range.
        end: usize,
    },
    /// A selected page does not exist in the document.
    #[error("page {page} is out of range: document has {count} pages")]
    PageOutOfRange {
        /// The requested one-based page number.
        page: usize,
        /// Pages available in the document.
        count: usize,
    },
    /// Zero copies is not a printable job.
    #[error("copies must be at least one, got {copies}")]
    InvalidCopies {
        /// The requested copy count.
        copies: usize,
    },
    /// A scale factor that cannot produce output.
    #[error("scale factor must be positive and finite, got {scale}")]
    InvalidScale {
        /// The offending scale factor.
        scale: f64,
    },
    /// A paper size with no area.
    #[error("paper size must be positive, got {w}x{h} mm")]
    InvalidPaper {
        /// Paper width in millimetres.
        w: f64,
        /// Paper height in millimetres.
        h: f64,
    },
}
