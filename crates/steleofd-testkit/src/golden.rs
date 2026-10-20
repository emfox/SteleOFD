//! The golden-image fixtures: documents whose exact pixels are committed. The
//! drawing ones exercise paths, paint and clips; the font ones pin glyph geometry,
//! which a parser swap moves without any op-level test noticing.

use steleofd_package::build_archive;

use crate::document::{FONT_BYTES, font_file};

/// The resource id the drawing fixtures name. Its face is the shared Orbitron;
/// the font fixtures are numbered on from it, since one resource file serves
/// every page of the book and two faces cannot share an id.
pub const DRAWING_FONT: u16 = 7;

/// A page content entry that draws a filled path and a text run, the shape
/// every backend has to get right for the most common document there is.
pub const SOLID_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="5 5 40 20" Fill="true" Stroke="false">
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
      <PathObject ID="3" Boundary="0 0 100 60" Fill="true" Stroke="false">
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
      <PathObject ID="3" Boundary="5 5 90 50" Fill="true" Stroke="false">
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
      <PathObject ID="3" Boundary="5 5 90 50" Fill="true" Stroke="false">
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
/// `Stroke` is off because the standard defaults it to true.
pub const EVENODD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <PathObject ID="3" Boundary="20 5 60 50" Fill="true" Stroke="false" Rule="Even-Odd">
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
      <PathObject ID="3" Boundary="0 0 100 60" Fill="true" Stroke="false">
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
      <PathObject ID="3" Boundary="20 20 257 60" Fill="true" Stroke="false">
        <FillColor Value="60 60 120"/>
        <AbbreviatedData>M 0 0 L 257 0 L 257 60 L 0 60 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

/// A page of one Latin face at a size large enough for the glyph outlines to
/// show: a hinting or control-point change in the parser moves these pixels
/// while leaving the advance so nearly the same that no op-level test sees it.
pub fn latin_page(font: u16) -> String {
    // The id is filled in by the builder, so one page cannot be copied into
    // another with the wrong face attached.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <TextObject ID="3" Boundary="0 0 90 25" Font="{font}" Size="12">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="18">AVWa</TextCode>
      </TextObject>
      <TextObject ID="4" Boundary="0 0 90 25" Font="{font}" Size="12">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="45">Hamburgefonstiv</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#
    )
}

/// Two characters drawn as three glyphs, clause 11.4.5. Glyphs 1, 13 and 15 of the
/// CJK subset all have outlines, so the page shows the extra glyph getting an
/// advance and a place, which the cluster map alone cannot.
pub fn many_to_many_page(font: u16) -> String {
    // The id is filled in by the builder, so one page cannot be copied into
    // another with the wrong face attached.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <TextObject ID="3" Boundary="5 5 90 50" Font="{font}" Size="12">
        <FillColor Value="0 0 0"/>
        <CGTransform CodePosition="0" CodeCount="2" GlyphCount="3"><Glyphs>1 13 15</Glyphs></CGTransform>
        <TextCode X="6" Y="30">&#x4E2D;&#x56FD;</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#
    )
}

/// Twelve ideographs at a size that fills the page. A CJK advance that lands a
/// hundredth of an em off changes every column position on a real invoice, and
/// nothing in the op-level suites measures the advance in millimetres.
pub fn cjk_page(font: u16) -> String {
    // The id is filled in by the builder, so one page cannot be copied into
    // another with the wrong face attached.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <TextObject ID="3" Boundary="0 0 90 25" Font="{font}" Size="9">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="22">&#x4E2D;&#x53D1;&#x53F7;&#x56FD;&#x5B50;&#x5E74;</TextCode>
      </TextObject>
      <TextObject ID="4" Boundary="0 0 90 25" Font="{font}" Size="9">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="49">&#x65E5;&#x6708;&#x7535;&#x7968;&#x4F60;&#x597D;</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#
    )
}

/// The same face as a collection. Opening a `.ttc` means picking a face out of
/// a directory of them, and the wrong index draws a plausible page of the wrong
/// glyphs, which only a pixel comparison against the single-face page catches.
pub fn ttc_page(font: u16) -> String {
    // The id is filled in by the builder, so one page cannot be copied into
    // another with the wrong face attached.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <TextObject ID="3" Boundary="5 5 90 50" Font="{font}" Size="9">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="30">&#x4E2D;&#x56FD;</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#
    )
}

