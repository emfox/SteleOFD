//! Conformance tests bound to GB/T 33190-2016 clauses.
//! Names map to entries in compliance/gb-t-33190.toml.

use steleofd_common::Rect;
use steleofd_display::DrawOp;
use steleofd_display::ops::Paint;
use steleofd_geometry::Affine;
use steleofd_package::{Package, build_archive};
use steleofd_parser::Parser;
use steleofd_parser::pathdata::parse_path;

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

fn open(entries: &[(&str, String)]) -> Parser<'static> {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, s)| (*p, s.as_bytes())).collect();
    let bytes = build_archive(&borrowed).expect("build fixture");
    // Test-only leak: the package must outlive the parser borrowing it.
    let package = Box::new(Package::from_bytes(&bytes).expect("open package"));
    Parser::new(Box::leak(package))
}

#[test]
fn gb_t_33190_7_3_physical_box_is_element_text() {
    // ST_Box carries its four numbers as element text; the attribute form is
    // what sloppy writers emit, so both must parse to the same rectangle.
    let text_form = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox>0 0 210 297</PhysicalBox></PageArea>
  </CommonData>
  <Pages><Page ID="1"/></Pages>
</Document>"#;
    let mut parser = open(&[
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", text_form.to_string()),
    ]);
    let doc = parser.open_document().expect("open document");
    assert_eq!(
        doc.document.common_data.page_area.physical_box,
        Rect::new(0.0, 0.0, 210.0, 297.0)
    );

    let mut parser = open(&[
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", document_xml(r#"<Page ID="1"/>"#)),
    ]);
    let doc = parser.open_document().expect("open document");
    assert_eq!(
        doc.document.common_data.page_area.physical_box,
        Rect::new(0.0, 0.0, 210.0, 297.0)
    );
}

#[test]
fn gb_t_33190_7_4_main_entry_is_required() {
    let bytes = build_archive(&[("Doc_0/Document.xml", b"<Document/>".as_slice())])
        .expect("archive without OFD.xml");
    let mut package = Package::from_bytes(&bytes).expect("open package");
    let mut parser = Parser::new(&mut package);
    match parser.open_document() {
        Ok(_) => panic!("an archive without OFD.xml must be rejected"),
        Err(err) => assert!(err.to_string().contains("not a valid OFD"), "got {err}"),
    }
}

#[test]
fn gb_t_33190_7_5_document_root_requires_pages() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", document_xml("")),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    assert!(
        doc.document.pages.pages.is_empty(),
        "an empty Pages element is allowed"
    );
    assert_eq!(
        doc.doc_dir, "Doc_0",
        "the document directory is the root's parent"
    );
}

#[test]
fn gb_t_33190_7_4_version_attribute_is_read() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", document_xml(r#"<Page ID="1"/>"#)),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    assert_eq!(doc.ofd.version.major, "1");
    assert_eq!(doc.ofd.version.sub.as_deref(), Some("0"));
}

#[test]
fn gb_t_33190_7_6_page_tree_lists_pages_in_order() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1"/><Page ID="2"/><Page ID="3"/>"#),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let ids: Vec<&str> = doc
        .document
        .pages
        .pages
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(ids, vec!["1", "2", "3"]);
}

#[test]
fn gb_t_33190_7_7_page_content_may_live_in_base_loc() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#),
        ),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Page><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10">
    <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    assert!(page.ops.iter().any(|op| matches!(op, DrawOp::Path { .. })));
}

#[test]
fn gb_t_33190_8_1_3_page_area_defaults_from_common_data() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", document_xml(r#"<Page ID="1"/>"#)),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    assert_eq!(page.page_box.w, 210.0);
    assert_eq!(page.page_box.h, 297.0);
}

#[test]
fn gb_t_33190_8_1_5_ctm_orders_rows_as_a_b_c_d_e_f() {
    // a b c d e f applied as x' = a*x + c*y + e, y' = b*x + d*y + f.
    let m = Affine::new(2.0, 0.0, 0.0, 3.0, 5.0, 7.0);
    let (x, y) = m.apply(1.0, 1.0);
    assert_eq!((x, y), (7.0, 10.0));
    let shear = Affine::new(1.0, 0.0, 1.0, 1.0, 0.0, 0.0);
    let (x, y) = shear.apply(2.0, 1.0);
    assert_eq!((x, y), (3.0, 1.0));
}

