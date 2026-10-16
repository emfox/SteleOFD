//! Timestamp tokens (GB/T 20520, the profile GM/T 0099-2020 clause 7.2.5
//! points an OFD signature timestamp at): a CMS `SignedData` wrapping a
//! `TSTInfo` the TSA signs with SM2 over SM3.

use der::asn1::{AnyRef, ContextSpecific, Int, ObjectIdentifier, OctetString, SetOfVec};
use der::{Encode, Reader, SliceReader, Tag, TagNumber};
use steleofd_error::{CryptoError, OfdError};

/// The content type a timestamp token carries, `id-ct-TSTInfo`.
pub const TST_INFO_OID: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.1.4");

/// The OID of the SM3 hash algorithm, as a token's imprint names it.
pub const SM3_OID: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.156.10197.1.401");

/// A parsed timestamp token: the TSA's signed time, the imprint it binds, and
/// the bytes and signature a verifier needs to check the TSA's own proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeStampToken {
    /// Version of the `TSTInfo`, as the token writes it.
    pub version: u32,
    /// Security policy identifier the TSA applied.
    pub policy: String,
    /// The trusted time, in `GeneralizedTime` form, exactly as it was signed.
    pub gen_time: String,
    /// Serial number the TSA assigned; unique per TSA.
    pub serial_number: String,
    /// OID of the hash algorithm the imprint was made with, as written.
    pub imprint_algorithm: String,
    /// The hashed value the TSA timestamped: the digest of the signature.
    pub imprint: Vec<u8>,
    /// The TSA's signature over `tst_info_der`, DER encoded.
    pub signature: Vec<u8>,
    /// The raw DER of the `TSTInfo` that was signed.
    pub tst_info_der: Vec<u8>,
    /// DER of the `signedAttrs` SET when the token carries one, which is what
    /// the signature covers in that case; `None` means the TSTInfo is signed.
    pub signed_attrs: Option<Vec<u8>>,
    /// DER of the TSA certificate, when the token embeds one.
    pub certificate: Option<Vec<u8>>,
}

impl TimeStampToken {
    /// Parse a DER-encoded timestamp token.
    pub fn parse_der(der: &[u8]) -> Result<Self, OfdError> {
        // ContentInfo is the outermost element; take it whole and read inside.
        let mut reader = slice(der)?;
        let content_info = reader.tlv_bytes().map_err(asn1)?.to_vec();
        parse_content_info(&content_info)
    }

    /// Whether the token carries both the signature and the bytes it covers,
    /// which is the least a verifier can work with.
    pub fn is_complete(&self) -> bool {
        !self.signature.is_empty() && !self.tst_info_der.is_empty()
    }

    /// The data the TSA actually signed. RFC 5652 says that when `signedAttrs`
    /// is present the signature covers its re-encoded SET, and otherwise the
    /// eContent directly, so this returns whichever the token carries.
    #[must_use]
    pub fn signed_data(&self) -> &[u8] {
        match &self.signed_attrs {
            Some(attrs) => attrs,
            None => &self.tst_info_der,
        }
    }

    /// Check the TSA's own SM2 signature over the token, using the public key
    /// of the certificate the token embeds, or one the caller supplies.
    pub fn verify_tsa_signature(&self, public_key: &[u8]) -> Result<bool, OfdError> {
        if !self.is_complete() {
            return Err(OfdError::Crypto(CryptoError::Operation(
                "timestamp token carries no signature".into(),
            )));
        }
        // GM/T 0099-2020 clause 7.2.5 sends a timestamp through GB/T 20520,
        // which signs with SM2 over SM3; a token naming any other digest is
        // one this build will not pretend to check.
        if !self.imprint_algorithm.is_empty() && self.imprint_algorithm != SM3_OID.to_string() {
            return Err(OfdError::Crypto(CryptoError::UnsupportedAlgorithm(
                format!("timestamp imprint uses {}", self.imprint_algorithm),
            )));
        }
        crate::asymmetric::sm2_verify(public_key, self.signed_data(), &self.signature)
    }

    /// Whether the token timestamps exactly this value: the imprint must be the
    /// SM3 digest of the bytes handed in, which is how a timestamp binds to the
    /// signature it countersigns (GB/T 38540-2020 clause 7.2).
    #[must_use]
    pub fn covers(&self, message: &[u8]) -> bool {
        self.imprint.len() == crate::digest::SM3_DIGEST_LEN
            && self.imprint.as_slice() == crate::digest::sm3(message)
    }
}

/// `ContentInfo ::= SEQUENCE { contentType OID, content [0] EXPLICIT ANY }`.
fn parse_content_info(content_info: &[u8]) -> Result<TimeStampToken, OfdError> {
    let mut reader = slice(content_info)?;
    let (content_type, content) = reader
        .sequence(|inner| {
            let oid: ObjectIdentifier = inner.decode()?;
            let explicit: ContextSpecific<AnyRef<'_>> = inner.decode()?;
            Ok((oid, explicit.value.to_der()?))
        })
        .map_err(asn1)?;
    if content_type != TST_INFO_OID {
        return Err(OfdError::Crypto(CryptoError::UnsupportedAlgorithm(
            format!("timestamp content type {content_type} is not TSTInfo"),
        )));
    }
    parse_signed_data(&content)
}

