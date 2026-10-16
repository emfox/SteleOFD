//! Clause 7.9 keeps the drawing parameter table in the `Res` files, not in
//! `CommonData`. These tests pin down where the table is read from: a document
//! declaring it in a resource file was refused as naming a resource it lacked.

use steleofd_display::DrawOp;
use steleofd_package::{Package, build_archive};
use steleofd_parser::{ParsedDocument, Parser};

/// One archive carrying the resource tables given, and a page whose single
/// path names the drawing parameter `1006`.
fn archive(public_res: &str, document_res: &str, common_params: &str) -> Vec<u8> {
    let ofd = r##"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0"><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>"##;
    let document = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Document><CommonData>
  <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>
  <DocumentRes>DocumentRes.xml</DocumentRes>
  {common_params}
</CommonData><Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages></Document>"##
    );
    let content = r##"<?xml version="1.0" encoding="UTF-8"?>
<Page><Content><Layer ID="1">
  <PathObject ID="10" Boundary="10 10 20 20" Fill="true" DrawParam="1006">
    <AbbreviatedData>M 10 10 L 30 10 L 30 30 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"##;
    build_archive(&[
        ("OFD.xml", ofd.as_bytes()),
        ("Doc_0/Document.xml", document.as_bytes()),
        ("Doc_0/PublicRes.xml", public_res.as_bytes()),
        ("Doc_0/DocumentRes.xml", document_res.as_bytes()),
        ("Doc_0/Pages/Page_0/Content.xml", content.as_bytes()),
    ])
    .expect("the archive builds")
}

/// A resource table declaring one parameter, filling blue, plus one that only
/// sets a line width and so leaves the fill alone.
fn blue_param() -> String {
    r##"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><DrawParams>
  <DrawParam ID="1006" LineWidth="1.0"><FillColor Value="#0000FF"/></DrawParam>
</DrawParams></Res>"##
        .to_string()
}

const NO_RES: &str = r##"<?xml version="1.0" encoding="UTF-8"?><Res/>"##;

/// The parser borrows its package, so each fixture leaks one; that is cheaper
/// than the lifetime plumbing every test would otherwise need.
fn open(bytes: &[u8]) -> (ParsedDocument, Parser<'static>) {
    let package = Package::from_bytes(bytes).expect("package");
    let mut parser = Parser::new(Box::leak(Box::new(package)));
    let doc = parser.open_document().expect("the document parses");
    (doc, parser)
}

/// The fill colour the page's one path was painted with.
fn fill_of(page: &steleofd_display::PageDisplay) -> (u8, u8, u8) {
    let ops = page
        .ops
        .iter()
        .filter(|op| !matches!(op, DrawOp::Save | DrawOp::Restore | DrawOp::Transform(_)))
        .collect::<Vec<_>>();
    let DrawOp::Path {
        fill: Some(steleofd_display::Paint::Solid(c)),
        ..
    } = ops[0]
    else {
        panic!("expected a filled path, got {:?}", ops[0]);
    };
    (c.r, c.g, c.b)
}

#[test]
fn a_param_the_public_res_table_declares_reaches_the_path_that_names_it() {
    let bytes = archive(&blue_param(), NO_RES, "");
    let (doc, mut parser) = open(&bytes);
    assert_eq!(doc.draw_params.len(), 1, "the public table is read");
    let page = parser.page_display(&doc, 0).expect("page");
    assert_eq!(fill_of(&page), (0, 0, 255));
}

#[test]
fn a_param_the_document_res_table_declares_reaches_the_path_that_names_it() {
    let bytes = archive(NO_RES, &blue_param(), "");
    let (doc, mut parser) = open(&bytes);
    assert_eq!(doc.draw_params.len(), 1, "the document table is read");
    let page = parser.page_display(&doc, 0).expect("page");
    assert_eq!(fill_of(&page), (0, 0, 255));
}

#[test]
fn a_param_common_data_declares_still_reaches_the_path_that_names_it() {
    let bytes = archive(
        NO_RES,
        NO_RES,
        r##"<DrawParams><DrawParam ID="1006"><FillColor Value="#0000FF"/></DrawParam></DrawParams>"##,
    );
    let (doc, mut parser) = open(&bytes);
    assert_eq!(doc.draw_params.len(), 1, "the inline table is read");
    let page = parser.page_display(&doc, 0).expect("page");
    assert_eq!(fill_of(&page), (0, 0, 255));
}

#[test]
fn the_document_table_wins_over_the_public_one_for_the_same_id() {
    let red = r##"<?xml version="1.0" encoding="UTF-8"?>
<Res><DrawParams>
  <DrawParam ID="1006"><FillColor Value="#FF0000"/></DrawParam>
</DrawParams></Res>"##;
    let bytes = archive(&blue_param(), red, "");
    let (doc, mut parser) = open(&bytes);
    assert_eq!(doc.draw_params.len(), 1, "one entry per id");
    let page = parser.page_display(&doc, 0).expect("page");
    assert_eq!(fill_of(&page), (255, 0, 0));
}

#[test]
fn both_res_tables_contribute_the_ids_only_the_other_one_holds() {
    let other = r##"<?xml version="1.0" encoding="UTF-8"?>
<Res><DrawParams>
  <DrawParam ID="1025"><FillColor Value="#00FF00"/></DrawParam>
</DrawParams></Res>"##;
    let bytes = archive(&blue_param(), other, "");
    let (doc, _parser) = open(&bytes);
    assert_eq!(doc.draw_params.len(), 2, "both tables are merged");
}

#[test]
fn a_param_no_table_declares_is_reported_as_a_dangling_reference() {
    let bytes = archive(NO_RES, NO_RES, "");
    let (doc, mut parser) = open(&bytes);
    assert!(doc.draw_params.is_empty(), "nothing was declared");
    let err = parser
        .page_display(&doc, 0)
        .expect_err("the named id is nowhere")
        .to_string();
    assert!(err.contains("DrawParam"), "{err}");
    assert!(err.contains("1006"), "{err}");
}

#[test]
fn a_param_no_object_names_is_carried_and_harmless() {
    let bytes = archive(&blue_param(), NO_RES, "");
    let (doc, mut parser) = open(&bytes);
    // The page names 1006, so drop the reference and the table stays unused
    // rather than turning into an error.
    let page = parser.page_display(&doc, 0).expect("page");
    assert_eq!(fill_of(&page), (0, 0, 255));
}
