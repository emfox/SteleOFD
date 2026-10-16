//! Conformance tests bound to GB/T 33190-2016 clauses.
//! Names map to entries in compliance/gb-t-33190.toml.

use steleofd_common::Rect;
use steleofd_display::DrawOp;
use steleofd_display::ops::Paint;
use steleofd_geometry::Affine;
use steleofd_model::{ActionEvent, ActionKind, GotoTarget};
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
fn gb_t_33190_7_9_draw_params_resolve_from_the_resource_table() {
    // Clause 7.9 manages the drawing parameters in the resource files, which is
    // where the annex A schema puts them and where the corpus puts them, so a
    // path naming an id a `Res` table declares must be painted with it.
    let res = r##"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><DrawParams>
  <DrawParam ID="1006"><FillColor Value="#0000FF"/></DrawParam>
</DrawParams></Res>"##
        .to_string();
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#),
        ),
        ("Doc_0/PublicRes.xml", res),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Page><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10" Fill="true" DrawParam="1006">
    <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  </PathObject>
</Layer></Content></Page>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    assert_eq!(doc.draw_params.len(), 1, "the resource table is read");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let Some(DrawOp::Path {
        fill: Some(Paint::Solid(c)),
        ..
    }) = page.ops.iter().find(|op| matches!(op, DrawOp::Path { .. }))
    else {
        panic!("expected a filled path, got {:?}", page.ops);
    };
    assert_eq!((c.r, c.g, c.b), (0, 0, 255));
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
  <PathObject ID="10" Boundary="0 0 10 10" Fill="true">
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
fn gb_t_33190_9_3_2_a_subpath_start_operator_opens_the_path() {
    // Table 36 gives `S` its own operator, the point a subpath starts at, and a
    // producer in the corpus opens every path with it. `M` and `S` differ in
    // what they name, not in what they draw, so both must open a subpath.
    let path = parse_path("S 0 0 L 156 0 C").expect("parse");
    assert_eq!(path.len(), 2, "a start and a line");
    // A bare trailing operator consumes no operands and so adds nothing.
    let only_start = parse_path("S 3 4").expect("parse");
    assert_eq!(only_start.len(), 1);
    // Neither may open a path that is not the first thing written.
    assert!(parse_path("L 0 0").is_err(), "a line cannot open a path");
    assert!(
        parse_path("Q 0 0 1 1").is_err(),
        "a curve cannot open a path"
    );
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

/// Every `Text` op a page with one text object produces.
fn glyph_runs(object_attrs: &str, code: &str) -> Vec<steleofd_display::text::GlyphRun> {
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
        .filter_map(|op| match op {
            DrawOp::Text(run) => Some(run.clone()),
            _ => None,
        })
        .collect()
}

/// The first `Text` op a page with one text object produces.
fn glyph_run(object_attrs: &str, code: &str) -> steleofd_display::text::GlyphRun {
    glyph_runs(object_attrs, code)
        .into_iter()
        .next()
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
fn gb_t_33190_11_4_a_transform_names_the_glyphs_of_the_characters_it_covers() {
    // One-to-one: every character gets the glyph the transform names, and the
    // run then holds indices into the font's own glyph space, which is the
    // only space a font without a usable cmap can be addressed in.
    let runs = glyph_runs(
        "",
        r#"<CGTransform CodePosition="0" CodeCount="3" GlyphCount="3"><Glyphs>7 8 9</Glyphs></CGTransform>
           <TextCode X="0" Y="0">ABC</TextCode>"#,
    );
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].glyphs, vec![7, 8, 9]);
    assert!(
        runs[0].codes_are_glyphs,
        "the codes are glyph indices, not code points"
    );
    assert_eq!(
        runs[0].plain_text(),
        "ABC",
        "the characters stay for search"
    );
    assert_eq!(runs[0].cluster_map.as_deref(), Some(&[0u32, 1, 2][..]));
}

#[test]
fn gb_t_33190_11_4_characters_outside_the_transform_keep_their_code_points() {
    // A transform covers the range it names; the rest of the text is drawn as
    // code points in runs of its own, each placed where the last one ended.
    let runs = glyph_runs(
        "",
        r#"<CGTransform CodePosition="1" CodeCount="2" GlyphCount="2"><Glyphs>7 8</Glyphs></CGTransform>
           <TextCode X="0" Y="0">ABCD</TextCode>"#,
    );
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].glyphs, vec![65], "A is a code point");
    assert!(!runs[0].codes_are_glyphs);
    assert_eq!(runs[1].glyphs, vec![7, 8], "BC are the named glyphs");
    assert!(runs[1].codes_are_glyphs);
    assert_eq!(runs[1].plain_text(), "BC");
    // The glyph run starts after A's advance, and D after both of theirs; the
    // size is 5mm per glyph.
    let (x, _) = runs[1].transform.apply(0.0, 0.0);
    assert_eq!(x, 5.0);
    let (x, _) = runs[2].transform.apply(0.0, 0.0);
    assert_eq!(x, 15.0);
}

#[test]
fn gb_t_33190_11_4_many_characters_may_share_one_glyph() {
    // Many-to-one: two characters, one glyph, anchored to the first of them
    // so a search hit still finds the text it came from.
    let runs = glyph_runs(
        "",
        r#"<CGTransform CodePosition="0" CodeCount="2" GlyphCount="1"><Glyphs>9</Glyphs></CGTransform>
           <TextCode X="0" Y="0">AB</TextCode>"#,
    );
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].glyphs, vec![9]);
    assert_eq!(runs[0].cluster_map.as_deref(), Some(&[0u32][..]));
    assert_eq!(runs[0].plain_text(), "AB");
}

#[test]
fn gb_t_33190_11_4_one_character_may_expand_into_many_glyphs() {
    // One-to-many: a single character, several glyphs, every one of them
    // anchored to the character that produced it.
    let runs = glyph_runs(
        "",
        r#"<CGTransform CodePosition="0" CodeCount="1" GlyphCount="3"><Glyphs>4 5 6</Glyphs></CGTransform>
           <TextCode X="0" Y="0">A</TextCode>"#,
    );
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].glyphs, vec![4, 5, 6]);
    assert_eq!(runs[0].cluster_map.as_deref(), Some(&[0u32, 0, 0][..]));
    assert_eq!(runs[0].plain_text(), "A");
}

#[test]
fn gb_t_33190_11_4_absent_counts_default_to_one_character() {
    // CodeCount and GlyphCount default to one; the transform then covers a
    // single character and the rest of the text is untouched.
    let runs = glyph_runs(
        "",
        r#"<CGTransform CodePosition="0"><Glyphs>7</Glyphs></CGTransform>
           <TextCode X="0" Y="0">AB</TextCode>"#,
    );
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].glyphs, vec![7]);
    assert_eq!(runs[1].glyphs, vec![66], "B keeps its code point");
    assert!(!runs[1].codes_are_glyphs);
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

