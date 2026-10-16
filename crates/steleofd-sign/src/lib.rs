//! Electronic signature reading and verification (GB/T 33190 clause 18).
//! Subset: signature list, descriptor with references and check value, and
//! SM3 + SM2 verification against an embedded certificate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use steleofd_crypto::{Certificate, sm2_verify, sm3};
use steleofd_error::{OfdError, SignatureError};
use steleofd_package::Package;
use steleofd_xml::cursor::{Cursor, Event};

/// Outcome of verifying one signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyStatus {
    /// Signature matches the referenced content.
    Valid,
    /// Content no longer matches the signed digest.
    Tampered,
}

/// One entry of the signature list file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignRef {
    /// Identifier of the `Signature` element in the list.
    pub id: Option<String>,
    /// `Seal` for a stamp, `Sign` for a plain signature.
    pub kind: SignatureKind,
    /// Archive path of the signature descriptor file.
    pub base_loc: String,
}

/// GB/T 33190 clause 18.1: whether an entry is a seal or a plain signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignatureKind {
    /// A stamp, drawn over the page (`Seal`, the schema default).
    #[default]
    Seal,
    /// A plain electronic signature with no appearance.
    Sign,
}

/// A reference to one protected file inside the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Digest algorithm identifier, expected to be `SM3`.
    pub check_method: String,
    /// Archive path of the referenced file, relative to the package root.
    pub file_ref: String,
    /// Digest of the referenced file, as written.
    pub check_value: Vec<u8>,
}

/// A stamp placement: where a seal is drawn and which page it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct StampAnnot {
    /// Identifier of the stamp.
    pub id: String,
    /// Id of the page the stamp is placed on.
    pub page_ref: String,
    /// Where on the page the stamp sits.
    pub boundary: steleofd_common::Rect,
    /// Optional region the stamp is clipped to.
    pub clip: Option<steleofd_common::Rect>,
}

/// A parsed signature descriptor file.
#[derive(Debug, Clone, Default)]
pub struct Signature {
    /// Identifier of the signature element.
    pub id: Option<String>,
    /// DER-encoded check value (the SM2 signature).
    pub check_value: Vec<u8>,
    /// Archive path of the signed value, from `SignedValue`. The proof itself
    /// lives in a separate file there rather than inline in the descriptor.
    pub signed_value_ref: Option<String>,
    /// Protected files in signing order.
    pub references: Vec<Reference>,
    /// DER-encoded signing certificate, when embedded.
    pub certificate: Option<Vec<u8>>,
    /// Name of the signing provider, from the `ProviderName` attribute.
    pub provider: Option<String>,
    /// Signature algorithm name, when present.
    pub signature_method: Option<String>,
    /// Signing time as written, when present.
    pub sign_time: Option<String>,
    /// Stamp placements, from `StampAnnot`.
    pub stamps: Vec<StampAnnot>,
    /// Archive path of the seal image, from `Seal/BaseLoc`.
    pub seal: Option<String>,
}

/// Read the signature list file, returning one entry per signature. Clause 18.1
/// names the entry element `Signature`, on top of the older `Sign` spelling.
pub fn read_sign_list(package: &mut Package, loc: &str) -> Result<Vec<SignRef>, OfdError> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut signs = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(node) if node.is("Sign") || node.is("Signature") => {
                let a = node.attributes();
                let base = a.get("BaseLoc").unwrap_or_default();
                if !base.is_empty() {
                    signs.push(SignRef {
                        id: a.get("ID").map(str::to_string),
                        kind: match a.get("Type") {
                            Some(t) if t.eq_ignore_ascii_case("Sign") => SignatureKind::Sign,
                            _ => SignatureKind::Seal,
                        },
                        base_loc: base.to_string(),
                    });
                }
            }
            Event::Start(node) if node.is("Signs") || node.is("Signatures") => {}
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => {}
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(signs)
}

