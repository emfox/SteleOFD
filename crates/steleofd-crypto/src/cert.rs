//! X.509 certificate parsing for signature verification support.
//!
//! What is kept is what a certification path needs: the name a certificate
//! presents and the name it was issued under, the key identifiers that tie one
//! link to the next, the two extensions that decide whether a link may sign
//! others, and the algorithm its own signature uses. GM/T 0015-2023 5.2.4.1
//! makes the first of those mandatory reading for a verifying application and
//! 4) makes the subset below the one that has to be understood.

use steleofd_error::{CryptoError, OfdError};
use x509_parser::extensions::ParsedExtension;
use x509_parser::prelude::{FromDer, X509Certificate};

/// A parsed X.509 certificate kept together with its raw DER encoding.
#[derive(Debug, Clone)]
pub struct Certificate {
    raw_der: Vec<u8>,
    subject: String,
    issuer: String,
    serial: String,
    not_before: i64,
    not_after: i64,
    public_key: Vec<u8>,
    signature_algorithm: String,
    subject_key_id: Option<Vec<u8>>,
    authority_key_id: Option<Vec<u8>>,
    is_ca: bool,
    path_len: Option<u32>,
    key_usage: Option<KeyUsage>,
}

/// The bits GM/T 0015-2023 5.2.4.4 names, in the order the ASN.1 gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyUsage {
    /// `digitalSignature` (0), a signature over anything but a certificate or CRL.
    pub digital_signature: bool,
    /// `nonRepudiation` (1), a signature that supports non-repudiation.
    pub non_repudiation: bool,
    /// `keyEncipherment` (2).
    pub key_encipherment: bool,
    /// `dataEncipherment` (3).
    pub data_encipherment: bool,
    /// `keyAgreement` (4).
    pub key_agreement: bool,
    /// `keyCertSign` (5), a signature over a certificate.
    pub key_cert_sign: bool,
    /// `cRLSign` (6), a signature over a revocation list.
    pub crl_sign: bool,
    /// `encipherOnly` (7), meaningful only beside `keyAgreement`.
    pub encipher_only: bool,
    /// `decipherOnly` (8), meaningful only beside `keyAgreement`.
    pub decipher_only: bool,
}

impl KeyUsage {
    /// Whether the key may sign a document or a certificate.
    ///
    /// 0015 5.2.4.4 b) makes `nonRepudiation` the bit for signatures that
    /// cannot be denied, which is what a signed document is, so a certificate
    /// carrying either bit may sign one. A certificate carrying neither is an
    /// encryption certificate: 0015 5.2.4.4 recommends the two appear as a
    /// pair rather than as one certificate used for both.
    pub fn may_sign(&self) -> bool {
        self.digital_signature || self.non_repudiation
    }

    /// Whether the key may sign another certificate or a revocation list.
    pub fn may_certify(&self) -> bool {
        self.key_cert_sign
    }
}

impl Certificate {
    /// Parse a DER-encoded certificate.
    pub fn parse_der(der: &[u8]) -> Result<Self, OfdError> {
        let (_, cert) = X509Certificate::from_der(der).map_err(|e| {
            OfdError::Crypto(CryptoError::Operation(format!(
                "certificate parse failed: {e}"
            )))
        })?;
        Ok(Self {
            raw_der: der.to_vec(),
            subject: cert.subject().to_string(),
            issuer: cert.issuer().to_string(),
            serial: hex_of(cert.tbs_certificate.serial.to_bytes_be()),
            not_before: cert.validity().not_before.timestamp(),
            not_after: cert.validity().not_after.timestamp(),
            public_key: cert.public_key().subject_public_key.data.to_vec(),
            signature_algorithm: cert.signature_algorithm.algorithm.to_id_string(),
            subject_key_id: extension(&cert, |e| match e {
                ParsedExtension::SubjectKeyIdentifier(id) => Some(id.0.to_vec()),
                _ => None,
            }),
            authority_key_id: extension(&cert, |e| match e {
                ParsedExtension::AuthorityKeyIdentifier(aki) => {
                    aki.key_identifier.as_ref().map(|k| k.0.to_vec())
                }
                _ => None,
            }),
            is_ca: extension(&cert, |e| match e {
                ParsedExtension::BasicConstraints(bc) => Some(bc.ca),
                _ => None,
            })
            .unwrap_or(false),
            path_len: extension(&cert, |e| match e {
                ParsedExtension::BasicConstraints(bc) => bc.path_len_constraint,
                _ => None,
            }),
            key_usage: extension(&cert, |e| match e {
                ParsedExtension::KeyUsage(ku) => Some(KeyUsage {
                    digital_signature: ku.digital_signature(),
                    non_repudiation: ku.non_repudiation(),
                    key_encipherment: ku.key_encipherment(),
                    data_encipherment: ku.data_encipherment(),
                    key_agreement: ku.key_agreement(),
                    key_cert_sign: ku.key_cert_sign(),
                    crl_sign: ku.crl_sign(),
                    encipher_only: ku.encipher_only(),
                    decipher_only: ku.decipher_only(),
                }),
                _ => None,
            }),
        })
    }