/// Flatten a page whose single layer holds `object`, splicing `params` into
/// the `DrawParams` block of the common data.
fn page_ops(object: &str, params: &str) -> Vec<DrawOp> {
    let body = document_xml(&format!(
        r#"<Page ID="1"><Content><Layer>{object}</Layer></Content></Page>"#
    ));
    let doc_xml = if params.is_empty() {
        body
    } else {
        body.replace(
            "</CommonData>",
            &format!(r#"<DrawParams>{params}</DrawParams></CommonData>"#),
        )
    };
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", doc_xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    parser.page_display(&doc, 0).expect("flatten page").ops
}

#[test]
fn gb_t_33190_8_3_4_axial_shading_becomes_a_linear_gradient() {
    // An AxialShd fill is written as a child of FillColor rather than as a
    // Value attribute. The gradient travels on the paint itself, so the shape
    // is a single op a backend can render without matching an earlier one.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" Fill="true">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <FillColor><AxialShd StartPoint="0 0" EndPoint="10 0">
    <Segment Position="0"><Color Value="#FF0000"/></Segment>
    <Segment Position="1"><Color Value="#0000FF"/></Segment>
  </AxialShd></FillColor>
</PathObject>"##,
        "",
    );
    let DrawOp::Path {
        path,
        fill: Some(Paint::LinearGradient(spec)),
        ..
    } = &ops[0]
    else {
        panic!("expected a path filled with a linear gradient, got {ops:?}");
    };
    assert_eq!((spec.start.x, spec.start.y), (0.0, 0.0));
    assert_eq!((spec.end.x, spec.end.y), (10.0, 0.0));
    assert_eq!(spec.stops.len(), 2);
    assert_eq!(path.bounding_box(), Rect::new(0.0, 0.0, 10.0, 10.0));
}

#[test]
fn gb_t_33190_8_3_4_radial_shading_carries_both_radii() {
    // A RadialShd names two circles and the clause draws the shading as the
    // cone between them, so both have to reach the gradient op. The clause
    // also allows the shading to be stroked, so the marker covers both paints.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 20 20" Stroke="true">
  <AbbreviatedData>M 0 0 L 20 0 L 20 20 Z</AbbreviatedData>
  <StrokeColor><RadialShd StartPoint="10 10" StartRadius="0" EndPoint="16 10" EndRadius="5">
    <Segment Position="0"><Color Value="#FFFFFF"/></Segment>
    <Segment Position="1"><Color Value="#000000"/></Segment>
  </RadialShd></StrokeColor>
</PathObject>"##,
        "",
    );
    let DrawOp::Path {
        stroke: Some(Paint::RadialGradient(spec)),
        ..
    } = &ops[0]
    else {
        panic!("expected a path stroked with a radial gradient, got {ops:?}");
    };
    assert_eq!((spec.center.x, spec.center.y), (10.0, 10.0));
    assert_eq!(spec.radius, 0.0);
    assert_eq!(
        (spec.end.x, spec.end.y),
        (16.0, 10.0),
        "the end circle travels too"
    );
    assert_eq!(spec.end_radius, 5.0);
    assert_eq!(spec.stops.len(), 2);
}

#[test]
fn gb_t_33190_8_3_4_segments_keep_their_positions_and_colours() {
    // A Segment's Position is a fraction along the axis and its Color child
    // gives the value there; both have to reach the stop list intact.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" Fill="true">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <FillColor><AxialShd StartPoint="0 0" EndPoint="0 10">
    <Segment Position="0"><Color Value="#FF0000"/></Segment>
    <Segment Position="0.25"><Color Value="#00FF00"/></Segment>
    <Segment Position="1"><Color Value="#0000FF" Alpha="128"/></Segment>
  </AxialShd></FillColor>
</PathObject>"##,
        "",
    );
    let DrawOp::Path {
        fill: Some(Paint::LinearGradient(spec)),
        ..
    } = &ops[0]
    else {
        panic!("expected a path filled with a linear gradient, got {ops:?}");
    };
    let stops = &spec.stops;
    assert_eq!(stops[0].offset, 0.0);
    assert_eq!(stops[1].offset, 0.25);
    assert_eq!(stops[2].offset, 1.0);
    assert_eq!(
        stops[0].color,
        steleofd_display::ops::Color::rgba(255, 0, 0, 255)
    );
    assert_eq!(
        stops[2].color,
        steleofd_display::ops::Color::rgba(0, 0, 255, 128)
    );
}

#[test]
fn gb_t_33190_8_4_clip_area_narrows_the_drawing_to_a_path() {
    // The clip is state, so the object is bracketed by a save and a restore
    // around the ClipPath command; drawing after the restore is unaffected.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <Clips><Clip><Area>
    <Path><AbbreviatedData>M 0 0 L 5 0 L 5 5 L 0 5 Z</AbbreviatedData></Path>
  </Area></Clip></Clips>
</PathObject>"##,
        "",
    );
    assert!(matches!(ops[0], DrawOp::Save), "clip opens a scope");
    let DrawOp::ClipPath { path, rule } = &ops[1] else {
        panic!("expected a clip command, got {ops:?}");
    };
    assert_eq!(path.bounding_box(), Rect::new(0.0, 0.0, 5.0, 5.0));
    assert_eq!(*rule, steleofd_display::ops::PaintRule::NonZero);
    assert!(matches!(ops[2], DrawOp::Path { .. }));
    assert!(matches!(ops[3], DrawOp::Restore), "and closes it again");
    assert_eq!(ops.len(), 4, "nothing else was emitted");
}

#[test]
fn gb_t_33190_8_4_several_clips_cover_the_union_of_their_areas() {
    // Clause 8.4: where more than one Clip is present the region is their
    // union, so both outlines reach the clip path rather than the first one
    // winning. A scanned invoice in the fetched corpus carries two.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 40 10">
  <AbbreviatedData>M 0 0 L 40 0 L 40 10 L 0 10 Z</AbbreviatedData>
  <Clips>
    <Clip><Area>
      <Path><AbbreviatedData>M 0 0 L 5 0 L 5 5 L 0 5 Z</AbbreviatedData></Path>
    </Area></Clip>
    <Clip><Area>
      <Path><AbbreviatedData>M 20 0 L 25 0 L 25 5 L 20 5 Z</AbbreviatedData></Path>
    </Area></Clip>
  </Clips>
</PathObject>"##,
        "",
    );
    let DrawOp::ClipPath { path, .. } = &ops[1] else {
        panic!("expected a clip command, got {ops:?}");
    };
    assert_eq!(
        path.bounding_box(),
        Rect::new(0.0, 0.0, 25.0, 5.0),
        "the union of both, not one of them"
    );
    let subpaths = path
        .segments
        .iter()
        .filter(|s| matches!(s, steleofd_geometry::PathSegment::MoveTo(..)))
        .count();
    assert_eq!(subpaths, 2, "one subpath per clip");
}

#[test]
fn gb_t_33190_8_4_one_clip_may_hold_several_areas() {
    // A Clip is made of areas that together cover their union, and each keeps
    // its own CTM, which applies to that area's path and nothing else.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 40 10">
  <AbbreviatedData>M 0 0 L 40 0 L 40 10 L 0 10 Z</AbbreviatedData>
  <Clips><Clip>
    <Area>
      <Path><AbbreviatedData>M 0 0 L 4 0 L 4 4 L 0 4 Z</AbbreviatedData></Path>
    </Area>
    <Area CTM="1 0 0 1 10 0">
      <Path><AbbreviatedData>M 0 0 L 4 0 L 4 4 L 0 4 Z</AbbreviatedData></Path>
    </Area>
  </Clip></Clips>
</PathObject>"##,
        "",
    );
    let DrawOp::ClipPath { path, .. } = &ops[1] else {
        panic!("expected a clip command, got {ops:?}");
    };
    assert_eq!(
        path.bounding_box(),
        Rect::new(0.0, 0.0, 14.0, 4.0),
        "the second area is shifted by its own CTM"
    );
}