/// Read one signature descriptor file.
pub fn read_signature(package: &mut Package, base_loc: &str) -> Result<Signature, OfdError> {
    let xml = package.read_string(base_loc)?;
    let mut signature = Signature::default();
    // Clause 18.2.2: CheckMethod sits on References once and governs every
    // Reference below it. Clause 18.2.3: the proof itself is not inline; the
    // descriptor points at it with SignedValue, a path.
    let mut method = String::new();
    let mut pending: Option<usize> = None;
    let mut depth = 0usize;
    let mut in_ref_digest = false;
    let mut in_cert = false;
    let mut in_method = false;
    let mut in_time = false;
    let mut in_seal = false;
    let mut in_signed_value = false;
    let mut cursor = Cursor::new(&xml);
    loop {
        match cursor.next_event()? {
            Event::Start(node) => {
                depth += 1;
                let a = node.attributes();
                if node.is("Signature") {
                    signature.id = a.get("ID").map(str::to_string);
                } else if node.is("References") {
                    method = a.get("CheckMethod").unwrap_or_default().to_string();
                } else if node.is("Reference") {
                    signature.references.push(Reference {
                        check_method: method.clone(),
                        file_ref: a.get("FileRef").unwrap_or_default().to_string(),
                        check_value: Vec::new(),
                    });
                    pending = Some(signature.references.len() - 1);
                } else if node.is("CheckValue") && pending.is_some() {
                    in_ref_digest = true;
                } else if node.is("Cert") {
                    in_cert = true;
                } else if node.is("Provider") {
                    signature.provider = a.get("ProviderName").map(str::to_string);
                } else if node.is("SignatureMethod") {
                    in_method = true;
                } else if node.is("SignatureDateTime") {
                    in_time = true;
                } else if node.is("StampAnnot") {
                    signature.stamps.push(StampAnnot {
                        id: a.get("ID").unwrap_or_default().to_string(),
                        page_ref: a.get("PageRef").unwrap_or_default().to_string(),
                        boundary: read_box(a.get("Boundary")),
                        clip: a.get("Clip").map(|c| read_box(Some(c))),
                    });
                } else if node.is("SignedValue") {
                    in_signed_value = true;
                } else if node.is("Seal") {
                    in_seal = true;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                pending = None;
                in_ref_digest = false;
                in_cert = false;
                in_method = false;
                in_time = false;
                in_seal = false;
                in_signed_value = false;
                if depth == 0 {
                    break;
                }
            }
            Event::Eof => break,
            Event::Text(text) if depth >= 1 => {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                absorb_text(
                    text,
                    &mut signature,
                    pending,
                    in_ref_digest,
                    in_cert,
                    in_method,
                    in_time,
                    in_seal,
                    in_signed_value,
                );
            }
            Event::Text(_) => {}
        }
    }
    Ok(signature)
}

impl Signature {
    /// Read the signed value the descriptor points at, if it has one. The proof
    /// lives in its own file rather than inline, so it takes a second read.
    pub fn read_check_value(&mut self, package: &mut Package) -> Result<(), OfdError> {
        if let Some(loc) = self.signed_value_ref.clone()
            && let Some(bytes) = package.read_optional(&loc)?
        {
            self.check_value = b64_ish(&bytes);
        }
        Ok(())
    }
}

/// One seal as a page should draw it: where the stamp sits and the image bytes
/// the seal names, already read from the package.
#[derive(Debug, Clone, PartialEq)]
pub struct SealPlacement {
    /// Id of the stamp, from `StampAnnot`.
    pub id: String,
    /// Id of the page the stamp sits on, as the descriptor writes it.
    pub page_ref: String,
    /// Where on the page the seal is drawn, in page space.
    pub boundary: steleofd_common::Rect,
    /// Region the seal is clipped to, when one is declared.
    pub clip: Option<steleofd_common::Rect>,
    /// The seal image bytes, empty when the descriptor names none or the file
    /// is missing, so a caller can still place a placeholder.
    pub image: Vec<u8>,
    /// Archive path of the seal image, when one is named.
    pub image_ref: Option<String>,
}

/// Resolve a signature's stamps into drawable placements. Clause 18.2.3: the
/// descriptor names the seal image beside itself and each `StampAnnot` says
/// where on which page it is drawn. A missing image keeps the empty placement.
pub fn read_seals(
    package: &mut Package,
    base_loc: &str,
    signature: &Signature,
) -> Result<Vec<SealPlacement>, OfdError> {
    let dir = steleofd_package::parent_of(base_loc);
    let image_ref = signature
        .seal
        .as_ref()
        .map(|loc| steleofd_package::resolve(&dir, loc));
    let image = match &image_ref {
        Some(path) => package.read_optional(path)?.unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(signature
        .stamps
        .iter()
        .map(|stamp| SealPlacement {
            id: stamp.id.clone(),
            page_ref: stamp.page_ref.clone(),
            boundary: stamp.boundary,
            clip: stamp.clip,
            image: image.clone(),
            image_ref: image_ref.clone(),
        })
        .collect())
}

/// What one signature covers, and what the container holds that it does not.
/// Clause 18.2.2 scopes a signature to the files its `References` name, so a
/// document can be extended without moving an earlier seal; this is that scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    /// Id of the signature, as its descriptor writes it.
    pub id: Option<String>,
    /// Archive path of the descriptor, so a caller can tell two apart.
    pub base_loc: String,
    /// The files this signature protects, in reference order.
    pub covered: Vec<String>,
    /// Entries the container holds that no reference names: content that
    /// arrived after this signature was applied, when it is the newest one.
    pub uncovered: Vec<String>,
}

/// Report what each signature in the list covers. The union of every
/// signature's covered set is subtracted from the container, so a file that
/// any signature protects is not reported as vulnerable to a later edit.
pub fn read_coverage(package: &mut Package, list_loc: &str) -> Result<Vec<Coverage>, OfdError> {
    let refs = read_sign_list(package, list_loc)?;
    let mut reports = Vec::new();
    let mut protected: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in &refs {
        let signature = read_signature(package, &entry.base_loc)?;
        let covered: Vec<String> = signature
            .references
            .iter()
            .map(|reference| reference.file_ref.clone())
            .collect();
        protected.extend(covered.iter().cloned());
        reports.push(Coverage {
            id: entry.id.clone(),
            base_loc: entry.base_loc.clone(),
            covered,
            uncovered: Vec::new(),
        });
    }
    // The descriptor files themselves are part of the container, and listing
    // one as uncovered would be noise: it is not content a reader draws.
    protected.extend(refs.iter().map(|entry| entry.base_loc.clone()));
    protected.insert(list_loc.to_string());
    let mut uncovered: Vec<String> = package
        .entries()
        .iter()
        .filter(|path| !protected.contains(path.as_str()))
        .cloned()
        .collect();
    uncovered.sort();
    for report in &mut reports {
        report.uncovered.clone_from(&uncovered);
    }
    Ok(reports)
}

/// A signed-value file holds the proof either as raw DER or as base64 text.
fn b64_ish(bytes: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) => base64_decode(text.trim()).unwrap_or_else(|| bytes.to_vec()),
        Err(_) => bytes.to_vec(),
    }
}

