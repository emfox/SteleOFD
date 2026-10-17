//! CLI coverage: every subcommand driven in-process against a built fixture.
//! The fixture follows the spec form of ST_Box (element text), so the runs
//! exercise the same paths real documents take.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use steleofd_cli::Report;
use steleofd_package::build_archive;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
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

/// A real face, so glyph outlining runs for real rather than degrading.
fn font_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
    );
    std::fs::read(path).expect("font fixture")
}

/// A private scratch directory per call, so parallel tests never collide.
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "steleofd-cli-{}-{}-{}",
        std::process::id(),
        name,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// The fixture document on disk, ready to name on a command line.
fn fixture_path() -> PathBuf {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes().to_vec()),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            CONTENT_XML.as_bytes().to_vec(),
        ),
        (
            "Doc_0/PublicRes.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><Fonts><Font ID="7" FontName="Orbitron"><FontFile>Orbitron-Bold.ttf</FontFile></Font></Fonts></Res>"#
                .to_vec(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ];
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(path, data)| (*path, data.as_slice()))
        .collect();
    let bytes = build_archive(&borrowed).expect("build fixture");
    let path = scratch("fixture").join("fixture.ofd");
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn report_for(args: &[&str]) -> Report {
    let cli = steleofd_cli::parse(args).expect("parse");
    let i18n = steleofd_cli::i18n_for(&cli);
    steleofd_cli::run(&cli, &i18n).expect("run")
}

fn error_for(args: &[&str]) -> Report {
    let cli = steleofd_cli::parse(args).expect("parse");
    let i18n = steleofd_cli::i18n_for(&cli);
    let err = steleofd_cli::run(&cli, &i18n).expect_err("must fail");
    steleofd_cli::error_report(&err, &i18n)
}

#[test]
fn info_reports_what_the_document_declares() {
    let path = fixture_path();
    let report = report_for(&["info", "--locale", "en-US", path.to_str().unwrap()]);
    assert!(report.ok);
    assert!(report.text.contains("Version: 1.0"), "{}", report.text);
    assert!(report.text.contains("Pages: 1"), "{}", report.text);
    assert!(
        report.text.contains("Page size: 210.0 x 297.0 mm"),
        "{}",
        report.text
    );
    assert!(report.text.contains("Embedded fonts: 1"), "{}", report.text);
    assert!(report.text.contains("Signatures: 0"), "{}", report.text);
}

#[test]
fn info_localizes_to_chinese() {
    let path = fixture_path();
    let report = report_for(&["info", "--locale", "zh-CN", path.to_str().unwrap()]);
    // Escaped, not literal: the source stays English by convention.
    assert!(
        report.text.contains("\u{9875}\u{6570}\u{ff1a}1"),
        "{}",
        report.text
    );
    assert!(
        report
            .text
            .contains("\u{9875}\u{9762}\u{5c3a}\u{5bf8}\u{ff1a}210.0 x 297.0 \u{6beb}\u{7c73}"),
        "{}",
        report.text
    );
}