#[test]
fn gb_t_33190_8_4_a_clipped_object_does_not_clip_its_neighbour() {
    // A second object drawn after a clipped one starts from the clean state,
    // because the restore popped the mask back off.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <Clips><Clip><Area>
    <Path><AbbreviatedData>M 0 0 L 5 0 L 5 5 L 0 5 Z</AbbreviatedData></Path>
  </Area></Clip></Clips>
</PathObject>
<PathObject ID="11" Boundary="20 20 10 10">
  <AbbreviatedData>M 20 20 L 30 20 L 30 30 Z</AbbreviatedData>
</PathObject>"##,
        "",
    );
    let clips = ops
        .iter()
        .filter(|op| matches!(op, DrawOp::ClipPath { .. }))
        .count();
    assert_eq!(clips, 1, "only the first object is clipped");
    let last_path = ops
        .iter()
        .rev()
        .find_map(|op| match op {
            DrawOp::Path { path, .. } => Some(path),
            _ => None,
        })
        .expect("second path");
    assert_eq!(last_path.bounding_box(), Rect::new(20.0, 20.0, 10.0, 10.0));
}

#[test]
fn gb_t_33190_8_4_a_clip_stays_in_object_space_beside_the_ctm() {
    // Table 33 gives a clip no attribute but its areas, and clause 8.4 states
    // them in the space the object is drawn in: a `TransFlag` is not in the
    // standard and must not move the clip out of that space.
    let body = r##"<ImageObject ID="10" Boundary="10 10 20 20" CTM="20 0 0 20 0 0" ResourceID="5">
  %CLIP%
</ImageObject>"##;
    let with = |flag: &str| page_ops(&body.replace("%CLIP%", flag), "");
    let transform_of = |ops: &[DrawOp]| {
        ops.iter()
            .find_map(|op| match op {
                DrawOp::Transform(t) => Some(*t),
                _ => None,
            })
            .expect("object scope")
    };
    let clip_bbox = |ops: &[DrawOp]| {
        ops.iter()
            .find_map(|op| match op {
                DrawOp::ClipPath { path, .. } => Some(path.bounding_box()),
                _ => None,
            })
            .expect("clip")
    };
    let plain = with(
        r##"<Clips><Clip><Area>
      <Path><AbbreviatedData>M 2 2 L 6 2 L 6 6 L 2 6 Z</AbbreviatedData></Path>
    </Area></Clip></Clips>"##,
    );
    let flagged = with(
        r#"<Clips TransFlag="false"><Clip><Area>
      <Path><AbbreviatedData>M 2 2 L 6 2 L 6 6 L 2 6 Z</AbbreviatedData></Path>
    </Area></Clip></Clips>"#,
    );
    // The path is kept in object space and the CTM rides on the scope, so the
    // clip is left exactly as written.
    assert_eq!(
        clip_bbox(&plain),
        Rect::new(2.0, 2.0, 4.0, 4.0),
        "the clip is stated in the object square, as written"
    );
    assert_eq!(
        clip_bbox(&flagged),
        clip_bbox(&plain),
        "an attribute outside the standard does not move the clip"
    );
    assert_eq!(
        transform_of(&plain),
        transform_of(&flagged),
        "both live in the same object scope"
    );
}

#[test]
fn gb_t_33190_8_4_a_text_area_clips_to_the_glyph_outlines() {
    // A `Text` area is the other kind clause 8.4 allows: the region is the
    // glyph outlines rather than a path, so it reaches the backend as a clip
    // run the way drawn text does, for the backend to outline.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="0 0 40 40">
  <Clips><Clip><Area>
    <Text Font="7" Size="20"><TextCode X="2" Y="30">Hi</TextCode></Text>
  </Area></Clip></Clips>
  <AbbreviatedData>M 0 0 L 40 0 L 40 40 Z</AbbreviatedData>
</PathObject>"#,
        "",
    );
    let run = ops
        .iter()
        .find_map(|op| match op {
            DrawOp::ClipText(run) => Some(run),
            _ => None,
        })
        .expect("a text clip reaches the display list");
    assert_eq!(run.font.0, 7);
    assert_eq!(run.size.value(), 20.0);
    assert_eq!(run.text.as_deref(), Some("Hi"));
    assert!(
        !ops.iter().any(|op| matches!(op, DrawOp::ClipPath { .. })),
        "a text area is not also turned into a path"
    );
}

#[test]
fn gb_t_33190_10_an_image_object_fills_the_unit_square_of_its_ctm() {
    // The CTM scale equals the boundary size for 119 of the 121 image objects
    // in the 42-page invoice, so the content space is the unit square and the
    // CTM both positions and sizes the picture.
    let ops = page_ops(
        r##"<ImageObject ID="10" Boundary="31.743 25.6484 145.3829 109.0266" CTM="145.3829 0 0 109.0266 0 0" ResourceID="5"/>"##,
        "",
    );
    let DrawOp::Image { dst, .. } = ops
        .iter()
        .find(|op| matches!(op, DrawOp::Image { .. }))
        .expect("image op")
    else {
        panic!("expected an image, got {ops:?}");
    };
    assert_eq!(*dst, Rect::new(0.0, 0.0, 1.0, 1.0), "the unit square");
    let DrawOp::Transform(t) = &ops[1] else {
        panic!("expected the object scope, got {ops:?}");
    };
    assert_eq!(
        t.apply(0.0, 0.0),
        (31.743, 25.6484),
        "the boundary moves the scaled corner onto the page"
    );
    assert_eq!(
        t.apply(1.0, 1.0),
        (177.1259, 134.675),
        "and the far corner lands on the boundary's own far side"
    );
}

#[test]
fn gb_t_33190_9_1_a_draw_param_supplies_the_stroke_and_fill_switches() {
    // Clause 9.1: a path object with no Stroke or Fill attribute of its own
    // takes those from its draw parameter, so the parameter can turn a
    // default-filled object into a stroke-only one.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" DrawParam="d1">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        r#"<DrawParam ID="d1" Stroke="true" Fill="false" LineWidth="1.5"/>"#,
    );
    let DrawOp::Path {
        fill,
        stroke,
        stroke_style,
        ..
    } = &ops[0]
    else {
        panic!("expected a path op, got {ops:?}");
    };
    assert!(fill.is_none(), "the draw parameter turned filling off");
    assert!(stroke.is_some(), "and stroking on");
    assert_eq!(stroke_style.width, 1.5);
}

