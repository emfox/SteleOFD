//! A `TextCode` whose offsets are fewer than its glyphs, which is how a
//! producer keeps a line of digits apart: the offsets place the glyphs rather
//! than step over them, and the two readings have to be told apart.

use sofd_display::DrawOp;
use sofd_package::{Package, build_archive};
use sofd_parser::Parser;

const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

/// Flatten a page holding one text object with `code` inside it, and return
/// where each glyph of the only run starts, in object space.
fn starts(object_attrs: &str, code: &str) -> Vec<(f64, f64)> {
    runs(object_attrs, code)
        .into_iter()
        .next()
        .expect("a run on the page")
}

/// Every glyph of a code, run after run, for a case whose code is cut into
/// rows rather than drawn as one run.
fn every_glyph(object_attrs: &str, code: &str) -> Vec<(f64, f64)> {
    runs(object_attrs, code).into_iter().flatten().collect()
}

/// Where every run of a page starts, run by run, so a test can tell a code
/// that was cut into runs from one that was not.
fn runs(object_attrs: &str, code: &str) -> Vec<Vec<(f64, f64)>> {
    let page = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData><PageArea><PhysicalBox>0 0 210 140</PhysicalBox></PageArea></CommonData>
  <Pages><Page ID="1"><Content><Layer>
    <TextObject ID="10" Boundary="0 0 100 30" Font="3" Size="5">{object_attrs}
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
        let mut placed = Vec::new();
        for i in 0..run.glyphs.len() {
            if i > 0 {
                at += run.advances.get(i - 1).copied().unwrap_or(0.0);
            }
            placed.push(run.transform.apply(at, 0.0));
        }
        out.push(placed);
    }
    out
}

fn xs(placed: &[(f64, f64)]) -> Vec<f64> {
    placed.iter().map(|(x, _)| *x).collect()
}