/// `ST_Box` written as `left top width height` in an attribute.
fn read_box(raw: Option<&str>) -> steleofd_common::Rect {
    let Some(raw) = raw else {
        return steleofd_common::Rect::EMPTY;
    };
    let v: Vec<f64> = raw
        .split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    if v.len() < 4 {
        return steleofd_common::Rect::EMPTY;
    }
    steleofd_common::Rect::new(v[0], v[1], v[2], v[3])
}

#[allow(clippy::too_many_arguments)]
fn absorb_text(
    text: &str,
    signature: &mut Signature,
    pending: Option<usize>,
    ref_digest: bool,
    cert: bool,
    method: bool,
    time: bool,
    seal: bool,
    signed_value: bool,
) {
    if let Some(index) = pending {
        if ref_digest && let Some(bytes) = base64_decode(text) {
            signature.references[index].check_value = bytes;
        }
        return;
    }
    if seal {
        signature.seal = Some(text.to_string());
    } else if signed_value {
        signature.signed_value_ref = Some(text.to_string());
    } else if cert {
        if let Some(bytes) = base64_decode(text) {
            signature.certificate = Some(bytes);
        }
    } else if method {
        signature.signature_method = Some(text.to_string());
    } else if time {
        signature.sign_time = Some(text.to_string());
    }
}