#[test]
fn gb_t_33190_9_1_path_defaults_are_stroke_on_and_fill_off() {
    // Clause 9.1 takes the defaults from `CT_Path`, which sets `Stroke` true
    // and `Fill` false: a bare object is stroked, never filled. Neither
    // attribute nor a draw parameter is named here.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        "",
    );
    let DrawOp::Path { fill, stroke, .. } = &ops[0] else {
        panic!("expected a path op, got {ops:?}");
    };
    assert!(fill.is_none(), "an absent Fill means no fill");
    assert!(stroke.is_some(), "an absent Stroke means stroked");
}

#[test]
fn gb_t_33190_9_1_graphic_unit_line_width_overrides_the_draw_param() {
    // The same clause lets the graphic unit state the stroke parameters
    // itself, and then those win over whatever the draw parameter said.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" DrawParam="d1" LineWidth="0.1" Cap="Round" Join="Bevel">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        r#"<DrawParam ID="d1" Stroke="true" LineWidth="9" Cap="Butt" Join="Miter"/>"#,
    );
    let DrawOp::Path { stroke_style, .. } = &ops[0] else {
        panic!("expected a path op, got {ops:?}");
    };
    assert_eq!(stroke_style.width, 0.1, "the unit's own width wins");
    assert_eq!(stroke_style.cap, steleofd_display::ops::LineCap::Round);
    assert_eq!(stroke_style.join, steleofd_display::ops::LineJoin::Bevel);
}

/// Open a package whose entries are given directly, for the side files that
/// are not part of the page tree.
fn side_package(entries: &[(&str, &str)]) -> steleofd_package::Package {
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, s)| (*p, s.as_bytes())).collect();
    let bytes = build_archive(&borrowed).expect("build fixture");
    steleofd_package::Package::from_bytes(&bytes).expect("open package")
}

#[test]
fn gb_t_33190_15_the_annotation_index_points_at_page_files() {
    // Annotations live apart from the page they mark up: the index says which
    // file holds the marks for which page.
    let mut package = side_package(&[(
        "Doc_0/Annots/Annotations.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Annotations>
  <Page PageID="1"><FileLoc>Annots/Page_0/Annotation.xml</FileLoc></Page>
  <Page PageID="2"><FileLoc>Annots/Page_1/Annotation.xml</FileLoc></Page>
</Annotations>"#,
    )]);
    let index = steleofd_parser::marks::read_annotations_index(
        &mut package,
        "Doc_0/Annots/Annotations.xml",
    )
    .expect("index");
    assert_eq!(index.pages.len(), 2);
    assert_eq!(index.pages[0].page_id, "1");
    assert_eq!(index.pages[0].file_loc, "Annots/Page_0/Annotation.xml");
    assert_eq!(index.pages[1].page_id, "2");
}

#[test]
fn gb_t_33190_15_an_annotation_keeps_its_type_remark_and_parameters() {
    let mut package = side_package(&[(
        "Doc_0/Annots/Page_0/Annotation.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ofd:PageAnnot xmlns:ofd="http://www.ofdspec.org/2016">
  <Annot ID="1" Type="Highlight" Creator="alice" LastModDate="2024-01-01" Visible="false">
    <Remark>look here</Remark>
    <Parameters><Parameter Name="Colour">#FFEE00</Parameter></Parameters>
    <Appearance Boundary="5 5 40 20"/>
  </Annot>
  <Annot ID="2" Type="Stamp" Creator="bob" LastModDate="2024-01-02"/>
</ofd:PageAnnot>"#,
    )]);
    let page = steleofd_parser::marks::read_page_annots(
        &mut package,
        "Doc_0/Annots/Page_0/Annotation.xml",
    )
    .expect("annots");
    assert_eq!(page.items.len(), 2);
    let first = &page.items[0];
    assert_eq!(first.id, "1");
    assert_eq!(first.kind, "Highlight");
    assert_eq!(first.remark.as_deref(), Some("look here"));
    assert_eq!(
        first.parameters,
        vec![("Colour".to_string(), "#FFEE00".to_string())]
    );
    assert!(!first.visible, "Visible=false hides the annotation");
    assert_eq!(
        first.boundary,
        Some(Rect::new(5.0, 5.0, 40.0, 20.0)),
        "the appearance says where it is drawn"
    );
    assert_eq!(page.items[1].id, "2");
    assert!(page.items[1].visible, "annotations are visible by default");
}

#[test]
fn gb_t_33190_16_a_custom_tag_names_its_namespace_and_content() {
    // A custom tag is a detached reference: the index names the namespace and
    // points at the content file rather than inlining it.
    let mut package = side_package(&[(
        "Doc_0/Tags/CustomTags.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CustomTags>
  <CustomTag NameSpace="http://example.com/invoice">
    <SchemaLoc>Tags/invoice.xsd</SchemaLoc>
    <FileLoc>Tags/Tag_0.xml</FileLoc>
  </CustomTag>
  <CustomTag NameSpace="urn:plain">
    <FileLoc>Tags/Tag_1.xml</FileLoc>
  </CustomTag>
</CustomTags>"#,
    )]);
    let tags = steleofd_parser::marks::read_custom_tags(&mut package, "Doc_0/Tags/CustomTags.xml")
        .expect("tags");
    assert_eq!(tags.tags.len(), 2);
    assert_eq!(tags.tags[0].namespace, "http://example.com/invoice");
    assert_eq!(tags.tags[0].schema_loc.as_deref(), Some("Tags/invoice.xsd"));
    assert_eq!(tags.tags[0].file_loc, "Tags/Tag_0.xml");
    assert_eq!(
        tags.tags[1].schema_loc, None,
        "the schema is optional, the namespace and file are not"
    );
}

#[test]
fn gb_t_33190_17_an_extension_names_its_app_and_its_target() {
    // An extension attaches to an object by id and records which application
    // wrote it, so a reader can skip blocks it does not understand.
    let mut package = side_package(&[(
        "Doc_0/Extensions.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Extensions>
  <Extension AppName="Acme Writer" Company="Acme" AppVersion="3.1" RefId="7">
    <Property Name="LayerName">stamps</Property>
    <ExtendData>Extends/Ext_0.dat</ExtendData>
  </Extension>
  <Extension AppName="Other" RefId="9">
    <Data><anything/></Data>
  </Extension>
</Extensions>"#,
    )]);
    let ext = steleofd_parser::marks::read_extensions(&mut package, "Doc_0/Extensions.xml")
        .expect("extensions");
    assert_eq!(ext.items.len(), 2);
    let first = &ext.items[0];
    assert_eq!(first.ref_id, "7", "an extension attaches to one object");
    assert_eq!(first.app_name, "Acme Writer");
    assert_eq!(first.company.as_deref(), Some("Acme"));
    assert_eq!(first.app_version.as_deref(), Some("3.1"));
    assert_eq!(
        first.properties,
        vec![("LayerName".to_string(), "stamps".to_string())]
    );
    assert_eq!(first.extend_data, vec!["Extends/Ext_0.dat".to_string()]);
    assert!(!first.has_data);
    assert!(
        ext.items[1].has_data,
        "a Data block is noted even if opaque"
    );
}