#[test]
fn gb_t_33190_8_3_2_colour_is_rgb_with_alpha() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(
                r##"<Page ID="1"><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10">
    <FillColor Value="#ff0000" Alpha="128"/>
    <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"##,
            ),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let fill = page.ops.iter().find_map(|op| match op {
        DrawOp::Path { fill, .. } => fill.clone(),
        _ => None,
    });
    let fill = fill.expect("filled path");
    match fill {
        Paint::Solid(color) => {
            assert_eq!((color.r, color.g, color.b), (255, 0, 0));
            assert_eq!(color.a, 128, "alpha is carried alongside RGB");
        }
        other => panic!("expected a solid colour, got {other:?}"),
    }
}

#[test]
fn gb_t_33190_8_5_object_carries_boundary_and_visibility() {
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(
                r#"<Page ID="1"><Content><Layer>
  <PathObject ID="10" Boundary="1 2 30 40" Visible="false">
    <AbbreviatedData>M 0 0 L 10 0 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"#,
            ),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    assert!(page.ops.is_empty(), "an invisible object draws nothing");
    assert_eq!(doc.document.pages.pages[0].id, "1");
}

#[test]
fn gb_t_33190_9_3_2_move_starts_a_subpath() {
    let path = parse_path("M 5 6").expect("parse");
    assert_eq!(path.len(), 1, "a move is a segment of its own");
}

#[test]
fn gb_t_33190_9_3_3_line_after_move_extends_the_path() {
    let path = parse_path("M 0 0 L 10 0 L 10 10").expect("parse");
    assert_eq!(path.len(), 3);
    let box_rect = path.bounding_box();
    assert_eq!((box_rect.x, box_rect.y), (0.0, 0.0));
    assert_eq!(box_rect.w, 10.0);
}

#[test]
fn gb_t_33190_9_3_4_cubic_and_quadratic_segments() {
    let cubic = parse_path("M 0 0 C 1 1 2 2 3 3").expect("parse cubic");
    assert_eq!(cubic.len(), 2);
    let quad = parse_path("M 0 0 Q 1 1 2 2").expect("parse quadratic");
    assert_eq!(quad.len(), 2);
}

#[test]
fn gb_t_33190_9_3_5_arc_is_split_into_cubics() {
    let path = parse_path("M 0 0 A 10 10 0 0 1 20 0").expect("parse arc");
    assert!(path.len() >= 2, "an arc becomes one or more cubic segments");
    let box_rect = path.bounding_box();
    assert!(box_rect.w > 0.0 && box_rect.h > 0.0, "the arc has extent");
}

#[test]
fn gb_t_33190_10_image_object_requires_resource_id() {
    let xml = document_xml(
        r#"<Page ID="1"><Content><Layer>
  <ImageObject ID="10" Boundary="0 0 10 10" ResourceID="5"/>
  <ImageObject ID="11" Boundary="0 0 10 10"/>
</Layer></Content></Page>"#,
    );
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let images = page
        .ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Image { .. }))
        .count();
    assert_eq!(images, 1, "only the object with a ResourceID is placed");
}

#[test]
fn gb_t_33190_11_2_text_object_keeps_font_size_and_codes() {
    let xml = document_xml(
        r#"<Page ID="1"><Content><Layer>
  <TextObject ID="10" Boundary="0 0 50 10" Font="3" Size="4.0">
    <TextCode X="1" Y="2">Hi</TextCode>
  </TextObject>
</Layer></Content></Page>"#,
    );
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let run = page.ops.iter().find_map(|op| match op {
        DrawOp::Text(run) => Some(run),
        _ => None,
    });
    let run = run.expect("text run");
    assert_eq!(run.text.as_deref(), Some("Hi"));
    assert_eq!(run.size.0, 4.0);
    assert_eq!(run.glyphs.len(), 2);
}

