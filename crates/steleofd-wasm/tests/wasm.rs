//! Host-side coverage for the browser bindings: the plain data shapes and
//! the methods that never build a JS value. The JS-returning methods only
//! run under wasm, where a browser supplies the runtime.

use steleofd_core::Document;
use steleofd_crypto::Sm2KeyPair;
use steleofd_package::{Package, build_archive};
use steleofd_search::SearchOptions;
use steleofd_sign::{read_signature, signed_digest};
use steleofd_wasm::OfdDocument;
use steleofd_wasm::shapes::{
    DocumentInfo, SearchHitData, SignatureData, search_hits, signature_statuses,
};

const SIGN_FILE: &str = "Doc_0/Signs/Sign_0.xml";

// The fixture follows the spec form of ST_Box (element text), so the runs
// exercise the same paths real documents take.

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

const OFD_XML_SIGNED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody>
    <DocRoot>Doc_0/Document.xml</DocRoot>
    <Signatures>Doc_0/Signs/Signs.xml</Signatures>
  </DocBody>
</OFD>"#;

const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>
  </CommonData>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;

const CONTENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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

const SIGNS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Signs><Sign BaseLoc="Doc_0/Signs/Sign_0.xml"/></Signs>"#;

/// A real face, so glyph outlining runs for real rather than degrading.
fn font_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
    );
    std::fs::read(path).expect("font fixture")
}

fn open_package(entries: &[(&str, Vec<u8>)]) -> Package {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let bytes = build_archive(&borrowed).expect("archive");
    Package::from_bytes(&bytes).expect("package")
}

/// The fixture document as `.ofd` bytes, optionally carrying one signature.
fn fixture_bytes(signed: bool) -> Vec<u8> {
    let ofd = if signed { OFD_XML_SIGNED } else { OFD_XML };
    let mut entries: Vec<(&str, Vec<u8>)> = vec![
        ("OFD.xml", ofd.as_bytes().to_vec()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes().to_vec()),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            CONTENT_XML.as_bytes().to_vec(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ];
    if signed {
        entries.push(("Doc_0/Signs/Signs.xml", SIGNS_XML.as_bytes().to_vec()));
        entries.push((
            SIGN_FILE,
            descriptor("sig1", &[("SM3", "Doc_0/Document.xml")]).into_bytes(),
        ));
        // The digest only covers the referenced file, so it can be computed
        // against the placeholder and the archive rebuilt with the proof.
        let pair = Sm2KeyPair::generate().expect("keygen");
        let mut package = open_package(&entries);
        let sig = read_signature(&mut package, SIGN_FILE).expect("descriptor");
        let digest = signed_digest(&mut package, &sig).expect("digest");
        let proof = base64_of(&pair.sign(&digest));
        let cert = base64_of(&sm2_cert_der(&pair.public_bytes()));
        let last = entries.len() - 1;
        entries[last].1 =
            descriptor_with_proof("sig1", &[("SM3", "Doc_0/Document.xml")], &proof, &cert)
                .into_bytes();
    }
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    build_archive(&borrowed).expect("archive")
}

fn open_document(signed: bool) -> Document {
    Document::open_bytes(&fixture_bytes(signed)).expect("document")
}

/// Search options that match the way the bindings call into the engine.
fn search_options() -> SearchOptions {
    SearchOptions {
        case_sensitive: false,
        whole_word: false,
        max_hits: None,
    }
}

#[test]
fn document_info_reports_the_declared_facts() {
    let info: DocumentInfo = DocumentInfo::of(&mut open_document(false));
    assert_eq!(info.version, "1.0");
    assert_eq!(info.pages, 1);
    assert_eq!(info.width_mm, 210.0);
    assert_eq!(info.height_mm, 297.0);
    assert_eq!(info.embedded_fonts, 1, "the container ships one face");
    assert_eq!(info.signatures, 0);
}

#[test]
fn search_hits_trim_to_page_and_snippet() {
    let doc = open_document(false);
    let hits: Vec<SearchHitData> = search_hits(&doc, "steleofd", search_options());
    assert_eq!(hits.len(), 1, "the page carries the text once");
    assert_eq!(hits[0].page, 0);
    assert!(
        hits[0].snippet.contains("SteleOFD"),
        "the snippet keeps the matched text"
    );
    assert!(
        search_hits(&doc, "absent", search_options()).is_empty(),
        "a needle nothing matches yields no hits"
    );
}

#[test]
fn signature_statuses_require_a_signature_list() {
    let mut doc = open_document(false);
    assert!(
        signature_statuses(&mut doc).is_err(),
        "a document that declares no signatures is an error, matching the CLI"
    );
}

#[test]
fn signature_statuses_report_the_valid_signature() {
    let mut doc = open_document(true);
    let statuses: Vec<SignatureData> = signature_statuses(&mut doc).expect("statuses");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].location, SIGN_FILE);
    assert!(statuses[0].valid, "the embedded certificate verifies");
}

#[test]
fn open_reports_the_page_count() {
    let doc = OfdDocument::open(&fixture_bytes(false)).expect("open");
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn render_png_returns_image_bytes() {
    let doc = OfdDocument::open(&fixture_bytes(false)).expect("open");
    let png = doc.render_png(0, 96.0).expect("png");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG magic");
    assert!(png.len() > 1000, "a rendered A4 page is not a stray byte");
}

#[test]
fn to_svg_returns_a_document() {
    let doc = OfdDocument::open(&fixture_bytes(false)).expect("open");
    let svg = doc.to_svg(0).expect("svg");
    assert!(svg.contains("<svg"), "an SVG document string");
    assert!(svg.contains("210") && svg.contains("297"), "page size");
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