/// The x of every glyph, to four places: a walk over a list of offsets adds up
/// rounding the page's own decimal steps do not have.
fn rounded_xs(placed: &[(f64, f64)]) -> Vec<f64> {
    xs(placed)
        .into_iter()
        .map(|x| (x * 10_000.0).round() / 10_000.0)
        .collect()
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

/// `g count value` repeats one offset `count` times, so a list that states a
/// carriage return also covers the steps between its glyphs. An invoice writes
/// `DeltaX="g 11 1.5875 -17.4625"` for a line of digits.
#[test]
fn a_repeated_offset_runs_as_long_as_its_count() {
    // Twelve glyphs and twelve offsets, a repeat and a return. This long a list
    // is read as steps, so the glyphs run a column apart; a repeat read as one
    // offset would end it a step short, which reads it as places.
    let placed = starts(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="g 11 1.5875 -17.4625">050001700111</TextCode>"#,
    );
    let columns: Vec<f64> = (0..12)
        .map(|i| ((i as f64 * 1.5875) * 10_000.0).round() / 10_000.0)
        .collect();
    assert_eq!(rounded_xs(&placed), columns);
}

#[test]
fn the_last_offset_is_a_step_the_line_does_not_take() {
    // A list this long is read as steps, and the step past the last glyph is
    // one the line would take for a glyph that is not there.
    let placed = every_glyph(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="2 2 -4 2 2">abcdef</TextCode>"#,
    );
    assert_eq!(xs(&placed), vec![0.0, 2.0, 4.0, 0.0, 2.0, 4.0]);
}

/// Where a run of a code that was cut into runs draws each of its glyphs.
#[test]
fn a_later_run_of_a_stepped_code_starts_where_its_offsets_have_come_to() {
    // Two glyphs and one offset, exactly the step between them, so this list is
    // read as steps. Cutting the code says nothing about the offsets, so the
    // second run picks up the ground the first covered instead of starting over.
    let object = r#"<CGTransform CodePosition="0" CodeCount="1" GlyphCount="1"><Glyphs>7</Glyphs></CGTransform>"#;
    let page = runs(object, r#"<TextCode X="0" Y="0" DeltaX="8">ab</TextCode>"#);
    assert_eq!(page.len(), 2, "the transformed character, then the rest");
    assert_eq!(xs(&page[0]), vec![0.0], "the first character");
    assert_eq!(xs(&page[1]), vec![8.0], "the second, one step on");
}

/// The offsets of a list that places its glyphs are where each glyph sits, so
/// a glyph that takes no room of its own is still reached at its own offset.
#[test]
fn a_glyph_after_a_space_sits_where_its_offset_puts_it() {
    // Six characters and four offsets, short of the five steps between them, so
    // the list places them at 6, 9, 15 and 21. The last place the list gives is
    // 21, so `d` and `e` follow it a size apart.
    let placed = starts(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="6 9 15 21">a bcde</TextCode>"#,
    );
    assert_eq!(xs(&placed), vec![6.0, 9.0, 15.0, 21.0, 26.0, 31.0]);
}

/// A `CGTransform` can cut a stretch out of a placing list and leave it with
/// no glyph to draw. The offsets have already put what follows, so an empty
/// stretch must not carry that ground a second time.
#[test]
fn a_stretch_left_without_glyphs_does_not_move_the_one_after_it() {
    // The space at index 1 is cut out into a stretch naming no glyphs; the
    // placing offsets put `b` and `c` at 15 and 21 whether it is there or not.
    let object = r#"<CGTransform CodePosition="1" CodeCount="1" GlyphCount="0"></CGTransform>"#;
    let page = runs(
        object,
        r#"<TextCode X="0" Y="0" DeltaX="6 9 15 21">a bcde</TextCode>"#,
    );
    assert_eq!(xs(&page[0]), vec![6.0], "the character before the cut");
    assert_eq!(
        xs(&page[1]),
        vec![15.0, 21.0, 26.0, 31.0],
        "the characters after it, where the offsets put them"
    );
}

/// A `CGTransform` cuts a code into one run per stretch. The offset that
/// reached the second stretch is only in the display list as the first run's
/// end, so a stretch without its span starts both at the code's X.
#[test]
fn a_later_stretch_starts_where_the_one_before_it_ended() {
    let transform = |position: usize, glyphs: &str| {
        format!(
            r#"<CGTransform CodePosition="{position}" CodeCount="2" GlyphCount="2">
                 <Glyphs>{glyphs}</Glyphs>
               </CGTransform>"#
        )
    };
    let object = format!("{}{}", transform(0, "65 66"), transform(2, "67 68"));
    let page = runs(
        &object,
        r#"<TextCode X="0" Y="0" DeltaX="5 5 4 4">abcd</TextCode>"#,
    );
    assert_eq!(page.len(), 2, "one run per transform");
    assert_eq!(
        xs(&page[0]),
        vec![0.0, 5.0],
        "the first stretch at the origin"
    );
    assert_eq!(
        xs(&page[1]),
        vec![10.0, 14.0],
        "the second stretch after the first, and stepped by its own offsets"
    );
}

/// A line that returns to the left edge starts the next row, and DeltaY says
/// how far down it sits. One run cannot carry the return, so the code is drawn
/// as one run per row.
#[test]
fn a_line_that_returns_starts_a_row_below() {
    let page = runs(
        "",
        r#"<TextCode X="3" Y="5" DeltaX="g 2 2 -4 g 2 2 -4 g 2 2"
                    DeltaY="g 2 0 4.5 g 2 0 4.5 g 2 0">abcdefghi</TextCode>"#,
    );
    assert_eq!(page.len(), 3, "three rows of three: {page:?}");
    for (row, placed) in page.iter().enumerate() {
        assert_eq!(xs(placed), vec![3.0, 5.0, 7.0], "row {row} columns");
        let across = ys(placed);
        assert!(
            across.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9),
            "row {row} keeps its own height: {across:?}"
        );
        let first = placed[0].1;
        assert!(
            (first - (5.0 + row as f64 * 4.5)).abs() < 1e-9,
            "row {row} sits below the one above by 4.5, got {first}"
        );
    }
}
