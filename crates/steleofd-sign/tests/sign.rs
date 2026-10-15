//! Tests for signature reading and verification on synthetic packages.

use steleofd_crypto::Sm2KeyPair;
use steleofd_error::OfdError;
use steleofd_package::Package;
use steleofd_sign::{
    Reference, Signature, VerifyStatus, read_sign_list, read_signature, signed_digest,
    verify_signature, verify_with_public_key,
};
use steleofd_testkit::{
    DOCUMENT_ENTRY, OFD_XML_SIGNED, SIGN_FILE, SIGNS_ENTRY, SIGNS_XML, descriptor, open_package,
    open_package_bytes, signed_descriptor_entries, signed_fixture,
};

const SIG_FILE: &str = "Signs/Sign_0.xml";

fn signs_xml() -> Vec<u8> {
    br#"<Signs><Sign BaseLoc="Signs/Sign_0.xml"/></Signs>"#.to_vec()
}

fn open(entries: &[(&str, Vec<u8>)]) -> Package {
    open_package(entries)
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

#[test]
fn gb_t_33190_18_2_4_certificate_key_verifies_the_check_value() {
    let (bytes, _pair) = signed_fixture();
    let mut package = open_package_bytes(&bytes);
    let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
    assert_eq!(
        verify_signature(&mut package, &sig).expect("verify"),
        VerifyStatus::Valid
    );
}

#[test]
fn gb_t_33190_18_2_4_tampered_content_fails_the_check_value() {
    let (mut extra, _key) = signed_descriptor_entries(
        "sig1",
        DOCUMENT_ENTRY,
        steleofd_testkit::DOCUMENT_XML.as_bytes(),
    );
    let mut entries = steleofd_testkit::base_entries(OFD_XML_SIGNED);
    entries.push((SIGNS_ENTRY, SIGNS_XML.as_bytes().to_vec()));
    let doc = entries.iter().position(|(n, _)| *n == DOCUMENT_ENTRY);
    entries[doc.expect("document entry")].1 = b"tampered".to_vec();
    entries.append(&mut extra);
    let mut package = open(&entries);
    let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
    assert_eq!(
        verify_signature(&mut package, &sig).expect("verify"),
        VerifyStatus::Tampered
    );
}
