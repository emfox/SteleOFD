//! A certificate whose subject, issuer, extensions and signature a test sets.
//!
//! `sm2_cert_der` builds the smallest certificate a parser accepts: an empty
//! name, no extensions and an empty signature value. A path check reads the
//! fields that certificate leaves out, so the tests that exercise one need a
//! certificate that actually carries them, signed by a key the test holds.

use crate::der::{oid, tlv};
use steleofd_crypto::Sm2KeyPair;

/// One link of a test chain, before it is encoded.
#[derive(Debug, Clone)]
pub struct CertSpec {
    /// Subject name, as a printable string. Empty is allowed and means the
    /// name lives elsewhere, which GM/T 0015-2023 5.2.3.6 permits.
    pub subject: String,
    /// Issuer name. Equal to `subject` makes it self-issued.
    pub issuer: String,
    /// Serial number.
    pub serial: u32,
    /// Not-before, UTCTime `YYMMDDHHMMSSZ`: two digits of year, as RFC 5280
    /// requires before 2050.
    pub not_before: String,
    /// Not-after, UTCTime `YYMMDDHHMMSSZ`.
    pub not_after: String,
    /// Subject key identifier, normally the SM3 truncation of the key.
    pub subject_key_id: Option<Vec<u8>>,
    /// Authority key identifier: the subject key identifier of the signer.
    pub authority_key_id: Option<Vec<u8>>,
    /// `basicConstraints`. `None` omits the extension entirely, which is what
    /// an end entity looks like when the producer skips it.
    pub basic_constraints: Option<BasicConstraints>,
    /// `keyUsage`. `None` omits the extension.
    pub key_usage: Option<KeyUsageBits>,
}

/// The `basicConstraints` extension of GM/T 0015-2023 5.2.4.12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicConstraints {
    /// Whether the key may verify certificates.
    pub ca: bool,
    /// How many intermediate CA certificates may follow.
    pub path_len: Option<u32>,
}

/// The `keyUsage` bits of GM/T 0015-2023 5.2.4.4, in ASN.1 order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyUsageBits {
    /// `digitalSignature` (0).
    pub digital_signature: bool,
    /// `nonRepudiation` (1).
    pub non_repudiation: bool,
    /// `keyEncipherment` (2).
    pub key_encipherment: bool,
    /// `keyCertSign` (5).
    pub key_cert_sign: bool,
    /// `cRLSign` (6).
    pub crl_sign: bool,
}

impl KeyUsageBits {
    /// What a signing certificate carries: 0015 5.2.4.4's signature pair.
    pub fn for_signing() -> Self {
        Self {
            digital_signature: true,
            non_repudiation: true,
            ..Self::default()
        }
    }

    /// What a CA certificate carries: `keyCertSign` and `cRLSign`, which
    /// 5.2.4.4 requires of every CA certificate.
    pub fn for_ca() -> Self {
        Self {
            key_cert_sign: true,
            crl_sign: true,
            ..Self::default()
        }
    }
}

impl CertSpec {
    /// An end-entity certificate that may sign documents.
    pub fn end_entity(subject: &str, issuer: &str, serial: u32) -> Self {
        Self {
            subject: subject.to_string(),
            issuer: issuer.to_string(),
            serial,
            not_before: "260101000000Z".to_string(),
            not_after: "491231235959Z".to_string(),
            subject_key_id: None,
            authority_key_id: None,
            basic_constraints: Some(BasicConstraints {
                ca: false,
                path_len: None,
            }),
            key_usage: Some(KeyUsageBits::for_signing()),
        }
    }

    /// A CA certificate with no limit on the depth below it.
    pub fn ca(subject: &str, issuer: &str, serial: u32) -> Self {
        Self {
            subject: subject.to_string(),
            issuer: issuer.to_string(),
            serial,
            not_before: "260101000000Z".to_string(),
            not_after: "491231235959Z".to_string(),
            subject_key_id: None,
            authority_key_id: None,
            basic_constraints: Some(BasicConstraints {
                ca: true,
                path_len: None,
            }),
            key_usage: Some(KeyUsageBits::for_ca()),
        }
    }

    /// The same specification with a validity window.
    pub fn between(mut self, not_before: &str, not_after: &str) -> Self {
        self.not_before = not_before.to_string();
        self.not_after = not_after.to_string();
        self
    }
}