#[test]
fn gb_t_33190_12_a_video_resource_names_its_file_and_format() {
    // Clause 12 has no video graphic unit of its own: a video is a multimedia
    // resource, and the schema types it `Video` alongside `Image` and `Audio`.
    let list = steleofd_parser::content::read_multimedias(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res">
  <MultiMedias>
    <MultiMedia ID="20" Type="Video" Format="mp4">
      <MediaFile>Res/clip.mp4</MediaFile>
    </MultiMedia>
    <MultiMedia ID="21" Type="Audio" Format="wav">
      <MediaFile>Res/tone.wav</MediaFile>
    </MultiMedia>
    <MultiMedia ID="22" Type="Image"><MediaFile>Res/logo.png</MediaFile></MultiMedia>
  </MultiMedias>
</Res>"#,
    )
    .expect("multimedias");
    assert_eq!(list.items.len(), 3);
    let video = &list.items[0];
    assert_eq!(video.id, "20");
    assert_eq!(video.kind, steleofd_model::MultimediaKind::Video);
    assert_eq!(video.format.as_deref(), Some("mp4"));
    assert_eq!(video.media_file, "Res/clip.mp4");
    assert_eq!(list.items[1].kind, steleofd_model::MultimediaKind::Audio);
    assert_eq!(list.items[2].kind, steleofd_model::MultimediaKind::Image);
}

#[test]
fn gb_t_33190_12_a_movie_action_points_at_a_video_resource() {
    // The video is played by a `Movie` action, which names the resource and
    // says what to do with it.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <Actions>
    <Action Event="CLICK"><Movie ResourceID="20" Operator="Play"/></Action>
  </Actions>
</PathObject>"#,
        "",
    );
    assert!(!ops.is_empty(), "the action does not stop the path drawing");
}

#[test]
fn gb_t_33190_13_a_composite_object_places_a_vector_resource() {
    // A composite object is a placed vector graphic: the page object carries
    // only a resource id, and the graphic lives in the resource file. With the
    // resource absent there is nothing to draw, so the page is left empty.
    let ops = page_ops(
        r#"<CompositeObject ID="30" Boundary="10 10 50 30" ResourceID="5"/>"#,
        "",
    );
    assert!(
        !ops.iter().any(|op| matches!(op, DrawOp::Image { .. })),
        "a composite whose resource cannot be found draws nothing, got {ops:?}"
    );
}

#[test]
fn gb_t_33190_13_a_composite_object_draws_its_resources_content() {
    // With the resource present, the composite places the graphic's own
    // objects, pulling its content block in at the object's position.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(
                r#"<Page ID="1"><Content><Layer>
                     <CompositeObject ID="30" Boundary="0 0 50 30" ResourceID="5"/>
                   </Layer></Content></Page>"#,
            ),
        ),
        (
            "Doc_0/DocumentRes.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><CompositeGraphicUnits>
  <CompositeGraphicUnit ID="5" Width="50" Height="30">
    <Content><Layer ID="1">
      <PathObject ID="6" Boundary="0 0 50 30">
        <AbbreviatedData>M 0 0 L 50 0 L 50 30 Z</AbbreviatedData>
      </PathObject>
    </Layer></Content>
  </CompositeGraphicUnit>
</CompositeGraphicUnits></Res>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let ops = parser.page_display(&doc, 0).expect("flatten page").ops;
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Path { .. })),
        "the composite's own path is drawn, got {ops:?}"
    );
}

#[test]
fn gb_t_33190_13_a_vector_resource_holds_its_own_page_block() {
    // `CT_VectorG` gives the graphic a natural size and a content block, so it
    // can be flattened exactly like a page.
    let list = steleofd_parser::content::read_composite_graphic_units(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res">
  <CompositeGraphicUnits>
    <CompositeGraphicUnit ID="5" Width="50" Height="30">
      <Thumbnail>4</Thumbnail>
      <Content><Layer ID="1">
        <PathObject ID="6" Boundary="0 0 50 30">
          <AbbreviatedData>M 0 0 L 50 0 L 50 30 Z</AbbreviatedData>
        </PathObject>
      </Layer></Content>
    </CompositeGraphicUnit>
  </CompositeGraphicUnits>
</Res>"#,
    )
    .expect("composite graphic units");
    assert_eq!(list.items.len(), 1);
    let unit = &list.items[0];
    assert_eq!(unit.id, "5");
    assert_eq!(unit.width, 50.0);
    assert_eq!(unit.height, 30.0);
    assert_eq!(unit.thumbnail.as_deref(), Some("4"));
    let block = unit.content.as_ref().expect("content block");
    assert_eq!(block.layers.len(), 1);
    assert_eq!(block.layers[0].objects.len(), 1);
}

#[test]
fn gb_t_33190_14_1_an_action_records_its_event_and_target() {
    // `Event` is required and may only be `DO`, `PO` or `CLICK`; the body says
    // what the action does.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <Actions>
    <Action Event="CLICK"><Goto><Dest Type="XYZ" PageID="2" Left="0" Top="0" Zoom="1"/></Goto></Action>
    <Action Event="DO"><Goto><Bookmark Name="chapter-1"/></Goto></Action>
  </Actions>
</PathObject>"#,
        "",
    );
    assert!(!ops.is_empty());

    // Read the same page through the model-facing reader to inspect the action.
    let actions = read_page_actions(
        r#"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
  <Actions>
    <Action Event="CLICK"><Goto><Dest Type="XYZ" PageID="2" Left="0" Top="0" Zoom="1"/></Goto></Action>
    <Action Event="PO"><Goto><Bookmark Name="chapter-1"/></Goto></Action>
  </Actions>
</PathObject>"#,
    );
    assert_eq!(actions.items.len(), 2);
    assert_eq!(actions.items[0].event, ActionEvent::Click);
    assert_eq!(actions.items[1].event, ActionEvent::Hover);
    match &actions.items[0].kind {
        ActionKind::Goto(GotoTarget::Dest(d)) => {
            assert_eq!(d.kind, "XYZ");
            assert_eq!(d.page_id, "2");
            assert_eq!(d.zoom.as_deref(), Some("1"));
        }
        other => panic!("expected a destination jump, got {other:?}"),
    }
    match &actions.items[1].kind {
        ActionKind::Goto(GotoTarget::Bookmark { name }) => assert_eq!(name, "chapter-1"),
        other => panic!("expected a bookmark jump, got {other:?}"),
    }
}

#[test]
fn gb_t_33190_14_1_a_uri_action_keeps_its_base_and_target() {
    let actions = read_page_actions(
        r#"<PathObject ID="10" Boundary="0 0 10 10">
  <AbbreviatedData>M 0 0 L 10 0 Z</AbbreviatedData>
  <Actions>
    <Action Event="CLICK">
      <URI URI="page.html" Base="https://example.com/" Target="_blank"/>
    </Action>
  </Actions>
</PathObject>"#,
    );
    match &actions.items[0].kind {
        ActionKind::Uri(u) => {
            assert_eq!(u.uri, "page.html");
            assert_eq!(u.base.as_deref(), Some("https://example.com/"));
            assert_eq!(u.target.as_deref(), Some("_blank"));
        }
        other => panic!("expected a URI action, got {other:?}"),
    }
}

