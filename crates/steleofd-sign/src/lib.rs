//! Electronic signature reading and verification (GB/T 33190 clause 11).
//! Subset: signature list, descriptor with references and check value, and
//! SM3 + SM2 verification against an embedded certificate. Stamps and
//! certificate chain validation are follow-up work.

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
    /// Archive path of the signature descriptor file.
    pub base_loc: String,
}

/// A reference to one protected file inside the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Digest algorithm identifier, expected to be `SM3`.
    pub check_method: String,
    /// Archive path of the referenced file, relative to the package root.
    pub file_ref: String,
}

/// A parsed signature descriptor file.
#[derive(Debug, Clone, Default)]
pub struct Signature {
    /// Identifier of the signature element.
    pub id: Option<String>,
    /// DER-encoded check value (the SM2 signature).
    pub check_value: Vec<u8>,
    /// Protected files in signing order.
    pub references: Vec<Reference>,
    /// DER-encoded signing certificate, when embedded.
    pub certificate: Option<Vec<u8>>,
    /// Name of the signing provider, when present.
    pub provider: Option<String>,
    /// Signature algorithm name, when present.
    pub signature_method: Option<String>,
    /// Signing time as written, when present.
    pub sign_time: Option<String>,
}

/// Read the signature list file, returning one entry per `Sign` element.
pub fn read_sign_list(package: &mut Package, loc: &str) -> Result<Vec<SignRef>, OfdError> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut signs = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(node) if node.is("Sign") => {
                let base = node.attributes().get("BaseLoc").unwrap_or_default();
                signs.push(SignRef {
                    base_loc: base.to_string(),
                });
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(signs)
}

/// Read one signature descriptor file.
pub fn read_signature(package: &mut Package, base_loc: &str) -> Result<Signature, OfdError> {
    let xml = package.read_string(base_loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut signature = Signature::default();
    let mut depth = 0usize;
    let mut in_check_value = false;
    let mut in_cert = false;
    let mut in_provider = false;
    let mut in_method = false;
    let mut in_time = false;
    loop {
        match cursor.next_event()? {
            Event::Start(node) => {
                depth += 1;
                if node.is("Signature") {
                    signature.id = node.attributes().get("ID").map(str::to_string);
                } else if node.is("CheckValue") {
                    in_check_value = true;
                } else if node.is("Reference") {
                    signature.references.push(Reference {
                        check_method: node
                            .attributes()
                            .get("CheckMethod")
                            .unwrap_or_default()
                            .into(),
                        file_ref: node.attributes().get("FileRef").unwrap_or_default().into(),
                    });
                } else if node.is("Cert") {
                    in_cert = true;
                } else if node.is("Provider") {
                    in_provider = true;
                } else if node.is("SignatureMethod") {
                    in_method = true;
                } else if node.is("SignTime") {
                    in_time = true;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                in_check_value = false;
                in_cert = false;
                in_provider = false;
                in_method = false;
                in_time = false;
                if depth == 0 {
                    break;
                }
            }
            Event::Eof => break,
            Event::Text(text) if depth >= 1 => {
                absorb_text(
                    text.trim(),
                    &mut signature,
                    in_check_value,
                    in_cert,
                    in_provider,
                    in_method,
                    in_time,
                );
            }
            Event::Text(_) => {}
        }
    }
    Ok(signature)
}

fn absorb_text(
    text: &str,
    signature: &mut Signature,
    check_value: bool,
    cert: bool,
    provider: bool,
    method: bool,
    time: bool,
) {
    if text.is_empty() {
        return;
    }
    if check_value {
        if let Some(bytes) = base64_decode(text) {
            signature.check_value = bytes;
        }
    } else if cert {
        if let Some(bytes) = base64_decode(text) {
            signature.certificate = Some(bytes);
        }
    } else if provider {
        signature.provider = Some(text.to_string());
    } else if method {
        signature.signature_method = Some(text.to_string());
    } else if time {
        signature.sign_time = Some(text.to_string());
    }
}

/// Verify a signature: recompute the digest over all referenced files and
/// check the SM2 check value against the embedded certificate.
pub fn verify_signature(package: &mut Package, sig: &Signature) -> Result<VerifyStatus, OfdError> {
    let cert = sig.certificate.as_ref().ok_or_else(|| {
        OfdError::Signature(SignatureError::CertificateChain(
            "signature carries no certificate".into(),
        ))
    })?;
    Certificate::parse_der(cert).map_err(|e| {
        OfdError::Signature(SignatureError::UnsupportedFormat(format!(
            "embedded certificate unreadable: {e}"
        )))
    })?;
    verify_with_public_key(package, sig, cert)
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

/// Compute the digest that is signed: the SM3 digest of the concatenated
/// per-file SM3 digests, in reference order.
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
        combined.extend_from_slice(&sm3(&data));
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