/// `SignedData ::= SEQUENCE { version, digestAlgorithms, encapContentInfo,
/// certificates OPTIONAL, crls OPTIONAL, signerInfos }`.
fn parse_signed_data(der: &[u8]) -> Result<TimeStampToken, OfdError> {
    let mut reader = slice(der)?;
    reader
        .sequence(|inner| {
            let _version: u32 = inner.decode()?;
            let _digest_algorithms: SetOfVec<AnyRef<'_>> = inner.decode()?;
            let tst_info_der = read_encapsulated_content(inner)?;
            let certificate = read_certificates(inner)?;
            skip_crls(inner)?;
            let (signature, signed_attrs) = read_signer_info(inner)?;
            let mut token = parse_tst_info(&tst_info_der)?;
            token.signature = signature;
            token.certificate = certificate;
            token.tst_info_der = tst_info_der;
            token.signed_attrs = signed_attrs;
            Ok(token)
        })
        .map_err(asn1)
}

/// `EncapsulatedContentInfo ::= SEQUENCE { eContentType OID, eContent [0]
/// EXPLICIT OCTET STRING OPTIONAL }`. The octet string wraps the real TSTInfo.
fn read_encapsulated_content(reader: &mut SliceReader<'_>) -> der::Result<Vec<u8>> {
    reader.sequence(|inner| {
        let _oid: ObjectIdentifier = inner.decode()?;
        let explicit: ContextSpecific<OctetString> = inner.decode()?;
        Ok(explicit.value.as_bytes().to_vec())
    })
}

/// Certificates are `[0] IMPLICIT`; each contained element is read back as DER
/// and the first one kept, which is the signer's by convention.
fn read_certificates(reader: &mut SliceReader<'_>) -> der::Result<Option<Vec<u8>>> {
    if Tag::peek(reader)? != implicit_tag(0) {
        return Ok(None);
    }
    let explicit: ContextSpecific<AnyRef<'_>> = reader.decode()?;
    let bytes = explicit.value.to_der()?;
    let mut inner = SliceReader::new(&bytes)?;
    let mut first = None;
    while !inner.is_finished() {
        let any: AnyRef<'_> = inner.decode()?;
        if first.is_none() {
            first = Some(any.to_der()?);
        }
    }
    Ok(first)
}

fn skip_crls(reader: &mut SliceReader<'_>) -> der::Result<()> {
    if Tag::peek(reader)? == implicit_tag(1) {
        let _: ContextSpecific<AnyRef<'_>> = reader.decode()?;
    }
    Ok(())
}

/// `SignerInfos ::= SET OF SignerInfo`; the token carries one.
fn read_signer_info(reader: &mut SliceReader<'_>) -> der::Result<(Vec<u8>, Option<Vec<u8>>)> {
    let infos: SetOfVec<AnyRef<'_>> = reader.decode()?;
    let first = infos.iter().next().ok_or_else(empty_error)?;
    let bytes = first.to_der()?;
    read_signature_value(&bytes)
}

/// `SignerInfo ::= SEQUENCE { version, sid, digestAlgorithm, signedAttrs
/// OPTIONAL, signatureAlgorithm, signature OCTET STRING }`. The field order is
/// fixed by RFC 5652, so the signature is reached by walking the elements.
fn read_signature_value(der: &[u8]) -> der::Result<(Vec<u8>, Option<Vec<u8>>)> {
    let mut reader = SliceReader::new(der)?;
    reader.sequence(|inner| {
        let _version: u32 = inner.decode()?;
        let _sid: AnyRef<'_> = inner.decode()?;
        let _digest_algorithm: AnyRef<'_> = inner.decode()?;
        // signedAttrs is the optional `[0] IMPLICIT` field, and what follows
        // it is a SEQUENCE, so a tag check tells the two apart. When present,
        // the signature covers its re-encoded SET form, not the TSTInfo.
        let attrs = if Tag::peek(inner)? == implicit_tag(0) {
            let raw = inner.tlv_bytes()?.to_vec();
            Some(as_set_der(&raw)?)
        } else {
            None
        };
        let _signature_algorithm: AnyRef<'_> = inner.decode()?;
        let signature: OctetString = inner.decode()?;
        Ok((signature.as_bytes().to_vec(), attrs))
    })
}

/// `signedAttrs` is written `[0] IMPLICIT`, but the signature covers the same
/// bytes retagged as a universal SET, so the two differ by exactly one octet.
fn as_set_der(implicit: &[u8]) -> der::Result<Vec<u8>> {
    let (header, body) = implicit.split_first().ok_or_else(empty_error)?;
    // 0xA0 is `[0] constructed`; 0x31 is a constructed SET.
    if *header != 0xA0 {
        return Err(empty_error());
    }
    let mut out = vec![0x31u8];
    out.extend_from_slice(body);
    Ok(out)
}

