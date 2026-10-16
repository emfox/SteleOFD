//! OFD document parser: entry point discovery, page-tree loading and
//! flattening of the nested template tree into display lists.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod content;
pub mod flatten;
pub mod fonts;
pub mod images;
pub mod marks;
pub mod pathdata;

use std::collections::HashMap;

use steleofd_display::{DisplayList, PageDisplay};
use steleofd_error::Result;
use steleofd_model::document::{Document, Template};
use steleofd_model::object::DrawParam;
use steleofd_model::{OfdRoot, Page, Valid};
use steleofd_package::{Package, parent_of, resolve};
use steleofd_xml::cursor::DEFAULT_MAX_DEPTH;

/// Everything the parser learned from opening a document.
pub struct ParsedDocument {
    /// The container entry point, `OFD.xml`.
    pub ofd: OfdRoot,
    /// The primary document, `Document.xml`.
    pub document: Document,
    /// Directory of the document inside the archive, the base for all
    /// relative references made from within the document.
    pub doc_dir: String,
    /// Image resources the document declares, with their bytes read in.
    pub images: images::ImageTable,
}

/// Reads OFD files from a [`Package`] and flattens pages on demand.
pub struct Parser<'a> {
    package: &'a mut Package,
    max_depth: usize,
}

impl<'a> Parser<'a> {
    /// Open a parser over a package.
    pub fn new(package: &'a mut Package) -> Self {
        Self {
            package,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Cap the nesting depth used for both XML reading and graphics state.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Load the entry point and the primary document.
    pub fn open_document(&mut self) -> Result<ParsedDocument> {
        if !self.package.has_entry_point() {
            return Err(steleofd_error::OfdError::Format(
                steleofd_error::FormatError::NotOfd,
            ));
        }
        let xml = self.package.read_string(steleofd_package::ENTRY_POINT)?;
        let ofd = content::read_ofd_root(&xml)?;
        ofd.validate()?;
        let body = ofd.primary_body()?;
        let doc_dir = parent_of(&body.doc_root.value).to_string();
        let doc_xml = self.package.read_string(&body.doc_root.value)?;
        let document = content::read_document(&doc_xml)?;
        let images = images::load_images(self.package, &doc_dir)?;
        Ok(ParsedDocument {
            ofd,
            document,
            doc_dir,
            images,
        })
    }

    /// Flatten one page into a display list.
    /// Templates named by the page are drawn first, in listed order, then the
    /// page's own content on top.
    pub fn page_display(&mut self, doc: &ParsedDocument, index: usize) -> Result<PageDisplay> {
        let page_ref = doc.document.pages.get(index)?;
        let page = self.load_page(doc, page_ref)?;
        let page_box = page
            .area
            .as_ref()
            .map(|a| a.physical_box)
            .unwrap_or(doc.document.common_data.page_area.physical_box);
        let mut builder =
            steleofd_display::DisplayListBuilder::new(page_box).with_max_depth(self.max_depth);
        let params: HashMap<String, DrawParam> = doc
            .document
            .common_data
            .draw_params
            .as_ref()
            .map(|list| {
                list.items
                    .iter()
                    .map(|p| (p.id.clone(), p.clone()))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(tpls) = &page.templates {
            let table = template_table(doc);
            for tref in &tpls.items {
                let template = table
                    .get(tref.template_id.as_str())
                    .ok_or_else(|| {
                        steleofd_error::OfdError::Format(
                            steleofd_error::FormatError::DanglingReference {
                                kind: "Template",
                                id: tref.template_id.clone(),
                            },
                        )
                    })?
                    .clone();
                let block = self.load_template(doc, &template)?;
                flatten::flatten_block(&block, &params, &mut builder)?;
            }
        }
        if let Some(inline) = &page.content {
            flatten::flatten_block(inline, &params, &mut builder)?;
        }
        let mut page = builder.finish(index);
        // The flattened list names image resources but holds no bytes, so they
        // are filled in here, where the archive is still to hand.
        images::attach_images(&mut page, &doc.images);
        Ok(page)
    }

    /// Flatten a page range into a display list for the whole document.
    pub fn display_list(
        &mut self,
        doc: &ParsedDocument,
        start: usize,
        count: usize,
    ) -> Result<DisplayList> {
        let total = doc.document.pages.len();
        let end = (start.saturating_add(count)).min(total);
        let mut pages = Vec::new();
        for index in start..end {
            pages.push(self.page_display(doc, index)?);
        }
        Ok(DisplayList {
            page_count: total,
            pages,
        })
    }

    /// Load one page's authored model, following its `BaseLoc` when the page
    /// content lives in its own file.
    pub fn page_model(&mut self, doc: &ParsedDocument, index: usize) -> Result<Page> {
        let page_ref = doc.document.pages.get(index)?;
        self.load_page(doc, page_ref)
    }

    fn load_page(&mut self, doc: &ParsedDocument, page: &Page) -> Result<Page> {
        match &page.base_loc {
            None => Ok(page.clone()),
            Some(loc) => {
                let path = resolve(&doc.doc_dir, loc);
                let xml = self.package.read_string(&path)?;
                content::read_page_file(&xml)
            }
        }
    }

    fn load_template(
        &mut self,
        doc: &ParsedDocument,
        template: &Template,
    ) -> Result<steleofd_model::PageBlock> {
        let path = resolve(&doc.doc_dir, &template.base_loc);
        let xml = self.package.read_string(&path)?;
        content::read_template_block(&xml)
    }
}

fn template_table(doc: &ParsedDocument) -> HashMap<&str, Template> {
    doc.document
        .common_data
        .templates
        .as_ref()
        .map(|list| {
            list.items
                .iter()
                .map(|t| (t.id.as_str(), t.clone()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_display::DrawOp;
    use steleofd_package::build_archive;

    /// The drawing commands of a page, without the save/transform/restore
    /// brackets that place each object in its own space.
    fn drawn(page: &steleofd_display::PageDisplay) -> Vec<&DrawOp> {
        page.ops
            .iter()
            .filter(|op| !matches!(op, DrawOp::Save | DrawOp::Restore | DrawOp::Transform(_)))
            .collect()
    }

    const OFD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFD Version="1.0">
  <DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody>
</OFD>"#;

    const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document>
  <CommonData>
    <PageArea><PhysicalBox Left="0" Top="0" Width="210" Height="297"/></PageArea>
    <Templates>
      <Template ID="1" BaseLoc="Tpls/Tpl_0/Content.xml"/>
    </Templates>
  </CommonData>
  <Pages>
    <Page ID="1">
      <Content>
        <Layer>
          <PathObject ID="10" Boundary="10 10 20 20">
            <AbbreviatedData>M 10 10 L 30 10 L 30 30 L 10 30 Z</AbbreviatedData>
          </PathObject>
        </Layer>
      </Content>
    </Page>
  </Pages>
</Document>"#;

    const TEMPLATE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Content>
    <Layer>
      <PathObject ID="1" Boundary="0 0 10 10" CTM="1 0 0 1 5 5">
        <AbbreviatedData>M 0 0 L 10 0 L 10 10 Z</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;

    fn archive() -> Vec<u8> {
        build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", DOCUMENT_XML.as_bytes()),
            ("Doc_0/Tpls/Tpl_0/Content.xml", TEMPLATE_XML.as_bytes()),
        ])
        .expect("build fixture")
    }

    fn open_parser(bytes: Vec<u8>) -> Parser<'static> {
        // Test-only leak: the package must outlive the parser borrowing it.
        let package = Box::new(Package::from_bytes(&bytes).expect("open package"));
        Parser::new(Box::leak(package))
    }

    #[test]
    fn a_minimal_document_flattens_its_page() {
        let mut parser = open_parser(archive());
        let doc = parser.open_document().expect("open document");
        assert_eq!(doc.doc_dir, "Doc_0");
        assert_eq!(doc.document.pages.len(), 1);

        let page = parser.page_display(&doc, 0).expect("page");
        assert_eq!(page.index, 0);
        assert_eq!(drawn(&page).len(), 1);
        assert!((page.page_box.w - 210.0).abs() < 1e-9);
        assert!((page.page_box.h - 297.0).abs() < 1e-9);
    }

    #[test]
    fn a_template_draws_before_the_page_content_with_its_ctm() {
        let doc_xml = DOCUMENT_XML.replace(
            "<Page ID=\"1\">",
            "<Page ID=\"1\"><Template TemplateID=\"1\"/>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
            ("Doc_0/Tpls/Tpl_0/Content.xml", TEMPLATE_XML.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");

        // The template object sits at the page origin and carries a CTM, so
        // only the CTM enters the state stack.
        assert!(matches!(page.ops[0], DrawOp::Save));
        assert!(matches!(page.ops[1], DrawOp::Transform(t) if t.apply(0.0, 0.0) == (5.0, 5.0)));
        assert!(matches!(page.ops[2], DrawOp::Path { .. }));
        assert!(matches!(page.ops[3], DrawOp::Restore));
        // The page's own object follows on top, entering its boundary origin.
        assert!(matches!(page.ops[4], DrawOp::Save));
        assert!(matches!(page.ops[5], DrawOp::Transform(t) if t.apply(0.0, 0.0) == (10.0, 10.0)));
        assert!(matches!(page.ops[6], DrawOp::Path { .. }));
        assert!(matches!(page.ops[7], DrawOp::Restore));
        assert_eq!(drawn(&page).len(), 2);
    }

    #[test]
    fn a_page_area_override_resizes_the_page() {
        let doc_xml = DOCUMENT_XML.replace(
            "<Page ID=\"1\">",
            "<Page ID=\"1\"><Area><PhysicalBox Left=\"0\" Top=\"0\" Width=\"100\" Height=\"50\"/></Area>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");
        assert!((page.page_box.w - 100.0).abs() < 1e-9);
        assert!((page.page_box.h - 50.0).abs() < 1e-9);
    }

    #[test]
    fn an_external_page_file_is_loaded_through_base_loc() {
        let page_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Page>
  <Content>
    <Layer>
      <PathObject ID="1" Boundary="0 0 5 5">
        <AbbreviatedData>M 0 0 L 5 5</AbbreviatedData>
      </PathObject>
    </Layer>
  </Content>
</Page>"#;
        let doc_xml = DOCUMENT_XML.replace(
            "<Page ID=\"1\">",
            "<Page ID=\"1\" BaseLoc=\"Pages/Page_0/Content.xml\">",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
            ("Doc_0/Pages/Page_0/Content.xml", page_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");
        assert_eq!(page.ops.len(), 1);
    }

    #[test]
    fn a_missing_page_file_is_reported() {
        let doc_xml = DOCUMENT_XML.replace(
            "<Page ID=\"1\">",
            "<Page ID=\"1\" BaseLoc=\"Pages/Page_0/Content.xml\">",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        assert!(parser.page_display(&doc, 0).is_err());
    }

    #[test]
    fn a_dangling_template_reference_is_reported() {
        let doc_xml = DOCUMENT_XML.replace(
            "<Page ID=\"1\">",
            "<Page ID=\"1\"><Template TemplateID=\"999\"/>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let err = parser.page_display(&doc, 0).expect_err("must fail");
        assert!(err.to_string().contains("Template"), "{err}");
    }

    #[test]
    fn draw_params_resolve_from_the_document_table() {
        let doc_xml = DOCUMENT_XML
            .replace(
                "</CommonData>",
                "<DrawParams><DrawParam ID=\"3\"><FillColor Value=\"#0000FF\"/></DrawParam></DrawParams></CommonData>",
            )
            .replace("<PathObject ID=\"10\"", "<PathObject ID=\"10\" DrawParam=\"3\"");
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");
        let drawn_ops = drawn(&page);
        let DrawOp::Path {
            fill: Some(steleofd_display::Paint::Solid(c)),
            ..
        } = drawn_ops[0]
        else {
            panic!("expected a filled path, got {:?}", drawn_ops[0]);
        };
        assert_eq!((c.r, c.g, c.b), (0, 0, 255));
    }

    #[test]
    fn a_missing_entry_point_is_rejected() {
        let bytes = build_archive(&[("Doc_0/Document.xml", DOCUMENT_XML.as_bytes())])
            .expect("build fixture");
        let mut parser = open_parser(bytes);
        assert!(parser.open_document().is_err());
    }

    #[test]
    fn an_entry_point_without_a_document_body_is_rejected() {
        let xml = r#"<OFD Version="1.0"></OFD>"#;
        let bytes = build_archive(&[("OFD.xml", xml.as_bytes())]).expect("build fixture");
        let mut parser = open_parser(bytes);
        let err = match parser.open_document() {
            Err(e) => e,
            Ok(_) => panic!("an empty entry point must fail"),
        };
        assert!(err.to_string().contains("not a valid OFD"), "{err}");
    }

    #[test]
    fn display_list_honours_the_requested_range() {
        let two_pages = DOCUMENT_XML.replace(
            "</Pages>",
            "<Page ID=\"2\"><Content><Layer/></Content></Page></Pages>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", two_pages.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");

        let list = parser.display_list(&doc, 0, 1).expect("list");
        assert_eq!(
            list.page_count, 2,
            "the total is reported even if not all pages are materialized"
        );
        assert_eq!(list.pages.len(), 1);
        assert!(list.page(1).is_none());
        assert!(list.page(0).is_some());
    }

    #[test]
    fn out_of_range_page_indices_are_reported() {
        let mut parser = open_parser(archive());
        let doc = parser.open_document().expect("open document");
        let err = parser.page_display(&doc, 5).expect_err("must fail");
        assert!(err.to_string().contains("out of range"), "{err}");
    }

    #[test]
    fn a_vendor_extension_object_does_not_break_the_page() {
        let doc_xml = DOCUMENT_XML.replace(
            "</Layer>",
            "<VendorWidget ID=\"99\"/><TextObject ID=\"11\" Boundary=\"0 0 10 5\" Font=\"1\" Size=\"3\"><TextCode X=\"1\" Y=\"2\">Hi</TextCode></TextObject></Layer>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");
        // The unknown element is skipped; the text object still renders.
        let drawn_ops = drawn(&page);
        assert_eq!(drawn_ops.len(), 2);
        assert!(matches!(drawn_ops[1], DrawOp::Text(_)));
    }

    #[test]
    fn colors_decode_from_the_element_form_with_alpha() {
        let doc_xml = DOCUMENT_XML.replace(
            "<PathObject ID=\"10\" Boundary=\"10 10 20 20\">",
            "<PathObject ID=\"10\" Boundary=\"10 10 20 20\"><FillColor Value=\"#00FF00\" Alpha=\"128\"/>",
        );
        let bytes = build_archive(&[
            ("OFD.xml", OFD_XML.as_bytes()),
            ("Doc_0/Document.xml", doc_xml.as_bytes()),
        ])
        .expect("build fixture");
        let mut parser = open_parser(bytes);
        let doc = parser.open_document().expect("open document");
        let page = parser.page_display(&doc, 0).expect("page");
        let drawn_ops = drawn(&page);
        let DrawOp::Path {
            fill: Some(steleofd_display::Paint::Solid(c)),
            ..
        } = drawn_ops[0]
        else {
            panic!("expected a filled path, got {:?}", drawn_ops[0]);
        };
        assert_eq!((c.r, c.g, c.b, c.a), (0, 255, 0, 128));
    }
}
