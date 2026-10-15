//! X.509 certificate parsing for signature verification support.

use steleofd_error::{CryptoError, OfdError};
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

    /// Whether the certificate was issued by itself.
    pub fn is_self_signed(&self) -> bool {
        self.subject == self.issuer
    }
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
