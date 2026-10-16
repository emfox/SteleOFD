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
      <TextObject ID="5" Boundary="5 45 60 10" Font="7" Size="8">
        <FillColor Value="0 0 0"/>
        <TextCode X="5" Y="53">Stele</TextCode>
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

/// One entry in the golden set: a name, the page content, and the page box.
pub struct GoldenDoc {
    /// Stable file stem, used for the reference PNG.
    pub name: &'static str,
    /// The page content entry.
    pub content: &'static str,
}

/// Every golden fixture, in the order they are compared.
pub const DOCS: &[GoldenDoc] = &[
    GoldenDoc {
        name: "solid",
        content: SOLID_XML,
    },
    GoldenDoc {
        name: "clip",
        content: CLIP_XML,
    },
    GoldenDoc {
        name: "gradient",
        content: GRADIENT_XML,
    },
];

/// The archive bytes for one golden document: a single page, an embedded face
/// and the public resource table that declares it.
pub fn golden_archive(doc: &GoldenDoc) -> Vec<u8> {
    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData><PageArea><PhysicalBox>0 0 100 60</PhysicalBox></PageArea></CommonData>
  <Pages><Page ID="1" BaseLoc="Pages/Page_0/Content.xml"/></Pages>
</Document>"#;
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