#[test]
fn render_writes_a_png() {
    let path = fixture_path();
    let out = scratch("render").join("page.png");
    let report = report_for(&[
        "render",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(report.ok);
    assert!(
        report.text.contains("Rendered page 0 at 150 dpi"),
        "{}",
        report.text
    );
    assert!(report.text.contains("Wrote "), "{}", report.text);
    let png = std::fs::read(&out).expect("read png");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "not a PNG");
    assert!(png.len() > 1000, "the page rendered to almost nothing");
}

#[test]
fn render_rejects_an_out_of_range_page() {
    let path = fixture_path();
    let report = error_for(&[
        "render",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "-p",
        "9",
    ]);
    assert!(!report.ok);
    assert!(
        report.text.contains("Page index out of range"),
        "{}",
        report.text
    );
    assert!(report.text.starts_with("error:"), "{}", report.text);
}

#[test]
fn convert_to_pdf_holds_the_whole_document() {
    let path = fixture_path();
    let dir = scratch("pdf");
    let report = report_for(&[
        "convert",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "-f",
        "pdf",
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(report.ok);
    assert!(
        report.text.contains("Converted 1 page to pdf"),
        "{}",
        report.text
    );
    let pdf = std::fs::read(dir.join("fixture.pdf")).expect("read pdf");
    assert!(pdf.starts_with(b"%PDF"), "not a PDF");
}

#[test]
fn convert_to_svg_and_png_writes_per_page_files() {
    let path = fixture_path();
    let dir = scratch("svg-png");
    let report = report_for(&[
        "convert",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "-f",
        "svg",
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(report.ok);
    let svg = std::fs::read(dir.join("fixture-0.svg")).expect("read svg");
    let svg = String::from_utf8(svg).expect("utf8 svg");
    assert!(svg.contains("<svg"), "{}", svg);

    let report = report_for(&[
        "convert",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "-f",
        "png",
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(report.ok);
    let png = std::fs::read(dir.join("fixture-0.png")).expect("read png");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "not a PNG");
}

#[test]
fn search_finds_the_text_on_its_page() {
    let path = fixture_path();
    let report = report_for(&[
        "search",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "Stele",
    ]);
    assert!(report.ok);
    assert!(report.text.contains("page 0:"), "{}", report.text);
    assert!(report.text.contains("SteleOFD"), "{}", report.text);
}

#[test]
fn search_without_hits_is_empty_but_ok() {
    let path = fixture_path();
    let report = report_for(&[
        "search",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "no-such-text",
    ]);
    assert!(report.ok);
    assert!(report.text.is_empty(), "{}", report.text);
}

#[test]
fn audit_reports_findings_without_failing() {
    let path = fixture_path();
    let report = report_for(&["audit", "--locale", "en-US", path.to_str().unwrap()]);
    assert!(report.ok, "warnings must not fail the run: {}", report.text);
    assert!(report.text.contains("ARCH-006"), "{}", report.text);
}

#[test]
fn verify_without_signatures_fails_the_run() {
    let path = fixture_path();
    let report = report_for(&["verify", "--locale", "en-US", path.to_str().unwrap()]);
    assert!(!report.ok);
    assert!(
        report.text.contains("The document declares no signatures"),
        "{}",
        report.text
    );
}

#[test]
fn print_plans_the_job_over_the_whole_document() {
    let path = fixture_path();
    let report = report_for(&[
        "print",
        "--locale",
        "en-US",
        path.to_str().unwrap(),
        "--paper",
        "a4",
        "--copies",
        "2",
    ]);
    assert!(report.ok);
    // Simplex by default, so each copy lands on its own sheet.
    assert!(
        report.text.contains("2 pages on 2 sheets, A4 at 300 dpi"),
        "{}",
        report.text
    );
}

#[test]
fn a_missing_file_reports_an_error() {
    let dir = scratch("missing");
    let report = error_for(&[
        "info",
        "--locale",
        "en-US",
        dir.join("nope.ofd").to_str().unwrap(),
    ]);
    assert!(!report.ok);
    assert!(report.text.starts_with("error:"), "{}", report.text);
    assert!(report.text.contains("I/O"), "{}", report.text);
}

#[test]
fn a_container_without_the_entry_point_is_rejected() {
    // A valid archive that is not an OFD: no OFD.xml to enter through.
    let bytes =
        build_archive(&[("Doc_0/Document.xml", b"<Document/>".as_slice())]).expect("build archive");
    let path = scratch("no-entry").join("no-entry.ofd");
    std::fs::write(&path, bytes).expect("write archive");
    let report = error_for(&["info", "--locale", "en-US", path.to_str().unwrap()]);
    assert!(!report.ok);
    assert!(report.text.contains("not a valid OFD"), "{}", report.text);
}

#[test]
fn bytes_that_are_not_a_container_are_rejected() {
    let path = scratch("garbage").join("garbage.ofd");
    std::fs::write(&path, b"this is not an ofd package").expect("write garbage");
    let report = error_for(&["info", "--locale", "en-US", path.to_str().unwrap()]);
    assert!(!report.ok);
    assert!(report.text.contains("corrupted"), "{}", report.text);
}
