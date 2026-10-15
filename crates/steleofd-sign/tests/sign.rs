//! Tests for signature reading and verification on synthetic packages.

use steleofd_crypto::Sm2KeyPair;
use steleofd_error::OfdError;
use steleofd_package::Package;
use steleofd_sign::{
    Reference, Signature, VerifyStatus, read_sign_list, read_signature, signed_digest,
    verify_signature, verify_with_public_key,
};

const SIG_FILE: &str = "Signs/Sign_0.xml";

fn signs_xml() -> Vec<u8> {
    br#"<Signs><Sign BaseLoc="Signs/Sign_0.xml"/></Signs>"#.to_vec()
}

fn descriptor(id: &str, refs: &[(&str, &str)]) -> String {
    let mut xml = format!(r#"<Signature ID="{id}">"#);
    for (method, file) in refs {
        xml.push_str(&format!(
            r#"<SignedInfo><Reference CheckMethod="{method}" FileRef="{file}"/></SignedInfo>"#
        ));
    }
    xml.push_str("</Signature>");
    xml
}

fn open(entries: &[(&str, Vec<u8>)]) -> Package {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let bytes = steleofd_package::build_archive(&borrowed).expect("archive");
    Package::from_bytes(&bytes).expect("package")
}

fn make_signature(entries: &[(&str, Vec<u8>)], _refs: &[(&str, &str)]) -> (Signature, Sm2KeyPair) {
    let mut package = open(entries);
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    let pair = Sm2KeyPair::generate().expect("keygen");
    let mut signed = sig.clone();
    if let Ok(digest) = signed_digest(&mut package, &sig) {
        signed.check_value = pair.sign(&digest);
    }
    (signed, pair)
}

#[test]
fn sign_list_is_parsed() {
    let mut package = open(&[("Signs/Signs.xml", signs_xml())]);
    let signs = read_sign_list(&mut package, "Signs/Signs.xml").expect("list");
    assert_eq!(signs.len(), 1);
    assert_eq!(signs[0].base_loc, SIG_FILE);
}

#[test]
fn descriptor_fields_are_parsed() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("sig1", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
    ];
    let mut package = open(&entries);
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    assert_eq!(sig.id.as_deref(), Some("sig1"));
    assert_eq!(sig.references.len(), 1);
    assert_eq!(
        sig.references[0],
        Reference {
            check_method: "SM3".into(),
            file_ref: "Doc_0/Document.xml".into(),
        }
    );
}

#[test]
fn valid_signature_verifies() {
    let content: Vec<u8> = b"document bytes".to_vec();
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", content),
    ];
    let (sig, pair) = make_signature(&entries, &[("SM3", "Doc_0/Document.xml")]);
    let mut package = open(&entries);
    assert_eq!(
        verify_with_public_key(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Valid
    );
}

#[test]
fn tampered_content_is_detected() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"document bytes".to_vec()),
    ];
    let (sig, pair) = make_signature(&entries, &[("SM3", "Doc_0/Document.xml")]);
    let tampered = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"document BYTES".to_vec()),
    ];
    let mut package = open(&tampered);
    assert_eq!(
        verify_with_public_key(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Tampered
    );
}

#[test]
fn missing_reference_is_tampered() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
    ];
    let (sig, pair) = make_signature(&entries, &[("SM3", "Doc_0/Document.xml")]);
    let mut package = open(&entries);
    let out = verify_with_public_key(&mut package, &sig, &pair.public_bytes());
    assert!(
        matches!(out, Err(OfdError::Signature(_))),
        "missing file fails"
    );
}

#[test]
fn unsupported_check_method_is_rejected() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SHA256", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"x".to_vec()),
    ];
    let (sig, pair) = make_signature(&entries, &[("SHA256", "Doc_0/Document.xml")]);
    let mut package = open(&entries);
    let out = verify_with_public_key(&mut package, &sig, &pair.public_bytes());
    match out {
        Err(OfdError::Signature(e)) => {
            assert!(e.to_string().contains("unsupported signature format"));
        }
        other => panic!("expected unsupported format, got {other:?}"),
    }
}

#[test]
fn verify_without_certificate_fails_cleanly() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"y".to_vec()),
    ];
    let (mut sig, _pair) = make_signature(&entries, &[("SM3", "Doc_0/Document.xml")]);
    sig.certificate = None;
    let mut package = open(&entries);
    let out = verify_signature(&mut package, &sig);
    assert!(out.is_err(), "no certificate means no verification");
}

#[test]
fn digest_depends_on_reference_order() {
    let mk = |refs: &[(&str, &str)]| {
        let entries = vec![
            ("Signs/Signs.xml", signs_xml()),
            (SIG_FILE, descriptor("s", refs).into_bytes()),
            ("a.bin", vec![1u8; 32]),
            ("b.bin", vec![2u8; 32]),
        ];
        let mut package = open(&entries);
        let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
        signed_digest(&mut package, &sig).expect("digest")
    };
    let ab = mk(&[("SM3", "a.bin"), ("SM3", "b.bin")]);
    let ba = mk(&[("SM3", "b.bin"), ("SM3", "a.bin")]);
    assert_ne!(ab, ba, "order matters in the signed digest");
    assert_eq!(
        mk(&[("SM3", "a.bin"), ("SM3", "b.bin")]),
        ab,
        "digest is stable"
    );
}

#[test]
fn gb_t_33190_18_1_signature_list_points_at_sign_files() {
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
    ];
    let mut package = open(&entries);
    let signs = read_sign_list(&mut package, "Signs/Signs.xml").expect("list");
    assert_eq!(signs.len(), 1);
    assert_eq!(
        signs[0].base_loc, SIG_FILE,
        "each Sign names its descriptor file"
    );
    let sig = read_signature(&mut package, &signs[0].base_loc).expect("descriptor");
    assert_eq!(sig.references.len(), 1);
}