/// Hebrew through a face the run does not name, so the substitute the fallback
/// chain picks is what gets drawn. Which face that is decides whether the page
/// has letters or boxes, and no assertion on the chain's order sees the ink.
pub fn rtl_page(font: u16) -> String {
    // The id is filled in by the builder, so one page cannot be copied into
    // another with the wrong face attached.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Area><PhysicalBox>0 0 100 60</PhysicalBox></Area>
  <Content>
    <Layer ID="2">
      <TextObject ID="3" Boundary="5 5 90 50" Font="{font}" Size="12">
        <FillColor Value="0 0 0"/>
        <TextCode X="6" Y="30">&#x5D0;&#x5D1;&#x5D2;&#x5D3;</TextCode>
      </TextObject>
    </Layer>
  </Content>
</Page>"#
    )
}

/// How a page's content entry is produced: a constant for the drawing fixtures,
/// or a template for a font fixture, whose text objects have to name the resource
/// id the builder gave that page.
#[derive(Clone, Copy)]
pub enum Content {
    /// Written out once, with no substitution.
    Fixed(&'static str),
    /// A template taking the resource id to declare.
    WithFont(fn(u16) -> String),
}

impl Content {
    /// The entry's bytes for one page.
    fn build(self, font: u16) -> String {
        match self {
            Content::Fixed(text) => text.to_string(),
            Content::WithFont(build) => build(font),
        }
    }
}

/// One entry in the golden set: a name, the page content, and the page box.
/// The box is per-document because a fixture that only ever exercised one page
/// size would leave the size path itself uncompared.
pub struct GoldenDoc {
    /// Stable file stem, used for the reference PNG.
    pub name: &'static str,
    /// The page content entry, built with the resource id its text names.
    pub content: Content,
    /// The page box as `x y width height`, in millimetres.
    pub page_box: &'static str,
    /// The `DrawParam` block this document needs, spliced into `CommonData`,
    /// or an empty string when the page names no draw parameters.
    pub draw_params: &'static str,
    /// The face this document embeds and declares, or `None` for the drawing
    /// fixtures, which carry the shared Orbitron under the id every page uses.
    pub font: Option<FontResource>,
}

/// A face one document embeds, declared under the id its text objects name.
/// The file name is also the fixture the font matrix reads, so a glyph this
/// page draws is a glyph `tests/matrix.rs` has already claimed the face has.
pub struct FontResource {
    /// Family name the declaration gives.
    pub family: &'static str,
    /// File name under `steleofd-font/tests/data`, also its name in the package.
    pub file: &'static str,
}

/// Every golden fixture, in the order they are compared.
pub const DOCS: &[GoldenDoc] = &[
    GoldenDoc {
        name: "solid",
        content: Content::Fixed(SOLID_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "clip",
        content: Content::Fixed(CLIP_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "gradient",
        content: Content::Fixed(GRADIENT_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "radial",
        content: Content::Fixed(RADIAL_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "dash",
        content: Content::Fixed(DASH_XML),
        page_box: "0 0 100 60",
        draw_params: r#"<DrawParams><DrawParam ID="d1" DashPattern="6 4" LineWidth="3"/></DrawParams>"#,
        font: None,
    },
    GoldenDoc {
        name: "evenodd",
        content: Content::Fixed(EVENODD_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "strokeonly",
        content: Content::Fixed(STROKEONLY_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "arcs",
        content: Content::Fixed(ARCS_XML),
        page_box: "0 0 100 60",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "wide",
        content: Content::Fixed(WIDE_XML),
        page_box: "0 0 297 100",
        draw_params: "",
        font: None,
    },
    GoldenDoc {
        name: "latin",
        content: Content::WithFont(latin_page),
        page_box: "0 0 100 60",
        draw_params: "",
        font: Some(FontResource {
            family: "Noto Sans",
            file: "steleofd-latin-subset.ttf",
        }),
    },
    GoldenDoc {
        name: "manytomany",
        content: Content::WithFont(many_to_many_page),
        page_box: "0 0 100 60",
        draw_params: "",
        font: Some(FontResource {
            family: "Noto Sans CJK SC",
            file: "steleofd-cjk-subset.ttf",
        }),
    },
    GoldenDoc {
        name: "cjk",
        content: Content::WithFont(cjk_page),
        page_box: "0 0 100 60",
        draw_params: "",
        font: Some(FontResource {
            family: "Noto Sans CJK SC",
            file: "steleofd-cjk-subset.ttf",
        }),
    },
    GoldenDoc {
        name: "ttc",
        content: Content::WithFont(ttc_page),
        page_box: "0 0 100 60",
        draw_params: "",
        font: Some(FontResource {
            family: "Noto Sans CJK SC",
            file: "steleofd-collection.ttc",
        }),
    },
    GoldenDoc {
        name: "rtl",
        content: Content::WithFont(rtl_page),
        page_box: "0 0 100 60",
        draw_params: "",
        font: Some(FontResource {
            family: "Noto Sans Hebrew",
            file: "steleofd-hebrew-subset.ttf",
        }),
    },
];

/// The archive bytes for one golden document: a single page, an embedded face
/// and the public resource table that declares it.
pub fn golden_archive(doc: &GoldenDoc) -> Vec<u8> {
    build_golden(std::slice::from_ref(doc))
}

/// Every golden fixture as the pages of one document, in `DOCS` order: a
/// single file a third-party renderer can be handed once. The page index then
/// carries meaning, so a wrong page beside a right one names a defect.
pub fn golden_book() -> Vec<u8> {
    build_golden(DOCS)
}

/// Assemble one document from `docs`, one page each.
fn build_golden(docs: &[GoldenDoc]) -> Vec<u8> {
    let mut pages = String::new();
    for index in 0..docs.len() {
        pages.push_str(&format!(
            "    <Page ID=\"{id}\" BaseLoc=\"Pages/Page_{index}/Content.xml\"/>\n",
            id = index + 1,
        ));
    }
    // The common page area is the first document's box; every page overrides
    // it with its own, so a fixture is free to pick any size it needs.
    let box_ = docs.first().map(|d| d.page_box).unwrap_or("0 0 100 60");
    let params = docs
        .iter()
        .map(|d| d.draw_params)
        .collect::<Vec<_>>()
        .join("");
    // Every declaration in the file, so a page can name its own face while the
    // others keep the shared one. Ids must differ, since a resource id is the
    // only thing a text run says about which face it wants.
    let fonts = fonts_of(docs);
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData><PageArea><PhysicalBox>{box_}</PhysicalBox></PageArea>{params}</CommonData>
  <Pages>
{pages}  </Pages>
</Document>"#
    );
    let mut entries = vec![
        (
            "OFD.xml".to_string(),
            super::document::OFD_XML.as_bytes().to_vec(),
        ),
        (
            super::document::DOCUMENT_ENTRY.to_string(),
            document.into_bytes(),
        ),
    ];
    for (index, doc) in docs.iter().enumerate() {
        // Each page names its own id, so the book's pages cannot disagree with
        // the per-document renders about which face they draw.
        entries.push((
            format!("Doc_0/Pages/Page_{index}/Content.xml"),
            doc.content.build(font_id(doc)).into_bytes(),
        ));
    }
    for (file, bytes) in fonts.files {
        entries.push((format!("Doc_0/Res/{file}"), bytes));
    }
    entries.push((
        super::document::PUBLIC_RES_ENTRY.to_string(),
        fonts.declarations.into_bytes(),
    ));
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(p, d)| (p.as_str(), d.as_slice()))
        .collect();
    build_archive(&borrowed).expect("golden archive")
}

/// The id a document's text objects name for their face.
fn font_id(doc: &GoldenDoc) -> u16 {
    match DOCS.iter().position(|d| d.name == doc.name) {
        Some(index) => DRAWING_FONT + index as u16,
        None => DRAWING_FONT,
    }
}

/// The faces a set of documents declares, and the bytes for each once.
struct Fonts {
    /// The whole `PublicRes.xml`, which the parser reads by that fixed name.
    declarations: String,
    /// One entry per distinct file, so a face a page and the book both embed is
    /// stored once and a `.ttc` is not written twice.
    files: Vec<(String, Vec<u8>)>,
}

/// Collect the declarations a set of documents needs.
fn fonts_of(docs: &[GoldenDoc]) -> Fonts {
    let mut entries = String::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut ids: Vec<u16> = Vec::new();
    for doc in docs {
        let Some(font) = doc.font.as_ref() else {
            continue;
        };
        let id = font_id(doc);
        if !ids.contains(&id) {
            ids.push(id);
            entries.push_str(&format!(
                r#"
  <Font ID="{id}" FontName="{family}"><FontFile>{file}</FontFile></Font>"#,
                family = font.family,
                file = font.file,
            ));
        }
        if !files.iter().any(|(name, _)| name == font.file) {
            files.push((font.file.to_string(), font_file(font.file)));
        }
    }
    // The drawing fixtures draw with the shared face under id 7. A font
    // fixture may have claimed 7 already, in which case its declaration is the
    // one that stands and those fixtures are not part of this set.
    if !ids.contains(&DRAWING_FONT) {
        files.push((FONT_BYTES.to_string(), font_file(FONT_BYTES)));
        entries.push_str(&format!(
            r#"
  <Font ID="{DRAWING_FONT}" FontName="Orbitron"><FontFile>{FONT_BYTES}</FontFile></Font>"#
        ));
    }
    Fonts {
        declarations: format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Res BaseLoc=\"Res\"><Fonts>{entries}\n</Fonts></Res>"
        ),
        files,
    }
}
