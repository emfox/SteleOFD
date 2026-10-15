//! Facade coverage: opening a document, pairing container fonts with the ids
//! the display list uses, and every capability the handle exposes, each behind
//! its own cargo feature.

use steleofd_display::DrawOp;
use steleofd_error::OfdError;
use steleofd_font::FontId;
use steleofd_package::build_archive;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

fn document_xml(pages: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
  </CommonData>
  <Pages>{pages}</Pages>
</Document>"#
    )
}

fn text_page(font: &str, size: &str, text: &str) -> String {
    format!(
        r#"<Page ID="1"><Content><Layer>
  <TextObject ID="10" Boundary="0 0 100 30" Font="{font}" Size="{size}">
    <TextCode X="10" Y="25">{text}</TextCode>
  </TextObject>
</Layer></Content></Page>"#
    )
}

/// A real face, so glyph outlining runs for real rather than degrading.
fn font_bytes() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../steleofd-render/tests/data/Orbitron-Bold.ttf"
    );
    std::fs::read(path).expect("font fixture")
}

fn open(entries: &[(&str, Vec<u8>)]) -> steleofd_core::Document {
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(path, data)| (*path, data.as_slice()))
        .collect();
    let bytes = build_archive(&borrowed).expect("build fixture");
    steleofd_core::Document::open_bytes(&bytes).expect("open document")
}

fn two_page_document() -> steleofd_core::Document {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1"/><Page ID="2"/>"#).into_bytes(),
        ),
    ];
    open(&entries)
}

#[test]
fn opens_and_counts_pages() {
    let doc = two_page_document();
    assert_eq!(doc.page_count(), 2);
    let page = doc.page(0).expect("first page");
    assert_eq!(page.index, 0);
    assert_eq!(page.page_box.w, 210.0);
    assert_eq!(page.page_box.h, 297.0);
}

#[test]
fn page_lookup_is_bounded() {
    let doc = two_page_document();
    assert!(doc.page(1).is_some());
    assert!(doc.page(2).is_none());
    assert!(doc.page(99).is_none());
}

#[test]
fn model_mirrors_the_declaration() {
    let doc = two_page_document();
    let model = doc.model();
    assert_eq!(model.pages.pages.len(), 2);
    assert_eq!(model.common_data.page_area.physical_box.w, 210.0);
}

#[test]
fn rejects_a_container_without_the_entry_point() {
    let bytes =
        build_archive(&[("Doc_0/Document.xml", b"<Document/>".as_slice())]).expect("build fixture");
    let err = steleofd_core::Document::open_bytes(&bytes).expect_err("must fail");
    assert!(err.to_string().contains("not a valid OFD"), "got {err}");
}

#[test]
fn rejects_bytes_that_are_not_a_container() {
    let err = steleofd_core::Document::open_bytes(b"not an ofd package").expect_err("must fail");
    assert!(err.to_string().contains("container"), "got {err}");
}

#[test]
fn registers_container_fonts_and_remaps_ids() {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(&text_page("7", "4.0", "Hi")).into_bytes(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ];
    let doc = open(&entries);
    assert_eq!(
        doc.fonts().registry().len(),
        1,
        "the one container font is registered"
    );
    let run = doc
        .page(0)
        .and_then(|page| {
            page.ops.iter().find_map(|op| match op {
                DrawOp::Text(run) => Some(run),
                _ => None,
            })
        })
        .expect("text run");
    assert_eq!(
        run.font,
        FontId(0),
        "the document font id is rewritten to the registered id"
    );
    assert!(
        doc.fonts().face(FontId(0)).is_ok(),
        "the remapped id resolves to a loadable face"
    );
}

#[test]
fn non_font_entries_are_ignored() {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(&text_page("7", "4.0", "Hi")).into_bytes(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
        ("Doc_0/Res/readme.txt", b"not a font".to_vec()),
    ];
    let doc = open(&entries);
    assert_eq!(doc.fonts().registry().len(), 1);
}

#[test]
fn fonts_outside_the_document_directory_are_ignored() {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1"/>"#).into_bytes(),
        ),
        ("Other_Dir/Orbitron-Bold.ttf", font_bytes()),
    ];
    let doc = open(&entries);
    assert_eq!(doc.fonts().registry().len(), 0);
}

#[cfg(feature = "render")]
#[test]
fn renders_page_at_requested_dpi() {
    let doc = two_page_document();
    let options = steleofd_render::RenderOptions {
        dpi: 36.0,
        ..Default::default()
    };
    let raster = doc.render_page(0, &options).expect("render page");
    // 210mm at 36dpi is 297.64px, 297mm is 420.71px, both rounded up.
    assert_eq!(raster.width, 298);
    assert_eq!(raster.height, 421);
}

#[cfg(feature = "render")]
#[test]
fn render_rejects_out_of_range_index() {
    let doc = two_page_document();
    let options = steleofd_render::RenderOptions::default();
    let err = doc.render_page(5, &options).expect_err("must fail");
    assert!(
        err.to_string().contains("page index out of range"),
        "got {err}"
    );
}