/// `TSTInfo ::= SEQUENCE { version, policy, messageImprint, serialNumber,
/// genTime, accuracy OPTIONAL, ... }`; the optional tail is not needed.
fn parse_tst_info(der: &[u8]) -> der::Result<TimeStampToken> {
    let mut reader = SliceReader::new(der)?;
    reader.sequence(|inner| {
        let version: u32 = inner.decode()?;
        let policy: ObjectIdentifier = inner.decode()?;
        let message_imprint: AnyRef<'_> = inner.decode()?;
        // Serial and time are kept as written rather than re-encoded, because
        // an archival tool should show the token's own bytes, not its idea of
        // the same value. The serial is an integer, so it reads as decimal.
        let serial_number: Int = inner.decode()?;
        let gen_time: AnyRef<'_> = inner.decode()?;
        let (imprint_algorithm, imprint) = read_imprint(&message_imprint)?;
        Ok(TimeStampToken {
            version,
            policy: policy.to_string(),
            gen_time: node_text(&gen_time),
            serial_number: int_text(&serial_number),
            imprint_algorithm,
            imprint,
            signature: Vec::new(),
            tst_info_der: Vec::new(),
            signed_attrs: None,
            certificate: None,
        })
    })
}

/// `MessageImprint ::= SEQUENCE { hashAlgorithm AlgorithmIdentifier,
/// hashedMessage OCTET STRING }`.
fn read_imprint(node: &AnyRef<'_>) -> der::Result<(String, Vec<u8>)> {
    let bytes = node.to_der()?;
    let mut reader = SliceReader::new(&bytes)?;
    reader.sequence(|inner| {
        let algorithm: AnyRef<'_> = inner.decode()?;
        let hashed: OctetString = inner.decode()?;
        Ok((algorithm_oid(&algorithm), hashed.as_bytes().to_vec()))
    })
}

/// The OID inside an `AlgorithmIdentifier`, as dotted arcs.
fn algorithm_oid(node: &AnyRef<'_>) -> String {
    let Ok(bytes) = node.to_der() else {
        return String::new();
    };
    let Ok(mut reader) = SliceReader::new(&bytes) else {
        return String::new();
    };
    reader
        .sequence(|inner| {
            let oid: ObjectIdentifier = inner.decode()?;
            Ok::<_, der::Error>(oid.to_string())
        })
        .unwrap_or_default()
}

/// A primitive node's content as text, so a serial number or a time is shown
/// the way the token wrote it rather than re-encoded through a calendar.
fn node_text(node: &AnyRef<'_>) -> String {
    let Ok(bytes) = node.to_der() else {
        return String::new();
    };
    let Some(content) = tlv_content(&bytes) else {
        return String::new();
    };
    String::from_utf8_lossy(content).into_owned()
}

/// An integer's value in decimal, so a serial number reads the way a person
/// would write it rather than as a string of hex digits.
fn int_text(value: &Int) -> String {
    let mut text = String::new();
    let mut carry = value.as_bytes().to_vec();
    if carry.is_empty() {
        return text;
    }
    // Long division by ten, one digit at a time, is plenty for a serial.
    while carry.iter().any(|b| *b != 0) {
        let mut remainder = 0u32;
        for byte in &mut carry {
            let current = remainder * 256 + u32::from(*byte);
            *byte = (current / 10) as u8;
            remainder = current % 10;
        }
        text.insert(0, char::from(b'0' + remainder as u8));
    }
    if text.is_empty() {
        text.push('0');
    }
    text
}

/// The bytes after a TLV header, which for a primitive node is its value. The
/// header is one tag octet plus a length of one to five octets.
fn tlv_content(bytes: &[u8]) -> Option<&[u8]> {
    let (&len_byte, rest) = bytes.get(1..)?.split_first()?;
    if len_byte & 0x80 == 0 {
        return rest.get(..len_byte as usize);
    }
    let count = (len_byte & 0x7f) as usize;
    if count == 0 || count > 4 {
        return None;
    }
    rest.get(count..)
}

fn slice(der: &[u8]) -> Result<SliceReader<'_>, OfdError> {
    SliceReader::new(der).map_err(asn1)
}

/// A `[n] IMPLICIT` tag, which frames `signedAttrs`, `certificates` and `crls`.
/// Only where two tags could collide inside one SEQUENCE does the tag decide.
fn implicit_tag(number: u32) -> Tag {
    Tag::ContextSpecific {
        constructed: true,
        number: TagNumber(number),
    }
}

/// A missing optional field, reported as an incomplete read.
fn empty_error() -> der::Error {
    der::Error::from(der::ErrorKind::Incomplete {
        expected_len: der::Length::ONE,
        actual_len: der::Length::ZERO,
    })
}

fn asn1(e: der::Error) -> OfdError {
    OfdError::Crypto(CryptoError::Operation(format!(
        "timestamp parse failed: {e}"
    )))
}
