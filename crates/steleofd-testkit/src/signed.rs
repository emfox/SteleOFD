//! Descriptors and the signed fixture, where the proof has to be real. A
//! signature only means something if the digest covers the actual bytes, so the
//! fixture writes placeholders, computes the real digest, then rebuilds it.

use steleofd_crypto::Sm2KeyPair;
use steleofd_sign::{read_signature, signed_digest};

use crate::der::{base64_of, sm2_cert_der};
use crate::document::{
    DOCUMENT_ENTRY, OFD_XML, OFD_XML_SIGNED, SIGN_FILE, SIGNS_ENTRY, SIGNS_XML, base_entries,
    open_package,
};

/// A descriptor with no proof, as a first pass writes it.
pub fn descriptor(id: &str, refs: &[(&str, &str)]) -> String {
    let mut xml = format!(r#"<Signature ID="{id}">"#);
    for (method, file) in refs {
        xml.push_str(&format!(
            r#"<SignedInfo><Reference CheckMethod="{method}" FileRef="{file}"/></SignedInfo>"#
        ));
    }
    xml.push_str("</Signature>");
    xml
}

/// A descriptor carrying its proof, the way a signed document stores it.
pub fn descriptor_with_proof(
    id: &str,
    refs: &[(&str, &str)],
    check_value: &str,
    cert: &str,
) -> String {
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

/// Entries for a document whose single signature carries a real proof. Only the
/// signature entries come back, because the document and content entries belong
/// to `base_entries` and a second copy would be a duplicate archive name.
pub fn signed_descriptor_entries(
    id: &str,
    signed_entry: &str,
    document: &[u8],
) -> (Vec<(&'static str, Vec<u8>)>, Sm2KeyPair) {
    let mut entries: Vec<(&'static str, Vec<u8>)> = vec![
        (DOCUMENT_ENTRY, document.to_vec()),
        (
            SIGN_FILE,
            descriptor(id, &[("SM3", signed_entry)]).into_bytes(),
        ),
    ];
    let pair = Sm2KeyPair::generate().expect("keygen");
    let mut package = open_package(&entries);
    let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
    let digest = signed_digest(&mut package, &sig).expect("digest");
    let proof = base64_of(&pair.sign(&digest));
    let cert = base64_of(&sm2_cert_der(&pair.public_bytes()));
    let last = entries.len() - 1;
    let replacement =
        descriptor_with_proof(id, &[("SM3", signed_entry)], &proof, &cert).into_bytes();
    // The document entry only existed so the digest could be computed; it is
    // dropped here so the caller keeps exactly one copy of it.
    let _ = entries.remove(0);
    entries[last - 1].1 = replacement;
    (entries, pair)
}

/// The same entries before any proof is attached, for tamper tests.
pub fn unsigned_descriptor_entries(
    id: &str,
    signed_entry: &str,
    document: &[u8],
) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (DOCUMENT_ENTRY, document.to_vec()),
        (
            SIGN_FILE,
            descriptor(id, &[("SM3", signed_entry)]).into_bytes(),
        ),
    ]
}

/// The fixture document as `.ofd` bytes, optionally carrying one signature. The
/// signature covers the real document entry, so the proof verifies against the
/// bytes the engine will read rather than against a stand-in.
pub fn signed_fixture() -> (Vec<u8>, Sm2KeyPair) {
    let (mut extra, pair) = signed_descriptor_entries(
        "sig1",
        DOCUMENT_ENTRY,
        crate::document::DOCUMENT_XML.as_bytes(),
    );
    let mut entries = base_entries(OFD_XML_SIGNED);
    entries.push((SIGNS_ENTRY, SIGNS_XML.as_bytes().to_vec()));
    entries.append(&mut extra);
    (crate::document::archive_of(&entries), pair)
}

/// The unsigned fixture document.
pub fn fixture_bytes(signed: bool) -> Vec<u8> {
    if signed {
        return signed_fixture().0;
    }
    crate::document::archive_of(&base_entries(OFD_XML))
}

// The fixture must agree with the engine, never the other way round, so this
// file's own contract is checked once rather than by every consumer.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::CONTENT_ENTRY;

    #[test]
    fn the_fixture_opens_and_carries_its_entries() {
        let unsigned = crate::document::open_package(&base_entries(OFD_XML));
        assert!(unsigned.contains("OFD.xml"));
        assert!(unsigned.contains(CONTENT_ENTRY));
    }

    #[test]
    fn a_signed_fixture_really_verifies() {
        let (bytes, _pair) = signed_fixture();
        let mut package = steleofd_package::Package::from_bytes(&bytes).expect("archive");
        let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
        assert_eq!(
            steleofd_sign::verify_signature(&mut package, &sig).expect("verify"),
            steleofd_sign::VerifyStatus::Valid,
            "the fixture's own proof must verify, or nothing built on it means anything"
        );
    }
}