#[cfg(feature = "render")]
#[test]
fn render_pages_matches_single_page_renders() {
    let doc = two_page_document();
    let options = steleofd_render::RenderOptions {
        dpi: 36.0,
        ..Default::default()
    };
    let batch = doc.render_pages(&[0, 1], &options);
    assert_eq!(batch.len(), 2);
    for (index, result) in batch.iter().enumerate() {
        let raster = result.as_ref().expect("render page");
        let single = doc.render_page(index, &options).expect("render page");
        assert_eq!(raster.width, single.width);
        assert_eq!(raster.height, single.height);
    }
    assert!(doc.render_pages(&[0, 9], &options)[1].is_err());
}

#[cfg(feature = "render")]
#[test]
fn renders_text_with_the_registered_face() {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(&text_page("7", "20.0", "A")).into_bytes(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ];
    let doc = open(&entries);
    let options = steleofd_render::RenderOptions {
        dpi: 36.0,
        ..Default::default()
    };
    let raster = doc.render_page(0, &options).expect("render page");
    let rgba = raster.rgba_bytes();
    // The run sits at x=10mm, baseline y=25mm: scan the box above it.
    let dark = (12..50).any(|x| {
        (5..35).any(|y| {
            let offset = ((y * raster.width + x) * 4) as usize;
            let (r, g, b) = (
                rgba[offset] as u32,
                rgba[offset + 1] as u32,
                rgba[offset + 2] as u32,
            );
            r + g + b < 300
        })
    });
    assert!(dark, "the glyph is outlined from the registered font");
}

#[cfg(feature = "convert")]
#[test]
fn converts_page_to_svg() {
    let doc = two_page_document();
    let svg = doc.to_svg(0).expect("convert to svg");
    assert!(svg.contains("<svg"));
    assert!(svg.contains("width=\"210mm\" height=\"297mm\""));
    assert!(svg.contains("viewBox=\"0 0 210 297\""));
}

#[cfg(feature = "convert")]
#[test]
fn svg_conversion_rejects_out_of_range_index() {
    let doc = two_page_document();
    let err = doc.to_svg(4).expect_err("must fail");
    assert!(
        err.to_string().contains("page index out of range"),
        "got {err}"
    );
}

#[cfg(feature = "convert")]
#[test]
fn converts_page_to_png() {
    let doc = two_page_document();
    let options = steleofd_render::RenderOptions {
        dpi: 36.0,
        ..Default::default()
    };
    let png = doc.to_png(0, &options).expect("convert to png");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG magic bytes");
}

#[cfg(feature = "pdf")]
#[test]
fn converts_document_to_pdf() {
    let doc = two_page_document();
    let pdf = doc.to_pdf().expect("convert to pdf");
    assert!(pdf.starts_with(b"%PDF-"), "PDF header");
    assert!(pdf.windows(5).any(|w| w == b"%%EOF"), "PDF trailer");
}

#[cfg(feature = "print")]
#[test]
fn plans_print_job_over_the_document() {
    let doc = two_page_document();
    let job = doc
        .print_job(&steleofd_print::PrintSettings::default())
        .expect("plan job");
    assert_eq!(job.sheet_count(), 2, "one sheet per page, duplex off");
}

#[cfg(feature = "print")]
#[test]
fn renders_print_job() {
    let doc = two_page_document();
    let settings = steleofd_print::PrintSettings {
        selection: steleofd_print::PageSelection::parse("1").expect("selection"),
        dpi: 36.0,
        ..Default::default()
    };
    let pages = doc.print_render(&settings).expect("render job");
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].width, 298);
    assert_eq!(pages[0].height, 421);
}

#[cfg(feature = "search")]
#[test]
fn finds_text_across_pages() {
    let entries = [
        ("OFD.xml", OFD_XML.as_bytes().to_vec()),
        (
            "Doc_0/Document.xml",
            document_xml(&format!(
                "{}<Page ID=\"2\">{}</Page>",
                text_page("7", "4.0", "Hello"),
                text_page("7", "4.0", "World")
            ))
            .into_bytes(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", font_bytes()),
    ];
    let doc = open(&entries);
    let hits = doc.search("Hello", steleofd_search::SearchOptions::default());
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].page, 0);
    let missing = doc.search("goodbye", steleofd_search::SearchOptions::default());
    assert!(missing.is_empty());
}

#[cfg(feature = "edit")]
#[test]
fn edit_session_wraps_a_copy_of_the_model() {
    let doc = two_page_document();
    let session = doc.edit(steleofd_edit::EditOptions::default());
    assert_eq!(session.document().pages.pages.len(), doc.page_count());
    assert!(!session.can_undo());
    assert_eq!(session.history_len(), 0);
}

#[cfg(feature = "sign")]
#[test]
fn reports_when_no_signatures_are_declared() {
    let mut doc = two_page_document();
    let err = doc.signatures().expect_err("must fail");
    assert!(
        err.to_string().contains("declares no signatures"),
        "got {err}"
    );
    let err = doc.verify_signatures().expect_err("must fail");
    assert!(matches!(err, OfdError::Format(_)), "got {err:?}");
}

#[cfg(feature = "archive")]
#[test]
fn audits_the_container() {
    let mut doc = two_page_document();
    let report = doc.audit().expect("audit container");
    assert!(report.findings.iter().all(|f| !f.detail.is_empty()));
}
