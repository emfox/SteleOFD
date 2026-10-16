//! The golden-image fixtures: documents whose exact pixels are committed.
//! Each is deliberately small and built from the fixtures already here, so the
//! reference is a few kilobytes and the diff is legible.

use steleofd_package::build_archive;

/// A page content entry that draws a filled path and a text run, the shape
/// every backend has to get right for the most common document there is.
pub const SOLID_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="5 5 40 20" Fill="true">
        <FillColor Value="220 30 40"/>
        <AbbreviatedData>M 0 0 L 40 0 L 40 20 L 0 20 Z</AbbreviatedData>
      </PathObject>
      <PathObject ID="4" Boundary="5 30 60 20" Stroke="true" LineWidth="1">
        <StrokeColor Value="20 40 200"/>
        <AbbreviatedData>M 0 10 L 60 10</AbbreviatedData>
      </PathObject>
      <TextObject ID="5" Boundary="5 44 60 10" Font="7" Size="8">
        <FillColor Value="0 0 0"/>
        <TextCode X="5" Y="8">Stele</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#;

/// A clip built from a path, so a backend that loses the clip shows it.
pub const CLIP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="0 0 100 60" Fill="true">
        <FillColor Value="30 160 90"/>
        <Clips><Clip><Area>
          <Path><AbbreviatedData>M 20 15 L 80 15 L 80 45 L 20 45 Z</AbbreviatedData></Path>
        </Area></Clip></Clips>
        <AbbreviatedData>M 0 0 L 100 0 L 100 60 L 0 60 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// An axial shading, the gradient path a backend can turn into a flat colour
/// without any test noticing until a golden image compares pixels.
pub const GRADIENT_XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="5 5 90 50">
        <FillColor>
          <AxialShd StartPoint="5 5" EndPoint="95 55">
            <Segment Position="0"><Color Value="#FF0000"/></Segment>
            <Segment Position="1"><Color Value="#0000FF"/></Segment>
          </AxialShd>
        </FillColor>
        <AbbreviatedData>M 0 0 L 90 0 L 90 50 L 0 50 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"##;

/// A radial shading. The op-level tests assert the spec reaches the paint;
/// only pixels show whether the cone between the two circles is drawn or
/// flattened to the first stop.
pub const RADIAL_XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="5 5 90 50">
        <FillColor>
          <RadialShd StartPoint="50 30" StartRadius="0" EndPoint="50 30" EndRadius="45">
            <Segment Position="0"><Color Value="#FFFFFF"/></Segment>
            <Segment Position="1"><Color Value="#101820"/></Segment>
          </RadialShd>
        </FillColor>
        <AbbreviatedData>M 0 0 L 90 0 L 90 50 L 0 50 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"##;

/// A dashed outline. A backend that ignores the dash pattern draws a solid
/// rule, which no op-level assertion on the style would catch.
pub const DASH_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="5 25 90 10" Stroke="true" LineWidth="3" DrawParam="d1">
        <StrokeColor Value="180 20 90"/>
        <AbbreviatedData>M 0 5 L 90 5</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// An even-odd fill: a ring whose inner square is cut out. Under the non-zero
/// rule the hole fills in, so the two rules give visibly different pages.
pub const EVENODD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="20 5 60 50" Fill="true" FillRule="Even-Odd">
        <FillColor Value="40 90 200"/>
        <AbbreviatedData>M 0 0 L 60 0 L 60 50 L 0 50 Z M 15 12 L 45 12 L 45 38 L 15 38 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// A path that is stroked but not filled. A backend that fills every path
/// regardless of `Fill="false"` paints the interior and shows it here.
pub const STROKEONLY_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="10 10 80 40" Fill="false" Stroke="true" LineWidth="2" LineJoin="Round">
        <StrokeColor Value="10 10 10"/>
        <AbbreviatedData>M 0 0 L 80 0 L 80 40 L 0 40 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// A circle drawn from two half-arcs of radius 25 that meet at (25,27) and
/// (75,27), then closed. The shape whose control points a backend is likeliest
/// to get subtly wrong while still emitting the right op.
pub const ARCS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="25 2 50 50" Fill="true">
        <FillColor Value="230 140 20"/>
        <AbbreviatedData>M 25 27 A 25 25 0 0 1 75 27 A 25 25 0 0 1 25 27 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// A page 297 mm wide by 100 mm tall: a landscape box, so a backend that
/// assumes the golden set's usual portrait proportions renders it squashed.
pub const WIDE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 297 100</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="20 20 257 60" Fill="true">
        <FillColor Value="60 60 120"/>
        <AbbreviatedData>M 0 0 L 257 0 L 257 60 L 0 60 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// One entry in the golden set: a name, the page content, and the page box.
/// The box is per-document because a fixture that only ever exercised one page
/// size would leave the size path itself uncompared.
pub struct GoldenDoc {
    /// Stable file stem, used for the reference PNG.
    pub name: &'static str,
    /// The page content entry.
    pub content: &'static str,
    /// The page box as `x y width height`, in millimetres.
    pub page_box: &'static str,
    /// The `DrawParam` block this document needs, spliced into `CommonData`,
    /// or an empty string when the page names no draw parameters.
    pub draw_params: &'static str,
}

/// Every golden fixture, in the order they are compared.
pub const DOCS: &[GoldenDoc] = &[
    GoldenDoc {
        name: "solid",
        content: SOLID_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "clip",
        content: CLIP_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "gradient",
        content: GRADIENT_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "radial",
        content: RADIAL_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "dash",
        content: DASH_XML,
        page_box: "0 0 100 60",
        draw_params: r#"<DrawParams><DrawParam ID="d1" DashPattern="6 4" LineWidth="3"/></DrawParams>"#,
    },
    GoldenDoc {
        name: "evenodd",
        content: EVENODD_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "strokeonly",
        content: STROKEONLY_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "arcs",
        content: ARCS_XML,
        page_box: "0 0 100 60",
        draw_params: "",
    },
    GoldenDoc {
        name: "wide",
        content: WIDE_XML,
        page_box: "0 0 297 100",
        draw_params: "",
    },
];

/// The archive bytes for one golden document: a single page, an embedded face
/// and the public resource table that declares it.
pub fn golden_archive(doc: &GoldenDoc) -> Vec<u8> {
    let box_ = doc.page_box;
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData><PageArea><PhysicalBox>{box_}</PhysicalBox></PageArea>{params}</CommonData>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#,
        params = doc.draw_params,
    );
    let entries = [
        ("OFD.xml", super::document::OFD_XML.as_bytes().to_vec()),
        (
            super::document::DOCUMENT_ENTRY,
            document.as_bytes().to_vec(),
        ),
        (
            super::document::CONTENT_ENTRY,
            doc.content.as_bytes().to_vec(),
        ),
        ("Doc_0/Res/Orbitron-Bold.ttf", super::document::font_bytes()),
        (
            super::document::PUBLIC_RES_ENTRY,
            super::document::PUBLIC_RES_XML.as_bytes().to_vec(),
        ),
    ];
    let borrowed: Vec<(&str, &[u8])> = entries.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    build_archive(&borrowed).expect("golden archive")
}