/// Encode one certificate, signed by `signer`. A self-issued certificate is
/// signed by its own key.
pub fn cert_der(spec: &CertSpec, public_key: &[u8], signer: &Sm2KeyPair) -> Vec<u8> {
    let sm2_with_sm3 = oid(&[1, 2, 156, 10197, 1, 501]);
    let algorithm = tlv(0x30, &sm2_with_sm3);
    // Name ::= SEQUENCE OF RelativeDistinguishedName, and a relative name is
    // itself a SET OF AttributeTypeAndValue. Encoding the SET where the
    // SEQUENCE belongs leaves a parser looking for the next TBS field one tag
    // too early, which surfaces as a complaint about UTCTime rather than about
    // the name.
    let name = |value: &str| {
        if value.is_empty() {
            tlv(0x30, &[])
        } else {
            let attribute = tlv(
                0x30,
                &[oid(&[2, 5, 4, 3]), tlv(0x13, value.as_bytes())].concat(),
            );
            tlv(0x30, &tlv(0x31, &attribute))
        }
    };
    let validity = tlv(
        0x30,
        &[
            tlv(0x17, spec.not_before.as_bytes()),
            tlv(0x17, spec.not_after.as_bytes()),
        ]
        .concat(),
    );
    let mut point = vec![0x00];
    point.extend_from_slice(public_key);
    let ec_public_key = oid(&[1, 2, 840, 10045, 2, 1]);
    let sm2_curve = oid(&[1, 2, 156, 10197, 1, 301]);
    let key_algorithm = tlv(0x30, &[ec_public_key, sm2_curve].concat());
    let spki = tlv(0x30, &[key_algorithm, tlv(0x03, &point)].concat());

    let mut tbs = tlv(0xA0, &tlv(0x02, &[0x02]));
    tbs.extend(tlv(0x02, &serial_bytes(spec.serial)));
    tbs.extend(&algorithm);
    tbs.extend(name(&spec.issuer));
    tbs.extend(&validity);
    tbs.extend(name(&spec.subject));
    tbs.extend(&spki);
    tbs.extend(extensions(spec));

    // The signature covers the whole `tbsCertificate`, SEQUENCE header
    // included, which is why the header is built before signing rather than
    // wrapped around a signed body afterwards.
    let tbs = tlv(0x30, &tbs);
    let signature = signer.sign(&tbs);
    let mut body = tbs;
    body.extend(&algorithm);
    body.extend(tlv(
        0x03,
        &[0x00]
            .iter()
            .chain(signature.iter())
            .copied()
            .collect::<Vec<_>>(),
    ));
    tlv(0x30, &body)
}

fn serial_bytes(serial: u32) -> Vec<u8> {
    let be = serial.to_be_bytes();
    let first = be.iter().position(|b| *b != 0).unwrap_or(be.len() - 1);
    let mut bytes = be[first..].to_vec();
    if bytes[0] & 0x80 != 0 {
        bytes.insert(0, 0);
    }
    bytes
}

fn extensions(spec: &CertSpec) -> Vec<u8> {
    let mut items = Vec::new();
    if let Some(id) = &spec.subject_key_id {
        items.push(extension(&[2, 5, 29, 14], false, tlv(0x04, id)));
    }
    if let Some(id) = &spec.authority_key_id {
        let aki = tlv(0x30, &tlv(0x80, id));
        items.push(extension(&[2, 5, 29, 35], false, aki));
    }
    if let Some(bc) = spec.basic_constraints {
        let mut content = Vec::new();
        if bc.ca {
            content.extend(tlv(0x01, &[0xFF]));
        }
        if let Some(limit) = bc.path_len {
            content.extend(tlv(0x02, &serial_bytes(limit)));
        }
        items.push(extension(&[2, 5, 29, 19], true, tlv(0x30, &content)));
    }
    if let Some(ku) = spec.key_usage {
        items.push(extension(&[2, 5, 29, 15], true, key_usage_bits(ku)));
    }
    if items.is_empty() {
        return Vec::new();
    }
    tlv(0xA3, &tlv(0x30, &items.concat()))
}

fn extension(id: &[u64], critical: bool, value: Vec<u8>) -> Vec<u8> {
    let mut content = oid(id);
    if critical {
        content.extend(tlv(0x01, &[0xFF]));
    }
    content.extend(tlv(0x04, &value));
    tlv(0x30, &content)
}

fn key_usage_bits(ku: KeyUsageBits) -> Vec<u8> {
    let bits = [
        ku.digital_signature,
        ku.non_repudiation,
        ku.key_encipherment,
        false,
        false,
        ku.key_cert_sign,
        ku.crl_sign,
    ];
    let highest = bits.iter().rposition(|b| *b).unwrap_or(0);
    let byte_len = highest / 8 + 1;
    let mut bytes = vec![0u8; byte_len];
    for (i, set) in bits.iter().enumerate() {
        if *set {
            bytes[i / 8] |= 0x80 >> (i % 8);
        }
    }
    let unused = (byte_len * 8 - highest - 1) as u8;
    let mut content = vec![unused];
    content.extend(bytes);
    tlv(0x03, &content)
}