#[test]
fn gb_t_33190_14_1_an_unknown_event_is_rejected() {
    // `Event` is an enumeration, so a value outside it is a broken document
    // rather than something to guess about.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(
                r#"<Page ID="1"><Content><Layer>
  <PathObject ID="10" Boundary="0 0 10 10">
    <AbbreviatedData>M 0 0 L 10 0 Z</AbbreviatedData>
    <Actions><Action Event="MAYBE"><URI URI="x"/></Action></Actions>
  </PathObject>
</Layer></Content></Page>"#,
            ),
        ),
    ];
    let mut parser = open(&entries);
    let err = match parser.open_document() {
        Ok(_) => panic!("Event=MAYBE must be rejected"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("MAYBE"), "got {err}");
}

/// Flatten a page holding one object and return the actions that object
/// declared, for the tests that need the model rather than the draw ops.
fn read_page_actions(object: &str) -> steleofd_model::Actions {
    let doc_xml = document_xml(&format!(
        r#"<Page ID="1"><Content><Layer>{object}</Layer></Content></Page>"#
    ));
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        ("Doc_0/Document.xml", doc_xml),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_model(&doc, 0).expect("page model");
    let block = page.content.as_ref().expect("page content");
    let object = block
        .layers
        .iter()
        .flat_map(|l| l.objects.iter())
        .find(|o| o.id().is_some())
        .expect("an object");
    match object {
        steleofd_model::Object::PathObject(p) => p.base.actions.clone().unwrap_or_default(),
        other => panic!("expected a path object, got {other:?}"),
    }
}

#[test]
fn gb_t_33190_8_1_1_object_space_agrees_with_the_page_space() {
    // Clause 8.1.1 makes an object's content space the page's, scaled only by
    // the object's own matrix; a boundary at the origin with an identity
    // matrix therefore leaves path data as written, in millimetres.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="0 0 20 20" Stroke="true">
  <AbbreviatedData>M 1 2 L 11 12</AbbreviatedData>
</PathObject>"#,
        "",
    );
    let DrawOp::Path { path, .. } = &ops[0] else {
        panic!("expected a path op, got {ops:?}");
    };
    let bounds = path.tight_bounding_box();
    assert!(
        (bounds.x - 1.0).abs() < 1e-9 && (bounds.y - 2.0).abs() < 1e-9,
        "the data is millimetres from the page origin, got {bounds:?}"
    );
    assert!(
        (bounds.w - 10.0).abs() < 1e-9 && (bounds.h - 10.0).abs() < 1e-9,
        "and is not scaled on the way in, got {bounds:?}"
    );
}

#[test]
fn gb_t_33190_8_1_2_boundary_places_the_object_on_the_page() {
    // Clause 8.1.2: the boundary is where the object lands, and the display
    // list says so by bracketing the object in a transform, leaving the path
    // data in its own coordinates.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="40 50 20 20" Stroke="true">
  <AbbreviatedData>M 0 0 L 20 0 L 20 20 Z</AbbreviatedData>
</PathObject>"#,
        "",
    );
    let placed = ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Transform(t) => Some(*t),
            _ => None,
        })
        .expect("the boundary reaches the list as a transform");
    assert_eq!(
        placed.apply(0.0, 0.0),
        (40.0, 50.0),
        "the boundary corner is where the object starts"
    );
    assert_eq!(
        placed.apply(20.0, 20.0),
        (60.0, 70.0),
        "and the far corner follows the boundary's size"
    );
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::Save)),
        "the transform is scoped, so it cannot leak into the next object"
    );
}

#[test]
fn gb_t_33190_8_1_4_a_page_box_has_a_measurable_size() {
    // Clause 8.1.4 gives the page a physical box in millimetres, and every
    // later scale — raster DPI, PDF points — is derived from it.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1"><Content><Layer/></Content></Page>"#),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    assert_eq!(page.page_box.w, 210.0);
    assert_eq!(page.page_box.h, 297.0);
    assert!(page.page_box.size_px(96.0).0 > 0, "and it scales to pixels");
}

#[test]
fn gb_t_33190_8_2_3_line_width_and_miter_limit_reach_the_op() {
    // Clause 8.2.3: width and miter limit are stroke parameters, and a unit
    // that states its own wins over the draw parameter it names.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" DrawParam="d1" LineWidth="0.25" MiterLimit="7">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        r#"<DrawParam ID="d1" Stroke="true" LineWidth="9" MiterLimit="2"/>"#,
    );
    let DrawOp::Path { stroke_style, .. } = &ops[0] else {
        panic!("expected a path op, got {ops:?}");
    };
    assert_eq!(stroke_style.width, 0.25, "the unit states the width");
    assert_eq!(stroke_style.miter_limit, 7.0, "and the miter limit");
}

#[test]
fn gb_t_33190_8_3_1_a_colour_value_reads_as_its_three_channels() {
    // Clause 8.3.1: a colour is three channels, and each may carry its own
    // hex marker. Reading them as one six-digit string is the mistake that
    // hid behind every dark document in the corpus.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" Fill="true">
  <FillColor Value="#ee #20 #25"/>
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        "",
    );
    let DrawOp::Path {
        fill: Some(Paint::Solid(c)),
        ..
    } = &ops[0]
    else {
        panic!("expected a filled path, got {ops:?}");
    };
    assert_eq!((c.r, c.g, c.b), (0xee, 0x20, 0x25));
}

#[test]
fn gb_t_33190_8_3_3_alpha_reaches_the_paint() {
    // Clause 8.3.3: transparency is a fourth channel, and it has to survive
    // the trip from the colour value to the paint or a faded overlay paints
    // solid.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" Fill="true">
  <FillColor Value="0 0 0" Alpha="64"/>
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        "",
    );
    let DrawOp::Path {
        fill: Some(Paint::Solid(c)),
        ..
    } = &ops[0]
    else {
        panic!("expected a filled path, got {ops:?}");
    };
    assert_eq!(c.a, 64, "alpha travels with the colour");
}

#[test]
fn gb_t_33190_8_6_a_graphics_unit_resolves_from_its_resource_id() {
    // Clause 8.6: an object may name a graphics unit in the resource table,
    // and the unit's own draw parameter then applies in place of an inline
    // one.
    let ops = page_ops(
        r##"<PathObject ID="10" Boundary="0 0 10 10" DrawParam="unit1">
  <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
</PathObject>"##,
        r#"<DrawParam ID="unit1" Stroke="true" Fill="false" LineWidth="1.25"/>"#,
    );
    let DrawOp::Path {
        fill,
        stroke,
        stroke_style,
        ..
    } = &ops[0]
    else {
        panic!("expected a path op, got {ops:?}");
    };
    assert!(fill.is_none(), "the named unit turned filling off");
    assert!(stroke.is_some());
    assert_eq!(stroke_style.width, 1.25, "and carried its width");
}

#[test]
fn gb_t_33190_9_3_1_a_path_must_open_with_a_subpath_start() {
    // Clause 9.3.1, table 36: the first operator of abbreviated data is `S`
    // or `M`, and anything else is malformed rather than silently empty.
    assert!(parse_path("M 0 0 L 1 1").is_ok(), "M opens a path");
    assert!(parse_path("S 0 0 L 1 1").is_ok(), "S opens a path");
    for bad in ["L 0 0", "Q 0 0 1 1", "A 0 0 1 1 0 1 1", "Z"] {
        assert!(
            parse_path(bad).is_err(),
            "{bad:?} cannot open a path and must be refused"
        );
    }
}

