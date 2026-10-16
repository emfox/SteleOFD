//! Tests for signature reading and verification on synthetic packages.

use steleofd_crypto::Sm2KeyPair;
use steleofd_error::OfdError;
use steleofd_package::Package;
use steleofd_sign::{
    Reference, Signature, SignatureKind, VerifyStatus, read_sign_list, read_signature,
    signed_digest, verify_descriptor, verify_signature, verify_with_public_key,
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
            check_value: Vec::new(),
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
    // The descriptor records each protected file's own digest, so the change is
    // caught there rather than only at the final signature check.
    let out = verify_signature(&mut package, &sig);
    assert!(
        matches!(out, Err(OfdError::Signature(_))),
        "tampered content must not verify, got {out:?}"
    );
}

#[test]
fn gb_t_33190_18_1_the_signature_list_carries_id_and_kind() {
    // Each Sign entry names its descriptor and says whether it is a seal or a
    // plain signature; Seal is what the schema defaults to.
    let xml = br#"<Signatures>
      <MaxSignId>S3</MaxSignId>
      <Signature ID="S1" BaseLoc="Signs/Sign_0.xml"/>
      <Signature ID="S2" Type="Sign" BaseLoc="Signs/Sign_1.xml"/>
      <Signature ID="S3" Type="Seal" BaseLoc="Signs/Sign_2.xml"/>
    </Signatures>"#
        .to_vec();
    let mut package = open(&[("Signs/Signs.xml", xml)]);
    let signs = read_sign_list(&mut package, "Signs/Signs.xml").expect("list");
    assert_eq!(signs.len(), 3);
    assert_eq!(signs[0].id.as_deref(), Some("S1"));
    assert_eq!(signs[0].kind, SignatureKind::Seal, "Seal is the default");
    assert_eq!(signs[1].kind, SignatureKind::Sign);
    assert_eq!(signs[2].kind, SignatureKind::Seal);
    assert_eq!(signs[2].base_loc, "Signs/Sign_2.xml");
}

#[test]
fn gb_t_33190_18_2_2_check_method_governs_every_reference() {
    // CheckMethod is written once on References and applies to each Reference
    // below it, so a descriptor with several protected files needs it only once.
    let xml = br#"<Signature ID="s">
      <SignedInfo><References CheckMethod="SM3">
        <Reference FileRef="a.bin"><CheckValue>AAAA</CheckValue></Reference>
        <Reference FileRef="b.bin"><CheckValue>BBBB</CheckValue></Reference>
      </References></SignedInfo>
    </Signature>"#
        .to_vec();
    let mut package = open(&[("Signs/Sign_0.xml", xml)]);
    let sig = read_signature(&mut package, "Signs/Sign_0.xml").expect("descriptor");
    assert_eq!(sig.references.len(), 2);
    assert!(sig.references.iter().all(|r| r.check_method == "SM3"));
    assert_eq!(sig.references[0].file_ref, "a.bin");
    assert_eq!(sig.references[1].file_ref, "b.bin");
    assert!(
        !sig.references[0].check_value.is_empty(),
        "each reference keeps its own recorded digest"
    );
    assert_ne!(
        sig.references[0].check_value, sig.references[1].check_value,
        "the two references do not share a digest"
    );
}

