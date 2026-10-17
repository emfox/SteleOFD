//! A `TextCode` whose offsets are fewer than its glyphs, which is how a
//! producer keeps a line of digits apart: the offsets place the glyphs rather
//! than step over them, and the two readings have to be told apart.

use steleofd_display::DrawOp;
use steleofd_package::{Package, build_archive};
use steleofd_parser::Parser;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

/// Flatten a page holding one text object with `code` inside it, and return
/// where each glyph of the only run starts, in object space.
fn starts(object_attrs: &str, code: &str) -> Vec<(f64, f64)> {
    let page = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData><PageArea><PhysicalBox>0 0 210 140</PhysicalBox></PageArea></CommonData>
  <Pages><Page ID="1"><Content><Layer>
    <TextObject ID="10" Boundary="0 0 100 30" Font="3" Size="5" {object_attrs}>
      {code}
    </TextObject>
  </Layer></Content></Page></Pages>
</Document>"#
    );
    let entries: Vec<(&str, &[u8])> = vec![
        ("OFD.xml", OFD_XML.as_bytes()),
        ("Doc_0/Document.xml", page.as_bytes()),
    ];
    let bytes = build_archive(&entries).expect("archive");
    let package = Package::from_bytes(&bytes).expect("package");
    let mut parser = Parser::new(Box::leak(Box::new(package)));
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let mut out = Vec::new();
    for op in &page.ops {
        let DrawOp::Text(run) = op else { continue };
        let mut at = 0.0;
        for i in 0..run.glyphs.len() {
            out.push(run.transform.apply(at, 0.0));
            at += run.advances.get(i).copied().unwrap_or(0.0);
        }
    }
    out
}

fn xs(placed: &[(f64, f64)]) -> Vec<f64> {
    placed.iter().map(|(x, _)| *x).collect()
}

fn ys(placed: &[(f64, f64)]) -> Vec<f64> {
    placed.iter().map(|(_, y)| *y).collect()
}

/// Two glyphs run together the moment a glyph is placed by the offset before
/// it rather than stepped over by it, so every case below asserts both.
#[test]
fn an_offset_per_step_advances_every_glyph_by_its_own() {
    // Three glyphs have two steps between them, so this list reaches the end
    // and is read as the standard says: each entry moves the next glyph.
    let placed = starts("", r#"<TextCode X="0" Y="0" DeltaX="2 3">abc</TextCode>"#);
    assert_eq!(xs(&placed), vec![0.0, 2.0, 5.0]);
    assert_eq!(ys(&placed), vec![0.0, 0.0, 0.0]);
}

#[test]
fn offsets_shorter_than_the_steps_place_glyphs_in_columns() {
    // Seven glyphs and six offsets, one short of the steps between them, so
    // the offsets place the glyphs: every glyph but the last steps on by its
    // offset.
    let placed = starts(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="2 2 2 2 2 2">abcdefg</TextCode>"#,
    );
    assert_eq!(xs(&placed), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
}

#[test]
fn a_single_glyph_needs_no_step_and_keeps_its_origin() {
    let placed = starts("", r#"<TextCode X="4" Y="3" DeltaX="9 9">a</TextCode>"#);
    assert_eq!(placed, vec![(4.0, 3.0)]);
}

#[test]
fn a_negative_offset_turns_the_line_back_and_moves_down() {
    // The offsets place the first glyphs and the negative one starts a new
    // row, which is the same step whether the list is being read as steps or
    // as positions.
    let placed = starts(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="2 2 -20" DeltaY="0 0 5">abcdef</TextCode>"#,
    );
    // The negative entry is a position like any other, so the glyph it reaches
    // starts at the left edge again, one row on, and the glyphs after it step.
    assert_eq!(xs(&placed), vec![0.0, 2.0, 4.0, 4.0, 9.0, 14.0]);
}