#[test]
fn gb_t_33190_18_2_1_digest_covers_every_reference() {
    let document = b"signed content".to_vec();
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", document.clone()),
    ];
    let mut package = open(&entries);
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    let first = signed_digest(&mut package, &sig).expect("digest");
    assert_eq!(first.len(), 32, "the SM3 digest is 32 bytes");

    let changed = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"edited content".to_vec()),
    ];
    let mut package = open(&changed);
    let second = signed_digest(&mut package, &sig).expect("digest");
    assert_ne!(
        first, second,
        "changing a referenced file changes the digest"
    );
}

/// Base64, the encoding CheckValue and Cert use on the wire.
fn base64_of(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut group = [0u8; 3];
        group[..chunk.len()].copy_from_slice(chunk);
        let bits = u32::from_be_bytes([group[0], group[1], group[2], 0]);
        out.push(TABLE[(bits >> 26) as usize & 0x3f] as char);
        out.push(TABLE[(bits >> 20) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(bits >> 14) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(bits >> 8) as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// One DER tag-length-value triple.
fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let be = len.to_be_bytes();
        let first = be.iter().position(|b| *b != 0).unwrap_or(be.len() - 1);
        out.push((be.len() - first) as u8 | 0x80);
        out.extend_from_slice(&be[first..]);
    }
    out.extend_from_slice(content);
    out
}

/// A DER object identifier from its arcs.
fn oid(arcs: &[u64]) -> Vec<u8> {
    let mut content = vec![(arcs[0] * 40 + arcs[1]) as u8];
    for &arc in &arcs[2..] {
        let mut stack = vec![(arc & 0x7f) as u8];
        let mut rest = arc >> 7;
        while rest > 0 {
            stack.push(((rest & 0x7f) as u8) | 0x80);
            rest >>= 7;
        }
        stack.reverse();
        content.extend(stack);
    }
    tlv(0x06, &content)
}

/// A minimal X.509 certificate carrying one SM2 public key: just enough
/// structure for a parser to read the SubjectPublicKeyInfo back out.
fn sm2_cert_der(public_key: &[u8]) -> Vec<u8> {
    let ec_public_key = oid(&[1, 2, 840, 10045, 2, 1]);
    let sm2_curve = oid(&[1, 2, 156, 10197, 1, 301]);
    let algorithm = tlv(0x30, &[ec_public_key, sm2_curve].concat());
    let empty_name = tlv(0x30, &[]);
    let validity = tlv(
        0x30,
        &[tlv(0x17, b"260101000000Z"), tlv(0x17, b"991231235959Z")].concat(),
    );
    let mut bit_string = vec![0x00];
    bit_string.extend_from_slice(public_key);
    let spki = tlv(0x30, &[algorithm.clone(), tlv(0x03, &bit_string)].concat());

    let mut tbs = tlv(0xA0, &tlv(0x02, &[0x02]));
    tbs.extend(tlv(0x02, &[0x01]));
    tbs.extend(&algorithm);
    tbs.extend(&empty_name);
    tbs.extend(&validity);
    tbs.extend(&empty_name);
    tbs.extend(&spki);

    let mut cert = tlv(0x30, &tbs);
    cert.extend(&algorithm);
    cert.extend(tlv(0x03, &[0x00]));
    tlv(0x30, &cert)
}

/// A descriptor carrying its proof, the way a signed document stores it.
fn descriptor_with_proof(id: &str, refs: &[(&str, &str)], check_value: &str, cert: &str) -> String {
    let mut xml = format!(r#"<Signature ID="{id}">"#);
    for (method, file) in refs {
        xml.push_str(&format!(
            r#"<SignedInfo><Reference CheckMethod="{method}" FileRef="{file}"/></SignedInfo>"#
        ));
    }
    xml.push_str(&format!("<CheckValue>{check_value}</CheckValue>"));
    xml.push_str(&format!("<Cert>{cert}</Cert>"));
    xml.push_str("</Signature>");
    xml
}

/// Entries for a document whose single signature carries a certificate.
fn signed_entries(document: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let mut entries = vec![
        ("Doc_0/Document.xml", document.to_vec()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
    ];
    // The digest only covers the referenced file, so it can be computed
    // against the placeholder and the archive rebuilt with the real proof.
    let pair = Sm2KeyPair::generate().expect("keygen");
    let mut package = open(&entries);
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    let digest = signed_digest(&mut package, &sig).expect("digest");
    let proof = base64_of(&pair.sign(&digest));
    let cert = base64_of(&sm2_cert_der(&pair.public_bytes()));
    entries[1].1 =
        descriptor_with_proof("s", &[("SM3", "Doc_0/Document.xml")], &proof, &cert).into_bytes();
    entries
}

#[test]
fn gb_t_33190_18_2_4_certificate_key_verifies_the_check_value() {
    let mut package = open(&signed_entries(b"document bytes"));
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    assert_eq!(
        verify_signature(&mut package, &sig).expect("verify"),
        VerifyStatus::Valid
    );
}

#[test]
fn gb_t_33190_18_2_4_tampered_content_fails_the_check_value() {
    let mut entries = signed_entries(b"document bytes");
    entries[0].1 = b"document BYTES".to_vec();
    let mut package = open(&entries);
    let sig = read_signature(&mut package, SIG_FILE).expect("descriptor");
    assert_eq!(
        verify_signature(&mut package, &sig).expect("verify"),
        VerifyStatus::Tampered
    );
}
