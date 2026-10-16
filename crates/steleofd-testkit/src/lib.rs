//! Fixtures shared by the test suites, so a document shape is defined once.
//! Four suites used to carry private copies of the same zip builder, the same
//! base64 and DER helpers and the same signed document. A dev-dependency only.

#![warn(missing_docs)]

mod bench;
mod der;
mod document;
mod signed;

pub use bench::{OBJECTS_PER_PAGE, bench_archive};
pub use der::{base64_of, oid, sm2_cert_der, tlv};
pub use document::{
    CONTENT_ENTRY, CONTENT_XML, DOCUMENT_ENTRY, DOCUMENT_XML, OFD_XML, OFD_XML_SIGNED, SIGN_FILE,
    SIGN_VALUE_FILE, SIGNS_ENTRY, SIGNS_XML, archive_of, base_entries, font_bytes,
    multipage_archive, multipage_document_xml, open_package, open_package_bytes,
};
pub use signed::{
    descriptor, descriptor_with_proof, fixture_bytes, signed_descriptor_entries, signed_fixture,
    unsigned_descriptor_entries,
};