/// The stroke style of the first `Path` op the page produces, with the given
/// draw parameter declared in `CommonData`.
fn stroke_style(content: &str, param: &str) -> steleofd_display::ops::StrokeStyle {
    let body = document_xml(&format!(
        r#"<Page ID="1"><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10" Stroke="true" DrawParam="d1">
    <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
    {content}
  </PathObject>
</Layer></Content></Page>"#
    ));
    // Splice the DrawParams block into the CommonData element document_xml
    // already wrote, since a DrawParam is only reachable from there.
    let params = format!(r#"<DrawParams><DrawParam ID="d1" {param}/></DrawParams>"#);
    let doc_xml = body.replace("</CommonData>", &format!("{params}</CommonData>"));
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", doc_xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    page.ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Path { stroke_style, .. } => Some(stroke_style.clone()),
            _ => None,
        })
        .expect("stroke path")
}

#[test]
fn gb_t_33190_8_2_1_draw_param_resolves_from_its_id() {
    // A DrawParam is named by ID and reached through the object's DrawParam
    // attribute; LineWidth travels with it.
    let style = stroke_style("", r#"LineWidth="0.5" Cap="Round""#);
    assert_eq!(style.width, 0.5);
    assert_eq!(style.cap, steleofd_display::ops::LineCap::Round);
}

#[test]
fn gb_t_33190_8_2_2_line_join_maps_to_the_named_style() {
    use steleofd_display::ops::LineJoin;
    assert_eq!(stroke_style("", r#"Join="Round""#).join, LineJoin::Round);
    assert_eq!(stroke_style("", r#"Join="Bevel""#).join, LineJoin::Bevel);
    assert_eq!(stroke_style("", r#"Join="Miter""#).join, LineJoin::Miter);
    // An unknown keyword falls back to the default rather than failing the
    // document, matching how the standard treats absent values.
    assert_eq!(stroke_style("", r#"Join="Wedge""#).join, LineJoin::Miter);
}

#[test]
fn gb_t_33190_8_2_4_dash_pattern_carries_its_values_and_offset() {
    // DashPattern is an ST_Array attribute, and DashOffset shifts the phase
    // into it; neither is a child element.
    let with_dash = stroke_style("", r#"LineWidth="1" DashPattern="3 1 2" DashOffset="1.5""#);
    assert_eq!(with_dash.dash, vec![3.0, 1.0, 2.0]);
    assert_eq!(with_dash.dash_offset, 1.5);

    // No DashPattern at all is a solid line, not a missing value.
    let solid = stroke_style("", r#"LineWidth="1""#);
    assert!(solid.dash.is_empty());
    assert_eq!(solid.dash_offset, 0.0);
}

#[test]
fn gb_t_33190_8_2_5_line_cap_maps_to_the_named_style() {
    use steleofd_display::ops::LineCap;
    assert_eq!(stroke_style("", r#"Cap="Butt""#).cap, LineCap::Butt);
    assert_eq!(stroke_style("", r#"Cap="Round""#).cap, LineCap::Round);
    assert_eq!(stroke_style("", r#"Cap="Square""#).cap, LineCap::Square);
    assert_eq!(
        stroke_style("", r#"LineWidth="2""#).cap,
        LineCap::Butt,
        "an absent cap is Butt, which is what the standard defaults to"
    );
}

#[test]
fn gb_t_33190_9_2_rule_selects_the_fill_rule() {
    use steleofd_display::ops::PaintRule;
    let rule_of = |attr: &str| -> PaintRule {
        let page_xml = document_xml(&format!(
            r#"<Page ID="1"><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10" Fill="true" {attr}>
    <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"#
        ));
        let entries = [
            ("OFD.xml", OFD_XML.to_string()),
            ("Doc_0/Document.xml", page_xml),
        ];
        let mut parser = open(&entries);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("flatten page");
        page.ops
            .iter()
            .find_map(|op| match op {
                DrawOp::Path { rule, .. } => Some(*rule),
                _ => None,
            })
            .expect("path op")
    };
    // The standard spells the two values `NonZero` and `Even-Odd`, and defaults
    // to `NonZero` when the attribute is absent.
    assert_eq!(rule_of(r#"Rule="Even-Odd""#), PaintRule::EvenOdd);
    assert_eq!(rule_of(r#"Rule="NonZero""#), PaintRule::NonZero);
    assert_eq!(rule_of(""), PaintRule::NonZero);
}

/// The first `Text` op a page with one text object produces.
fn glyph_run(object_attrs: &str, code: &str) -> steleofd_display::text::GlyphRun {
    let page_xml = document_xml(&format!(
        r#"<Page ID="1"><Content><Layer>
  <TextObject ID="10" Boundary="0 0 100 30" Font="3" Size="5" {object_attrs}>
    {code}
  </TextObject>
</Layer></Content></Page>"#
    ));
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", page_xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    page.ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Text(run) => Some(run.clone()),
            _ => None,
        })
        .expect("text run")
}

#[test]
fn gb_t_33190_11_1_font_reference_and_size_reach_the_run() {
    // The Font attribute is a resource id and Size a magnitude in millimetres;
    // both have to survive into the display list, since the backend looks the
    // face up by that id.
    let run = glyph_run("", r#"<TextCode X="0" Y="0">AB</TextCode>"#);
    assert_eq!(run.font, steleofd_font::FontId(3));
    assert_eq!(run.size.0, 5.0);
    assert_eq!(run.glyphs.len(), 2);
}

#[test]
fn gb_t_33190_11_3_text_code_origin_becomes_the_run_transform() {
    // TextCode X/Y is the origin of that line within the object; the run's
    // transform carries it so the renderer never re-reads the XML.
    let run = glyph_run("", r#"<TextCode X="12" Y="7">A</TextCode>"#);
    let (x, y) = run.transform.apply(0.0, 0.0);
    assert_eq!((x, y), (12.0, 7.0));
}

#[test]
fn gb_t_33190_11_3_char_space_widens_the_advance() {
    let run = glyph_run(r#"CharSpace="2""#, r#"<TextCode X="0" Y="0">AB</TextCode>"#);
    assert_eq!(run.letter_spacing, 2.0, "CharSpace rides on the run");
}

#[test]
fn gb_t_33190_11_3_delta_attributes_adjust_each_advance() {
    // DeltaX and DeltaY are per-glyph adjustments; the advance of glyph i is
    // the size plus that glyph's deltas.
    let run = glyph_run(
        "",
        r#"<TextCode X="0" Y="0" DeltaX="1 2 3" DeltaY="0.5 0.5 0.5">ABC</TextCode>"#,
    );
    assert_eq!(run.advances.len(), 3);
    assert_eq!(run.advances[0], 6.5);
    assert_eq!(run.advances[1], 7.5);
    assert_eq!(run.advances[2], 8.5);
}

#[test]
fn gb_t_33190_11_4_h_scale_is_applied_to_the_run_transform() {
    // HScale stretches the glyphs horizontally without touching the advance;
    // it lands in the transform, next to the origin.
    let run = glyph_run(r#"HScale="2""#, r#"<TextCode X="10" Y="0">A</TextCode>"#);
    let (x, y) = run.transform.apply(1.0, 0.0);
    assert_eq!(
        (x, y),
        (12.0, 0.0),
        "a unit of glyph space is twice as wide"
    );
    let (ox, oy) = run.transform.apply(0.0, 0.0);
    assert_eq!((ox, oy), (10.0, 0.0), "the origin itself does not move");
}

#[test]
fn gb_t_33190_7_8_text_may_be_stroked_as_an_outline() {
    // A text object defaults to filled and unstroked; Stroke="true" asks for
    // the glyph outlines to be drawn as well.
    let plain = glyph_run("", r#"<TextCode X="0" Y="0">A</TextCode>"#);
    assert!(plain.fill.is_some(), "text is filled by default");
    assert!(plain.stroke.is_none(), "and not stroked");

    let outlined = glyph_run(
        r##"Stroke="true" Fill="false" StrokeColor="#0000FF""##,
        r#"<TextCode X="0" Y="0">A</TextCode>"#,
    );
    assert!(outlined.stroke.is_some(), "Stroke=true outlines the glyphs");
    assert!(
        outlined.fill.is_none(),
        "Fill=false leaves the interiors out"
    );
}
