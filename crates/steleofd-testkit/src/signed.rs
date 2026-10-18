//! Descriptors and the signed fixture, where the proof has to be real. A
//! signature only means something if the digest covers the actual bytes, so the
//! fixture writes placeholders, computes the real digest, then rebuilds it.

use steleofd_crypto::Sm2KeyPair;
use steleofd_sign::{read_signature, signed_digest};

use crate::der::{base64_of, sm2_cert_der};
use crate::document::{
    DOCUMENT_ENTRY, OFD_XML, OFD_XML_SIGNED, SIGN_FILE, SIGN_VALUE_FILE, SIGNS_ENTRY, SIGNS_XML,
    base_entries, open_package,
};

/// A descriptor with no proof, as a first pass writes it. The method lives on
/// `References` and each `Reference` carries its own `CheckValue`, which is what
/// the annex A schema for clause 18.2.2 actually requires.
pub fn descriptor(id: &str, refs: &[(&str, &str)]) -> String {
    let mut xml = format!(r#"<Signature ID="{id}"><SignedInfo>"#);
    if let Some((method, _)) = refs.first() {
        xml.push_str(&format!(r#"<References CheckMethod="{method}">"#));
    } else {
        xml.push_str("<References>");
    }
    for (_, file) in refs {
        xml.push_str(&format!(
            r#"<Reference FileRef="{file}"><CheckValue></CheckValue></Reference>"#
        ));
    }
    xml.push_str("</References></SignedInfo></Signature>");
    xml
}

/// A descriptor carrying its proof, the way a signed document stores it. The
/// proof is not inline: `SignedValue` is a path, so the caller supplies it as a
/// separate entry.
pub fn descriptor_with_proof(
    id: &str,
    refs: &[(&str, &str)],
    digests: &[Vec<u8>],
    check_value_ref: &str,
    cert: &str,
) -> String {
    let mut xml = format!(r#"<Signature ID="{id}"><SignedInfo>"#);
    if let Some((method, _)) = refs.first() {
        xml.push_str(&format!(r#"<References CheckMethod="{method}">"#));
    } else {
        xml.push_str("<References>");
    }
    for (i, (_, file)) in refs.iter().enumerate() {
        // Each Reference carries the digest of the file it protects.
        let digest = digests.get(i).cloned().unwrap_or_default();
        xml.push_str(&format!(
            r#"<Reference FileRef="{file}"><CheckValue>{}</CheckValue></Reference>"#,
            base64_of(&digest)
        ));
    }
    xml.push_str("</References></SignedInfo>");
    xml.push_str(&format!("<SignedValue>{check_value_ref}</SignedValue>"));
    xml.push_str(&format!(r#"<Cert>{cert}</Cert>"#));
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
    let refs: &[(&str, &str)] = &[("SM3", signed_entry)];
    let mut entries: Vec<(&'static str, Vec<u8>)> = vec![
        (DOCUMENT_ENTRY, document.to_vec()),
        (SIGN_FILE, descriptor(id, refs).into_bytes()),
    ];
    let pair = Sm2KeyPair::generate().expect("keygen");
    // The proof is the SM2 signature over the digest of the referenced files.
    let mut package = open_package(&entries);
    let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
    let digest = signed_digest(&mut package, &sig)
        .expect("digest")
        .expect("the fixture signs with SM3");
    let proof = base64_of(&pair.sign(&digest));
    let cert = base64_of(&sm2_cert_der(&pair.public_bytes()));
    // The descriptor records each protected file's own digest, so a reader can
    // spot a changed file without holding the signer's key.
    let file_digests: Vec<Vec<u8>> = refs
        .iter()
        .map(|(_, _file)| steleofd_crypto::sm3(document).to_vec())
        .collect();
    // The document entry only existed so the digest could be computed; it is
    // dropped here so the caller keeps exactly one copy of it.
    entries.remove(0);
    entries[0].1 =
        descriptor_with_proof(id, refs, &file_digests, SIGN_VALUE_FILE, &cert).into_bytes();
    entries.push((SIGN_VALUE_FILE, proof.into_bytes()));
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