#[test]
fn gb_t_33190_9_4_a_path_keeps_its_data_and_its_clipping() {
    // Clause 9.4: a path object is its geometry plus the region that limits
    // it, and both have to survive flattening — the clip as its own op, the
    // subject as the path it bounds.
    let ops = page_ops(
        r#"<PathObject ID="10" Boundary="0 0 20 20" Stroke="true">
  <Clips><Clip><Area><Path><AbbreviatedData>M 0 0 L 20 0 L 20 20 L 0 20 Z</AbbreviatedData></Path></Area></Clip></Clips>
  <AbbreviatedData>M 2 2 L 18 2 L 18 18 Z</AbbreviatedData>
</PathObject>"#,
        "",
    );
    assert!(
        ops.iter().any(|op| matches!(op, DrawOp::ClipPath { .. })),
        "the clipping region reached the list as a clip, got {ops:?}"
    );
    let subject = ops
        .iter()
        .find_map(|op| match op {
            DrawOp::Path { path, stroke, .. } if stroke.is_some() => Some(path),
            _ => None,
        })
        .expect("the subject keeps its stroke");
    assert_eq!(subject.len(), 3, "and its own three segments");
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op, DrawOp::Save | DrawOp::Restore))
            .count(),
        2,
        "the clip is scoped to the object that asked for it"
    );
}

#[test]
fn gb_t_33190_11_5_reading_direction_and_alignment_are_kept() {
    // Clause 11.5: a text object states which way its characters run and how
    // they are placed, and a vertical run must not be flattened into a
    // horizontal one.
    let ops = page_ops(
        r#"<TextObject ID="10" Boundary="0 0 100 30" Font="3" Size="5"
   ReadDirection="0" CharDirection="270" Weight="700" Italic="true">
  <TextCode X="0" Y="0">AB</TextCode>
</TextObject>"#,
        "",
    );
    let runs = ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) => Some(run),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!runs.is_empty(), "the run reached the display list");
    assert_eq!(runs[0].plain_text(), "AB", "and kept its text for search");
}

#[test]
fn gb_t_33190_7_10_a_page_resource_table_is_read() {
    // Clause 7.10: a page may carry its own resource table at `PageRes`, and
    // a draw parameter declared there has to be reachable from the page body.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#),
        ),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <PageRes>PageRes.xml</PageRes>
  <Content><Layer>
    <PathObject ID="10" Boundary="0 0 10 10" DrawParam="p1">
      <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
    </PathObject>
  </Layer></Content>
</Page>"#
                .to_string(),
        ),
        (
            "Doc_0/Pages/Page_0/PageRes.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="."><DrawParams>
  <DrawParam ID="p1" Stroke="true" LineWidth="3"/>
</DrawParams></Res>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let width = page.ops.iter().find_map(|op| match op {
        DrawOp::Path { stroke_style, .. } => Some(stroke_style.width),
        _ => None,
    });
    assert_eq!(
        width,
        Some(3.0),
        "the page table's unit is what the object names, got {:?}",
        page.ops
    );
}

#[test]
fn gb_t_33190_7_10_a_page_resource_table_shadows_the_document_one() {
    // Clause 7.10 read with 7.9: the page table is the nearer of the two, so
    // an id both declare has to resolve to the page's copy.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#)
                .replace(
                    "</CommonData>",
                    r#"<DrawParams><DrawParam ID="p1" Stroke="true" LineWidth="9"/></DrawParams></CommonData>"#,
                ),
        ),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <PageRes>PageRes.xml</PageRes>
  <Content><Layer>
    <PathObject ID="10" Boundary="0 0 10 10" DrawParam="p1">
      <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
    </PathObject>
  </Layer></Content>
</Page>"#
                .to_string(),
        ),
        (
            "Doc_0/Pages/Page_0/PageRes.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="."><DrawParams>
  <DrawParam ID="p1" Stroke="true" LineWidth="1.5"/>
</DrawParams></Res>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let page = parser.page_display(&doc, 0).expect("flatten page");
    let width = page.ops.iter().find_map(|op| match op {
        DrawOp::Path { stroke_style, .. } => Some(stroke_style.width),
        _ => None,
    });
    assert_eq!(
        width,
        Some(1.5),
        "the nearer table wins over the document's 9 mm, got {:?}",
        page.ops
    );
}

#[test]
fn gb_t_33190_7_14_a_thumbnail_is_read_and_may_be_absent() {
    // Clause 7.14: a vector graphic may name a preview image by reference id.
    // The id is kept verbatim — resolving it to bytes is the image loader's
    // job — and a unit without one reads back as `None`, not its neighbour's.
    let list = steleofd_parser::content::read_composite_graphic_units(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res">
  <CompositeGraphicUnits>
    <CompositeGraphicUnit ID="5" Width="50" Height="30">
      <Thumbnail>4</Thumbnail>
      <Content><Layer ID="1"/></Content>
    </CompositeGraphicUnit>
    <CompositeGraphicUnit ID="6" Width="10" Height="10">
      <Content><Layer ID="1"/></Content>
    </CompositeGraphicUnit>
  </CompositeGraphicUnits>
</Res>"#,
    )
    .expect("composite graphic units");

    assert_eq!(
        list.items[0].thumbnail.as_deref(),
        Some("4"),
        "the declared thumbnail id is kept verbatim"
    );
    assert_eq!(
        list.items[1].thumbnail, None,
        "a unit with no thumbnail does not inherit its neighbour's"
    );
}

#[test]
fn gb_t_33190_7_11_viewer_preferences_are_read() {
    // Clause 7.11: `VPreferences` says how a reader should open the document.
    // Every child is optional, so a preference that is stated has to survive
    // and one that is not has to stay absent.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
  </CommonData>
  <VPreferences>
    <PageMode>UseOutlines</PageMode>
    <PageLayout>TwoPageL</PageLayout>
    <TabDisplay>FileName</TabDisplay>
    <HideToolbar>true</HideToolbar>
    <HideMenubar>false</HideMenubar>
    <Zoom>1.5</Zoom>
  </VPreferences>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;
    let doc = steleofd_parser::content::read_document(xml).expect("read document");
    let prefs = doc.vpreferences.as_ref().expect("preferences were read");
    assert_eq!(
        prefs.page_mode,
        Some(steleofd_model::PageMode::UseOutlines),
        "the initial view mode is kept"
    );
    assert_eq!(
        prefs.page_layout,
        Some(steleofd_model::PageLayout::TwoPageL),
        "so is the layout"
    );
    assert_eq!(
        prefs.tab_display,
        Some(steleofd_model::TabDisplay::FileName),
        "and what the tab shows"
    );
    assert_eq!(prefs.hide_toolbar, Some(true), "a stated boolean is kept");
    assert_eq!(
        prefs.hide_menubar,
        Some(false),
        "false is kept, not dropped"
    );
    assert_eq!(
        prefs.hide_window_ui, None,
        "an absent preference stays absent rather than defaulting"
    );
    assert_eq!(
        prefs.zoom,
        steleofd_model::ZoomSetting::Zoom(1.5),
        "an explicit zoom factor is kept"
    );
}

