//! Plain data the bindings hand to JavaScript.
//! No wasm types live here, so every shape is unit-testable on the host:
//! the browser gets these objects, and `Intl` formats their numbers.

use serde::Serialize;
use steleofd_core::Document;
use steleofd_error::OfdError;
use steleofd_font::FontSource;
use steleofd_search::SearchOptions;

/// What an open document declares, at a glance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentInfo {
    /// Version string, such as `1.0`.
    pub version: String,
    /// Number of pages.
    pub pages: usize,
    /// Page width in millimetres.
    pub width_mm: f64,
    /// Page height in millimetres.
    pub height_mm: f64,
    /// Fonts the container embeds.
    pub embedded_fonts: usize,
    /// Signatures the document points at.
    pub signatures: usize,
}

impl DocumentInfo {
    /// Gather the declaration-level facts.
    pub fn of(doc: &mut Document) -> Self {
        let page_box = doc.model().common_data.page_area.physical_box;
        let embedded = doc
            .fonts()
            .registry()
            .iter()
            .filter(|entry| matches!(entry.source, FontSource::Embedded { .. }))
            .count();
        let signatures = doc.signatures().map(|list| list.len()).unwrap_or(0);
        Self {
            version: doc.version(),
            pages: doc.page_count(),
            width_mm: page_box.w,
            height_mm: page_box.h,
            embedded_fonts: embedded,
            signatures,
        }
    }
}

/// One search match, trimmed for a result list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHitData {
    /// Zero-based page holding the match.
    pub page: usize,
    /// Text around the match.
    pub snippet: String,
}

/// The outcome of verifying one signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignatureData {
    /// Archive path of the signature descriptor.
    pub location: String,
    /// Whether the signature still matches the content it covers.
    pub valid: bool,
}

/// Search every page, returning the trimmed hits.
pub fn search_hits(doc: &Document, needle: &str, options: SearchOptions) -> Vec<SearchHitData> {
    doc.search(needle, options)
        .iter()
        .map(|hit| SearchHitData {
            page: hit.page,
            snippet: hit.snippet.clone(),
        })
        .collect()
}

/// Verify every signature, in document order.
/// A document that declares no signatures is an error, matching the CLI.
pub fn signature_statuses(doc: &mut Document) -> Result<Vec<SignatureData>, OfdError> {
    let refs = doc.signatures()?;
    let statuses = doc.verify_signatures()?;
    Ok(refs
        .iter()
        .zip(&statuses)
        .map(|(sig, status)| SignatureData {
            location: sig.base_loc.clone(),
            valid: matches!(status, steleofd_sign::VerifyStatus::Valid),
        })
        .collect())
}