/// A chain a test can hand to the verifier: a root, an intermediate and a
/// leaf, each signed by the key above it.
pub struct TestChain {
    /// The root's specification and encoding, which is also its own anchor.
    pub root_der: Vec<u8>,
    /// The intermediate as it appears in the document.
    pub intermediate_der: Vec<u8>,
    /// The leaf, the one that signs the document.
    pub leaf_der: Vec<u8>,
    /// The leaf's key, which signs the document.
    pub leaf_key: Sm2KeyPair,
    /// The intermediate's key, for a test that re-signs a link.
    pub intermediate_key: Sm2KeyPair,
    /// The root's key.
    pub root_key: Sm2KeyPair,
}

/// Build a three-link chain with key identifiers tying each link to the next.
pub fn chain() -> TestChain {
    let root_key = Sm2KeyPair::generate().expect("root key");
    let intermediate_key = Sm2KeyPair::generate().expect("intermediate key");
    let leaf_key = Sm2KeyPair::generate().expect("leaf key");

    let root_id = key_id(&root_key);
    let intermediate_id = key_id(&intermediate_key);
    let leaf_id = key_id(&leaf_key);

    let mut root = CertSpec::ca("Test Root", "Test Root", 1);
    root.subject_key_id = Some(root_id.clone());
    let root_der = cert_der(&root, &root_key.public_bytes(), &root_key);

    let mut intermediate = CertSpec::ca("Test Intermediate", "Test Root", 2);
    intermediate.subject_key_id = Some(intermediate_id.clone());
    intermediate.authority_key_id = Some(root_id);
    let intermediate_der = cert_der(&intermediate, &intermediate_key.public_bytes(), &root_key);

    let mut leaf = CertSpec::end_entity("Test Signer", "Test Intermediate", 3);
    leaf.subject_key_id = Some(leaf_id);
    leaf.authority_key_id = Some(intermediate_id);
    let leaf_der = cert_der(&leaf, &leaf_key.public_bytes(), &intermediate_key);

    TestChain {
        root_der,
        intermediate_der,
        leaf_der,
        leaf_key,
        intermediate_key,
        root_key,
    }
}

/// The SM3 truncation GM/T 0015-2023 5.2.4.2 a) names as a key identifier.
pub fn key_id(key: &Sm2KeyPair) -> Vec<u8> {
    steleofd_crypto::sm3(&key.public_bytes())[..20].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_crypto::Certificate;

    #[test]
    fn each_link_reads_back_the_fields_it_was_built_with() {
        let chain = chain();
        let root = Certificate::parse_der(&chain.root_der).expect("root parses");
        let intermediate =
            Certificate::parse_der(&chain.intermediate_der).expect("intermediate parses");
        let leaf = Certificate::parse_der(&chain.leaf_der).expect("leaf parses");

        assert_eq!(root.subject(), "CN=Test Root");
        assert_eq!(root.issuer(), "CN=Test Root");
        assert!(root.is_self_signed());
        assert!(root.is_ca());
        assert!(root.key_usage().expect("key usage").may_certify());

        assert_eq!(intermediate.subject(), "CN=Test Intermediate");
        assert_eq!(intermediate.issuer(), "CN=Test Root");
        assert!(!intermediate.is_self_signed());
        assert!(intermediate.is_ca());
        assert_eq!(intermediate.authority_key_id(), root.subject_key_id());

        assert_eq!(leaf.subject(), "CN=Test Signer");
        assert_eq!(leaf.issuer(), "CN=Test Intermediate");
        assert!(!leaf.is_ca());
        assert!(leaf.key_usage().expect("key usage").may_sign());
        assert_eq!(leaf.authority_key_id(), intermediate.subject_key_id());
    }

    #[test]
    fn a_link_verifies_under_the_key_above_it() {
        let chain = chain();
        let root = Certificate::parse_der(&chain.root_der).expect("root parses");
        let intermediate =
            Certificate::parse_der(&chain.intermediate_der).expect("intermediate parses");
        let leaf = Certificate::parse_der(&chain.leaf_der).expect("leaf parses");

        assert!(root
            .verify_signature(&chain.root_key.public_bytes())
            .expect("SM2 signature"));
        assert!(intermediate
            .verify_signature(&chain.root_key.public_bytes())
            .expect("SM2 signature"));
        assert!(leaf
            .verify_signature(&chain.intermediate_key.public_bytes())
            .expect("SM2 signature"));
        assert!(!leaf
            .verify_signature(&chain.root_key.public_bytes())
            .expect("SM2 signature"));
    }

    #[test]
    fn a_validity_window_reaches_the_parsed_certificate() {
        let key = Sm2KeyPair::generate().expect("key");
        let spec = CertSpec::end_entity("A", "A", 7).between("200101000000Z", "200201000000Z");
        let der = cert_der(&spec, &key.public_bytes(), &key);
        let cert = Certificate::parse_der(&der).expect("certificate parses");
        // 2020-01-01 is inside the window, 1970 and 2023 are either side.
        assert!(!cert.is_expired_at(1_577_836_800));
        assert!(cert.is_expired_at(1_000_000));
        assert!(cert.is_expired_at(1_700_000_000));
    }
}