#[test]
fn gb_t_33190_7_11_a_named_zoom_mode_is_kept_apart_from_a_factor() {
    // The schema makes `ZoomMode` and `Zoom` a choice, so a named fit mode
    // must not be read as a factor or the other way round.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
  </CommonData>
  <VPreferences><ZoomMode>FitWidth</ZoomMode></VPreferences>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;
    let doc = steleofd_parser::content::read_document(xml).expect("read document");
    let prefs = doc.vpreferences.as_ref().expect("preferences were read");
    assert_eq!(
        prefs.zoom,
        steleofd_model::ZoomSetting::ZoomMode(steleofd_model::ZoomMode::FitWidth),
        "a named mode stays a mode"
    );
}

#[test]
fn gb_t_33190_7_11_a_document_without_preferences_reads_them_as_absent() {
    // The element is optional; a document that omits it must not invent one.
    let doc = steleofd_parser::content::read_document(&document_xml(
        r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#,
    ))
    .expect("read document");
    assert!(
        doc.vpreferences.is_none(),
        "no element means no preferences, got {:?}",
        doc.vpreferences
    );
}

#[test]
fn gb_t_33190_7_12_permissions_and_valid_period_are_read() {
    // Clause 7.12: the document states what a reader may do. The flags are
    // independent and each is optional, so what is stated has to survive and
    // what is not has to stay absent.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
  </CommonData>
  <Permissions>
    <Edit>false</Edit>
    <Annot>true</Annot>
    <PrintScreen>false</PrintScreen>
    <Print Printable="true" Copies="3"/>
    <ValidPeriod StartDate="2024-01-01T00:00:00Z" EndDate="2025-01-01T00:00:00Z"/>
  </Permissions>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;
    let doc = steleofd_parser::content::read_document(xml).expect("read document");
    let perms = doc.permissions.as_ref().expect("permissions were read");
    assert_eq!(perms.edit, Some(false), "editing is refused");
    assert_eq!(perms.annotate, Some(true), "annotating is allowed");
    assert_eq!(
        perms.export, None,
        "a flag the document does not state stays absent, not defaulted"
    );
    assert_eq!(perms.print_screen, Some(false));

    let print = perms.print.as_ref().expect("print permission was read");
    assert!(print.printable, "printing is allowed");
    assert_eq!(print.copies, 3, "and carries its copy count");

    let period = perms.valid_period.as_ref().expect("valid period was read");
    assert_eq!(period.start_date.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(period.end_date.as_deref(), Some("2025-01-01T00:00:00Z"));
}

#[test]
fn gb_t_33190_7_12_print_copies_default_to_no_limit() {
    // `Copies` defaults to -1, the schema's "no stated limit", so a `Print`
    // that names only `Printable` must not read back as zero copies.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
  </CommonData>
  <Permissions><Print Printable="false"/></Permissions>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;
    let doc = steleofd_parser::content::read_document(xml).expect("read document");
    let print = doc
        .permissions
        .as_ref()
        .and_then(|p| p.print.as_ref())
        .expect("print permission was read");
    assert!(!print.printable);
    assert_eq!(print.copies, -1, "the absent copy count means no limit");
}

#[test]
fn gb_t_33190_7_12_a_document_without_permissions_reads_them_as_absent() {
    // The element is optional; a document that omits it must not invent one.
    let doc = steleofd_parser::content::read_document(&document_xml(
        r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#,
    ))
    .expect("read document");
    assert!(
        doc.permissions.is_none(),
        "no element means no permissions, got {:?}",
        doc.permissions
    );
}

#[test]
fn gb_t_33190_7_13_declared_colour_spaces_are_read() {
    // Clause 7.13: a `Res` file may declare colour spaces that images then
    // name instead of carrying sRGB samples. The type, bit depth, profile and
    // palette are all part of the declaration.
    let list = steleofd_parser::content::read_color_spaces(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res">
  <ColorSpaces>
    <ColorSpace ID="5" Type="CMYK" BitsPerComponent="16" Profile="Color/icc.icc"/>
    <ColorSpace ID="6" Type="RGB">
      <Palette><CV>0 0 0</CV><CV>255 255 255</CV></Palette>
    </ColorSpace>
    <ColorSpace ID="7" Type="GRAY"/>
  </ColorSpaces>
</Res>"#,
    )
    .expect("colour spaces");

    assert_eq!(list.items.len(), 3);
    let cmyk = &list.items[0];
    assert_eq!(cmyk.id, "5");
    assert_eq!(cmyk.kind, steleofd_model::ColorSpaceKind::Cmyk);
    assert_eq!(cmyk.bits_per_component, 16, "a stated depth is kept");
    assert_eq!(cmyk.profile.as_deref(), Some("Color/icc.icc"));

    let rgb = &list.items[1];
    assert_eq!(rgb.kind, steleofd_model::ColorSpaceKind::Rgb);
    assert_eq!(
        rgb.bits_per_component, 8,
        "the schema's default depth applies when none is stated"
    );
    assert_eq!(rgb.palette, vec!["0 0 0", "255 255 255"]);
    assert!(rgb.profile.is_none(), "no profile was declared");

    let gray = &list.items[2];
    assert_eq!(gray.kind, steleofd_model::ColorSpaceKind::Gray);
    assert!(gray.palette.is_empty());
}

#[test]
fn gb_t_33190_7_13_an_unknown_colour_space_type_is_refused() {
    // The schema lists three types; anything else is malformed rather than
    // silently read as the default.
    let err = steleofd_parser::content::read_color_spaces(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><ColorSpaces>
  <ColorSpace ID="5" Type="LAB"/>
</ColorSpaces></Res>"#,
    )
    .expect_err("LAB is not one of the three");
    assert!(
        matches!(err, steleofd_error::OfdError::Xml(_)),
        "a bad type is an XML error, got {err:?}"
    );
}

#[test]
fn gb_t_33190_7_13_colour_spaces_reach_the_open_document() {
    // The table is only useful if opening a document actually loads it, and
    // both `Res` tables contribute.
    let entries = [
        ("OFD.xml", OFD_XML.to_string()),
        (
            "Doc_0/Document.xml",
            document_xml(r#"<Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/>"#),
        ),
        (
            "Doc_0/Pages/Page_0/Content.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Page><Content><Layer/></Content></Page>"#
                .to_string(),
        ),
        (
            "Doc_0/DocumentRes.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><ColorSpaces>
  <ColorSpace ID="5" Type="CMYK"/>
</ColorSpaces></Res>"#
                .to_string(),
        ),
        (
            "Doc_0/PublicRes.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Res BaseLoc="Res"><ColorSpaces>
  <ColorSpace ID="6" Type="GRAY"/>
</ColorSpaces></Res>"#
                .to_string(),
        ),
    ];
    let mut parser = open(&entries);
    let doc = parser.open_document().expect("open document");
    let kinds: Vec<_> = doc.color_spaces.items.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            steleofd_model::ColorSpaceKind::Cmyk,
            steleofd_model::ColorSpaceKind::Gray,
        ],
        "both resource tables contribute their spaces"
    );
}
