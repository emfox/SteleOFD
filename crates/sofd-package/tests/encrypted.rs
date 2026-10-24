//! What an encrypted container answers: a state the reader has to name, since
//! the bytes are exactly what the producer wrote and nothing is damaged.

use sofd_error::OfdError;
use sofd_package::{Package, build_archive};

/// Entries for a document that opens, so the only thing under test is whether
/// its bytes are encrypted.
fn document() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            "OFD.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0"><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>"#
                as &[u8],
        ),
        (
            "Doc_0/Document.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Document><CommonData><PageArea><PhysicalBox>0 0 10 10</PhysicalBox></PageArea></CommonData>
<Pages><Page ID="1"/></Pages></Document>"# as &[u8],
        ),
    ]
}

/// Set the GP bit that marks an entry encrypted, which is the only thing a
/// header needs to declare: the bytes behind it stay whatever they were.
fn mark_encrypted(archive: &[u8]) -> Vec<u8> {
    const LOCAL: [u8; 4] = [b'P', b'K', 3, 4];
    const CENTRAL: [u8; 4] = [b'P', b'K', 1, 2];
    let mut out = archive.to_vec();
    let mut at = 0;
    while at + 8 <= out.len() {
        let header = &out[at..at + 4];
        // A local header puts Flags at 6, a central directory entry at 8.
        let offset = if header == LOCAL {
            6
        } else if header == CENTRAL {
            8
        } else {
            at += 1;
            continue;
        };
        let flags = u16::from_le_bytes([out[at + offset], out[at + offset + 1]]);
        let flagged = (flags | 0x0001).to_le_bytes();
        out[at + offset..at + offset + 2].copy_from_slice(&flagged);
        at += 4;
    }
    out
}

#[test]
fn a_plain_container_is_not_encrypted() {
    let bytes = build_archive(&document()).expect("archive");
    let package = Package::from_bytes(&bytes).expect("it opens");
    assert!(!package.is_encrypted());
}

#[test]
fn an_encrypted_entry_is_recorded_at_open_and_refused_at_read() {
    let bytes = build_archive(&document()).expect("archive");
    let encrypted = mark_encrypted(&bytes);
    let mut package = Package::from_bytes(&encrypted).expect("the directory still opens");
    assert!(
        package.is_encrypted(),
        "the header says so, and that is what a caller is told"
    );
    // Opening records the state without needing a password; reading is where
    // the password would be needed, and that is a state of its own.
    let out = package.read("Doc_0/Document.xml");
    assert!(
        matches!(out, Err(OfdError::PasswordRequired)),
        "a locked entry is not corruption, got {out:?}"
    );
}