#[test]
fn gb_t_33190_18_2_3_the_proof_lives_behind_signed_value() {
    // SignedValue is a path, not the proof itself, so the descriptor alone is
    // not enough to verify: the referenced file has to be read too.
    let (bytes, pair) = signed_fixture();
    let mut package = open_package_bytes(&bytes);
    let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
    assert!(
        sig.signed_value_ref.is_some(),
        "the descriptor points at its proof"
    );
    assert!(
        sig.check_value.is_empty(),
        "reading the descriptor alone leaves the proof unloaded"
    );

    // A key that is not the signer's must not verify it.
    let other = Sm2KeyPair::generate().expect("keygen");
    assert_eq!(
        verify_descriptor(&mut package, &sig, &other.public_bytes()).expect("verify"),
        VerifyStatus::Tampered,
        "a stranger's key does not validate the proof"
    );
    assert_eq!(
        verify_descriptor(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Valid,
        "the signer's key does"
    );
}

#[test]
fn gb_t_33190_18_2_3_a_stamp_records_where_it_sits() {
    // An electronic seal also says where on which page it is drawn, and may
    // clip itself to a smaller region.
    let xml = br#"<Signature ID="s">
      <SignedInfo><References CheckMethod="SM3">
        <Reference FileRef="a.bin"><CheckValue>AAAA</CheckValue></Reference>
      </References></SignedInfo>
      <StampAnnot ID="1" PageRef="3" Boundary="10 20 30 40" Clip="12 22 26 36"/>
      <Seal><BaseLoc>Res/Seal.esl</BaseLoc></Seal>
    </Signature>"#
        .to_vec();
    let mut package = open(&[("Signs/Sign_0.xml", xml)]);
    let sig = read_signature(&mut package, "Signs/Sign_0.xml").expect("descriptor");
    assert_eq!(sig.stamps.len(), 1);
    let stamp = &sig.stamps[0];
    assert_eq!(stamp.id, "1");
    assert_eq!(stamp.page_ref, "3");
    assert_eq!(
        stamp.boundary,
        steleofd_common::Rect::new(10.0, 20.0, 30.0, 40.0)
    );
    assert_eq!(
        stamp.clip,
        Some(steleofd_common::Rect::new(12.0, 22.0, 26.0, 36.0))
    );
    assert_eq!(sig.seal.as_deref(), Some("Res/Seal.esl"));
}

#[test]
fn gb_t_33190_18_2_2_the_provider_name_is_an_attribute() {
    // Provider carries its name as ProviderName rather than as element text,
    // which a text-only reader would silently lose.
    let xml = br#"<Signature ID="s">
      <SignedInfo>
        <Provider ProviderName="Example CA" Version="1.0"/>
        <SignatureMethod>SM2-SM3</SignatureMethod>
        <SignatureDateTime>2024-05-01T09:00:00</SignatureDateTime>
        <References CheckMethod="SM3">
          <Reference FileRef="a.bin"><CheckValue>AAAA</CheckValue></Reference>
        </References>
      </SignedInfo>
    </Signature>"#
        .to_vec();
    let mut package = open(&[("Signs/Sign_0.xml", xml)]);
    let sig = read_signature(&mut package, "Signs/Sign_0.xml").expect("descriptor");
    assert_eq!(sig.provider.as_deref(), Some("Example CA"));
    assert_eq!(sig.signature_method.as_deref(), Some("SM2-SM3"));
    assert_eq!(sig.sign_time.as_deref(), Some("2024-05-01T09:00:00"));
}

#[test]
fn gb_t_33190_18_3_a_sealed_document_verifies_and_an_edited_one_does_not() {
    // Clause 18.3: sealing fixes the content, so the same descriptor verifies
    // against what was sealed and reports a mismatch once that content moves.
    let sealed = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"the sealed document".to_vec()),
    ];
    let (sig, pair) = make_signature(&sealed, &[("SM3", "Doc_0/Document.xml")]);
    let mut package = open(&sealed);
    assert_eq!(
        verify_with_public_key(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Valid,
        "the sealed bytes verify"
    );

    let edited = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"the edited document".to_vec()),
    ];
    let mut package = open(&edited);
    assert_eq!(
        verify_with_public_key(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Tampered,
        "editing a sealed file breaks the seal"
    );
}

#[test]
fn gb_t_33190_18_4_validation_reports_the_scope_it_checked() {
    // Clause 18.4: validation covers exactly the files the descriptor names.
    // A file outside that list may change without the seal moving, and that
    // is the scope the check value was computed over.
    let entries = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"sealed".to_vec()),
        ("Doc_0/Res/Other.xml", b"outside the scope".to_vec()),
    ];
    let (sig, pair) = make_signature(&entries, &[("SM3", "Doc_0/Document.xml")]);
    let mut package = open(&entries);
    let read = read_signature(&mut package, SIG_FILE).expect("descriptor");
    assert_eq!(
        read.references
            .iter()
            .map(|r| r.file_ref.as_str())
            .collect::<Vec<_>>(),
        vec!["Doc_0/Document.xml"],
        "the descriptor states which file it covers"
    );

    // Editing a file outside the sealed set leaves the check value alone.
    let outside_edited = vec![
        ("Signs/Signs.xml", signs_xml()),
        (
            SIG_FILE,
            descriptor("s", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ),
        ("Doc_0/Document.xml", b"sealed".to_vec()),
        ("Doc_0/Res/Other.xml", b"edited outside the scope".to_vec()),
    ];
    let mut package = open(&outside_edited);
    assert_eq!(
        verify_with_public_key(&mut package, &sig, &pair.public_bytes()).expect("verify"),
        VerifyStatus::Valid,
        "the checked scope is the referenced files and nothing else"
    );
}
