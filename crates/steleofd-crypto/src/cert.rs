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

/// The signature algorithm GM/T 0015-2023 certificates carry: SM2 over SM3.
/// Objects that mean "SM2 with SM3" rather than "SM2" carry it, so a
/// certificate naming it is the one a path verifier expects to walk.
pub const SM2_WITH_SM3: &str = "1.2.156.10197.1.501";

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
    tbs_range: std::ops::Range<usize>,
    signature: Vec<u8>,
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
    ///
    /// The signature covers the `tbsCertificate` as it appears in the encoding,
    /// so its offset and length are recorded here rather than re-encoded: a
    /// re-encoding round trip is only byte-identical for the subset of DER this
    /// build happens to produce, not for what a producer sent.
    pub fn parse_der(der: &[u8]) -> Result<Self, OfdError> {
        let (rest, cert) = X509Certificate::from_der(der).map_err(|e| {
            OfdError::Crypto(CryptoError::Operation(format!(
                "certificate parse failed: {e}"
            )))
        })?;
        let _ = &rest;
        // `from_der` parses from the front and hands back the tail it did not
        // read, so the encoding of interest starts at `begin` and the parsed
        // nodes borrow that same buffer: locating the TBS means counting
        // forward from its start rather than asking a node for its address.
        let begin = 0;
        let tbs_range = tbs_range(der, begin).ok_or_else(|| {
            OfdError::Crypto(CryptoError::Operation(
                "certificate TBS offset is not within its encoding".into(),
            ))
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
            tbs_range,
            signature: cert.signature_value.data.to_vec(),
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

    /// Whether `issuer_key` signed this certificate.
    ///
    /// GB/T 38540-2020 7.3 e) makes a link's own signature part of what a
    /// verifier checks, and each link names the algorithm it was signed with:
    /// for a chain built under GM/T 0015-2023 that is SM2 over SM3. A
    /// certificate naming any other algorithm is one this build will not
    /// pretend to check, so it reports an error rather than `false`.
    pub fn verify_signature(&self, issuer_key: &[u8]) -> Result<bool, OfdError> {
        if self.signature_algorithm != SM2_WITH_SM3 {
            return Err(OfdError::Crypto(CryptoError::UnsupportedAlgorithm(format!(
                "certificate signature uses {}",
                self.signature_algorithm
            ))));
        }
        let tbs = &self.raw_der[self.tbs_range.clone()];
        crate::asymmetric::sm2_verify(issuer_key, tbs, &self.signature)
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

/// The range of the `tbsCertificate` inside the certificate that starts at
/// `start` and is `len` bytes long.
///
/// A DER TLV is a tag, a length and that many bytes of content, and the first
/// child of the outer SEQUENCE follows its header directly. Only the definite
/// lengths DER allows are walked; anything else is not DER.
fn tbs_range(der: &[u8], start: usize) -> Option<std::ops::Range<usize>> {
    let outer = tlv(der, start)?;
    let tbs = tlv(der, outer.content_at)?;
    let end = tbs.content_at + tbs.content_len;
    (tbs.total <= outer.content_len).then_some(outer.content_at..end)
}

/// A TLV's content position and length, and its total encoded length.
struct Tlv {
    content_at: usize,
    content_len: usize,
    total: usize,
}

/// Read a TLV header at `at`, checking only that the encoding is definite.
fn tlv(der: &[u8], at: usize) -> Option<Tlv> {
    if *der.get(at)? & 0x1F == 0x1F {
        return None;
    }
    let (content_at, content_len) = match *der.get(at + 1)? {
        l if l < 0x80 => (at + 2, l as usize),
        0x81 => (at + 3, usize::from(*der.get(at + 2)?)),
        0x82 => {
            let hi = usize::from(*der.get(at + 2)?);
            let lo = usize::from(*der.get(at + 3)?);
            (at + 4, hi << 8 | lo)
        }
        _ => return None,
    };
    Some(Tlv {
        content_at,
        content_len,
        total: content_at + content_len - at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tbs_range_walks_an_outer_sequence() {
        // SEQUENCE(5) { SEQUENCE(3) { 02 01 00 } }: the inner TLV opens at 2.
        let der = [0x30, 0x05, 0x30, 0x03, 0x02, 0x01, 0x00];
        assert_eq!(tbs_range(&der, 0), Some(2..7));
    }

    #[test]
    fn tbs_range_follows_a_long_form_length() {
        // 30 81 83: a 0x83-byte body, whose own first child is 30 81 80.
        let mut der = vec![0x30, 0x81, 0x83, 0x30, 0x81, 0x80];
        der.extend(std::iter::repeat_n(0u8, 0x80));
        assert_eq!(tbs_range(&der, 0), Some(3..der.len()));
    }
}