    /// Parse a PEM-encoded certificate (the first BEGIN CERTIFICATE block).
    pub fn parse_pem(pem: &str) -> Result<Self, OfdError> {
        let text = strip_pem_armor(pem)?;
        let der = base64_decode(text)?;
        Self::parse_der(&der)
    }

    /// The raw DER bytes.
    pub fn raw_der(&self) -> &[u8] {
        &self.raw_der
    }

    /// Subject public key bytes, exactly as the certificate encodes them:
    /// for an SM2 certificate, the uncompressed EC point.
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Distinguished name of the subject.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Distinguished name of the issuer.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Serial number as lowercase hex.
    pub fn serial_hex(&self) -> &str {
        &self.serial
    }

    /// Not-before time as a Unix timestamp.
    pub fn not_before(&self) -> i64 {
        self.not_before
    }

    /// Not-after time as a Unix timestamp.
    pub fn not_after(&self) -> i64 {
        self.not_after
    }

    /// Whether the certificate is expired at the given Unix timestamp.
    pub fn is_expired_at(&self, unix_seconds: i64) -> bool {
        unix_seconds < self.not_before || unix_seconds >= self.not_after
    }

    /// The Object Identifier of the algorithm this certificate's own signature
    /// uses, which is not the algorithm of the key it carries.
    ///
    /// A path is walked by verifying each link with the algorithm that link
    /// names, so the identifier and not an assumption about it is what the
    /// verifier needs.
    pub fn signature_algorithm(&self) -> &str {
        &self.signature_algorithm
    }

    /// The subject key identifier extension of GM/T 0015-2023 5.2.4.3.
    pub fn subject_key_id(&self) -> Option<&[u8]> {
        self.subject_key_id.as_deref()
    }

    /// The authority key identifier extension of GM/T 0015-2023 5.2.4.2,
    /// which names the key that signed this certificate.
    pub fn authority_key_id(&self) -> Option<&[u8]> {
        self.authority_key_id.as_deref()
    }

    /// Whether `basicConstraints` marks this certificate as a CA.
    ///
    /// The extension's `cA` field defaults to false and 0015 5.2.4.12 only
    /// says a CA certificate *should* carry it, so a certificate without the
    /// extension is an end entity. That is also the safe reading: trusting an
    /// absent field would make every leaf a possible issuer.
    pub fn is_ca(&self) -> bool {
        self.is_ca
    }

    /// The `pathLenConstraint` of `basicConstraints`, when the certificate is
    /// a CA and states a limit.
    pub fn path_len(&self) -> Option<u32> {
        self.path_len
    }

    /// The `keyUsage` extension, or `None` when the certificate omits it.
    ///
    /// 0015 5.2.4.4 requires a CA certificate to carry the extension, and
    /// 7.3 e) of GB/T 38540-2020 makes the key usage part of what a verifier
    /// checks, so an absent extension is a fact to report rather than a
    /// default to invent.
    pub fn key_usage(&self) -> Option<KeyUsage> {
        self.key_usage
    }

    /// Whether the certificate was issued by itself.
    ///
    /// A name match is not enough on its own: 0015 5.2.3.6 lets the subject be
    /// empty when the name lives in `subjectAltName` instead, and says an
    /// empty-subject CA must not repeat the issuer's name there. Two empty
    /// strings are therefore not evidence that a certificate signed itself.
    pub fn is_self_signed(&self) -> bool {
        !self.subject.is_empty() && self.subject == self.issuer
    }

}

/// Read one extension by the value it parses to, or `None` when the
/// certificate omits it. A certificate is allowed to carry an extension this
/// build cannot parse; that is not an error, it just contributes nothing.
fn extension<T>(
    cert: &X509Certificate<'_>,
    read: impl Fn(&ParsedExtension<'_>) -> Option<T>,
) -> Option<T> {
    cert.extensions()
        .iter()
        .find_map(|ext| read(ext.parsed_extension()))
}

fn strip_pem_armor(pem: &str) -> Result<&str, OfdError> {
    let start = pem.find("-----BEGIN CERTIFICATE-----").ok_or_else(|| {
        OfdError::Crypto(CryptoError::Operation("no PEM certificate found".into()))
    })? + "-----BEGIN CERTIFICATE-----".len();
    let body = &pem[start..];
    let end = body.find("-----END CERTIFICATE-----").ok_or_else(|| {
        OfdError::Crypto(CryptoError::Operation(
            "unterminated PEM certificate".into(),
        ))
    })?;
    Ok(body[..end].trim())
}

fn base64_decode(text: &str) -> Result<Vec<u8>, OfdError> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for ch in text.bytes() {
        if ch == b'=' || ch.is_ascii_whitespace() {
            continue;
        }
        let v = TABLE.iter().position(|c| *c == ch).ok_or_else(|| {
            OfdError::Crypto(CryptoError::Operation("invalid base64 in PEM".into()))
        })? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn hex_of(bytes: Vec<u8>) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
