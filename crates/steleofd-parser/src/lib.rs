//! OFD document parser: entry point discovery, page-tree loading and
//! flattening of the nested template tree into display lists.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod content;
pub mod flatten;
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
        let entry = self.package.entry_point()?;
        let xml = self.package.read_string(&entry)?;
        let ofd = content::read_ofd_root(&xml)?;
        ofd.validate()?;
        let body = ofd.primary_body()?;
        let doc_dir = parent_of(&body.doc_root.value).to_string();
        let doc_xml = self.package.read_string(&body.doc_root.value)?;
        let document = content::read_document(&doc_xml)?;
        Ok(ParsedDocument {
            ofd,
            document,
            doc_dir,
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
        Ok(builder.finish(index))
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

    /// Pages may live in their own file behind `BaseLoc`; materialize either
    /// form into an owned page.
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
