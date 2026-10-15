//! The fixture document: a one-page OFD package with a real embedded face.
//! The XML is written out rather than generated, because a fixture that a
//! builder produces can only ever confirm the builder itself.

use steleofd_package::{Package, build_archive};

/// Where a signed document keeps its signature descriptor.
pub const SIGN_FILE: &str = "Doc_0/Signs/Sign_0.xml";
/// The file the descriptor's `SignedValue` points at, holding the proof.
pub const SIGN_VALUE_FILE: &str = "Doc_0/Signs/Sign_0.dat";

/// The container entry, with no signature.
pub const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

/// The container entry, pointing at a signature list.
pub const OFD_XML_SIGNED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody>
    <DocRoot>Doc_0/Document.xml</DocRoot>
    <Signatures>Doc_0/Signs/Signs.xml</Signatures>
  </DocBody>
</OFD>"#;

/// The document entry: one A4 page.
pub const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>
  </CommonData>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;

/// The page content: one filled path and one text run.
pub const CONTENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 210 297</PhysicalBox></Area>
  <Content>
    <Layer ID="2" Type="Body">
      <PathObject ID="3" Boundary="10 10 60 20" Fill="true">
        <FillColor Value="255 0 0"/>
        <AbbreviatedData>M 10 10 L 70 10 L 70 30 L 10 30 L 10 10</AbbreviatedData>
      </PathObject>
      <TextObject ID="4" Boundary="10 40 90 20" Font="7" Size="3.5">
        <FillColor Value="0 0 0"/>
        <TextCode X="10" Y="50">SteleOFD</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#;

/// The signature list entry.
pub const SIGNS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Signs><Sign BaseLoc="Doc_0/Signs/Sign_0.xml"/></Signs>"#;

/// The path to the page content entry inside the archive.
pub const CONTENT_ENTRY: &str = "Doc_0/Pages/Page_0/Content.xml";
/// The path to the document entry inside the archive.
pub const DOCUMENT_ENTRY: &str = "Doc_0/Document.xml";
/// The path to the signature list entry inside the archive.
pub const SIGNS_ENTRY: &str = "Doc_0/Signs/Signs.xml";

/// A real face, so glyph outlining runs for real rather than degrading.
pub fn font_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
    );
    std::fs::read(path).expect("font fixture")
}

/// Build a package from entries written in the order given.
pub fn open_package(entries: &[(&str, Vec<u8>)]) -> Package {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let bytes = build_archive(&borrowed).expect("archive");
    Package::from_bytes(&bytes).expect("package")
}

/// Open a package straight from `.ofd` bytes, as a caller who read a file has.
pub fn open_package_bytes(bytes: &[u8]) -> Package {
    Package::from_bytes(bytes).expect("package")
}

/// The four entries every fixture document carries, unsigned.
pub fn base_entries(ofd: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("OFD.xml", ofd.as_bytes().to_vec()),
        (DOCUMENT_ENTRY, DOCUMENT_XML.as_bytes().to_vec()),
        (CONTENT_ENTRY, CONTENT_XML.as_bytes().to_vec()),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ]
}

/// The archive bytes for a list of entries.
pub fn archive_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    build_archive(&borrowed).expect("archive")
}