/// Verify a signature: recompute the digest over all referenced files and
/// check the SM2 check value against the embedded certificate. The proof is
/// read from the file `SignedValue` points at, since it is not inline.
pub fn verify_signature(package: &mut Package, sig: &Signature) -> Result<VerifyStatus, OfdError> {
    let cert = sig.certificate.as_ref().ok_or_else(|| {
        OfdError::Signature(SignatureError::CertificateChain(
            "signature carries no certificate".into(),
        ))
    })?;
    let parsed = Certificate::parse_der(cert).map_err(|e| {
        OfdError::Signature(SignatureError::UnsupportedFormat(format!(
            "embedded certificate unreadable: {e}"
        )))
    })?;
    let mut sig = sig.clone();
    sig.read_check_value(package)?;
    verify_with_public_key(package, &sig, parsed.public_key())
}

/// Verify a signature against an externally supplied public key.
pub fn verify_with_public_key(
    package: &mut Package,
    sig: &Signature,
    public_key: &[u8],
) -> Result<VerifyStatus, OfdError> {
    let digest = signed_digest(package, sig)?;
    let ok = sm2_verify(public_key, &digest, &sig.check_value)
        .map_err(|e| OfdError::Signature(SignatureError::UnsupportedFormat(e.to_string())))?;
    Ok(if ok {
        VerifyStatus::Valid
    } else {
        VerifyStatus::Tampered
    })
}

/// Same as [`verify_with_public_key`], but reads the proof from the file the
/// descriptor points at first, as a caller holding only the descriptor must.
pub fn verify_descriptor(
    package: &mut Package,
    sig: &Signature,
    public_key: &[u8],
) -> Result<VerifyStatus, OfdError> {
    let mut sig = sig.clone();
    sig.read_check_value(package)?;
    verify_with_public_key(package, &sig, public_key)
}

/// Compute the digest that is signed: the SM3 digest of the concatenated
/// per-file digests, in reference order. Where a reference carries its own
/// `CheckValue` that is used, so a mismatch is visible without re-reading.
pub fn signed_digest(package: &mut Package, sig: &Signature) -> Result<Vec<u8>, OfdError> {
    let mut combined = Vec::new();
    for reference in &sig.references {
        if !reference.check_method.eq_ignore_ascii_case("SM3") {
            return Err(OfdError::Signature(SignatureError::UnsupportedFormat(
                reference.check_method.clone(),
            )));
        }
        let data = package
            .read_optional(&reference.file_ref)?
            .ok_or(OfdError::Signature(SignatureError::Tampered))?;
        let digest = sm3(&data);
        if !reference.check_value.is_empty() && reference.check_value != digest.as_slice() {
            // The descriptor's own record of the file no longer matches it.
            return Err(OfdError::Signature(SignatureError::Tampered));
        }
        combined.extend_from_slice(&digest);
    }
    Ok(sm3(&combined).to_vec())
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for ch in text.bytes() {
        if ch == b'=' || ch.is_ascii_whitespace() {
            continue;
        }
        let v = TABLE.iter().position(|c| *c == ch)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}
