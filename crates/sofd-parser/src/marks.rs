//! The files that hang off the side of a document: annotations, custom tags
//! and extension blocks. None of them draw anything, so they are read here
//! rather than flattened into the display list.

use sofd_error::{OfdError, Result, XmlError};
use sofd_package::{Package, parent_of, resolve};
use sofd_xml::cursor::{Cursor, Event};

use crate::ParsedDocument;

/// The index that says which annotation file belongs to which page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationsIndex {
    /// One entry per page that carries annotations.
    pub pages: Vec<AnnotPage>,
}

/// One page's entry in the annotation index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotPage {
    /// Id of the page these annotations sit on.
    pub page_id: String,
    /// Archive path of the page's annotation file.
    pub file_loc: String,
}

/// One annotation, resolved onto the page index it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedAnnot {
    /// Zero-based page index the annotation marks up.
    pub page: usize,
    /// The annotation itself.
    pub annot: Annot,
}

/// Read every annotation in a document, in page order. The index names them
/// by page id rather than by index, so the two have to be joined; a page whose
/// file will not read costs its own annotations and no others.
pub fn read_annotations(
    package: &mut Package,
    parsed: &ParsedDocument,
) -> Result<Vec<ResolvedAnnot>> {
    let Some(loc) = annotation_index_loc(parsed) else {
        return Ok(Vec::new());
    };
    // The document names the index relative to its own directory, as every
    // other location in `Document.xml` is.
    let loc = resolve(&parsed.doc_dir, &loc);
    let index = read_annotations_index(package, &loc)?;
    let by_id = page_ids(parsed);
    let mut out = Vec::new();
    for entry in &index.pages {
        let Some(&page) = by_id.get(&entry.page_id) else {
            continue;
        };
        let file = resolve(&parent_of(&loc), &entry.file_loc);
        let Ok(annots) = read_page_annots(package, &file) else {
            continue;
        };
        out.extend(
            annots
                .items
                .into_iter()
                .map(|annot| ResolvedAnnot { page, annot }),
        );
    }
    Ok(out)
}

/// Where the annotation index sits, from the document body that names it.
fn annotation_index_loc(parsed: &ParsedDocument) -> Option<String> {
    parsed
        .document
        .annotations
        .as_ref()
        .map(|loc| loc.value.clone())
}

/// Page id to zero-based page index, in document order.
fn page_ids(parsed: &ParsedDocument) -> std::collections::HashMap<String, usize> {
    parsed
        .document
        .pages
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| (page.id.clone(), index))
        .collect()
}

/// One `Annot` inside a page annotation file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Annot {
    /// Identifier of the annotation.
    pub id: String,
    /// What kind of mark it is, as written.
    pub kind: String,
    /// Free-text note, when present.
    pub remark: Option<String>,
    /// Name/value pairs from `Parameters`.
    pub parameters: Vec<(String, String)>,
    /// Whether the annotation is drawn.
    pub visible: bool,
    /// Where the annotation's own appearance is drawn.
    pub boundary: Option<sofd_common::Rect>,
}

/// A page annotation file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageAnnots {
    /// Every annotation on the page, in document order.
    pub items: Vec<Annot>,
}

/// One `CustomTag` from the custom tag index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomTag {
    /// Namespace the tag belongs to.
    pub namespace: String,
    /// Optional schema describing the tag's payload.
    pub schema_loc: Option<String>,
    /// Archive path of the tag's content file.
    pub file_loc: String,
}

/// The custom tag index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomTags {
    /// Every registered tag.
    pub tags: Vec<CustomTag>,
}

/// One `Extension` block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extension {
    /// Id of the object this extension attaches to.
    pub ref_id: String,
    /// Name of the application that wrote it.
    pub app_name: String,
    /// Vendor, when given.
    pub company: Option<String>,
    /// Application version, when given.
    pub app_version: Option<String>,
    /// Name/value properties.
    pub properties: Vec<(String, String)>,
    /// Paths of extension data held in their own files.
    pub extend_data: Vec<String>,
    /// Whether a free-form `Data` block was present.
    pub has_data: bool,
}

/// The extension index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    /// Every extension block.
    pub items: Vec<Extension>,
}

/// Read the annotation index, `Annotations.xml`.
pub fn read_annotations_index(package: &mut Package, loc: &str) -> Result<AnnotationsIndex> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut index = AnnotationsIndex::default();
    let mut pending: Option<AnnotPage> = None;
    let mut in_file_loc = false;
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Page") => {
                let a = n.attributes();
                pending = Some(AnnotPage {
                    page_id: a.get("PageID").unwrap_or_default().to_string(),
                    file_loc: String::new(),
                });
            }
            Event::Start(n) if n.is("FileLoc") => in_file_loc = true,
            Event::Start(_) => {}
            Event::Text(t) if in_file_loc => {
                if let Some(page) = pending.as_mut() {
                    page.file_loc = t.trim().to_string();
                }
            }
            Event::Text(_) => {}
            Event::End(_) => {
                if in_file_loc {
                    in_file_loc = false;
                } else if let Some(page) = pending.take()
                    && !page.file_loc.is_empty()
                {
                    index.pages.push(page);
                }
            }
            Event::Eof => break,
        }
    }
    Ok(index)
}

/// Read one page annotation file.
pub fn read_page_annots(package: &mut Package, loc: &str) -> Result<PageAnnots> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut page = PageAnnots::default();
    let mut current: Option<Annot> = None;
    // A stack of open element names, so text is attributed to the element it
    // actually sits in rather than to whichever flag was last set.
    let mut open: Vec<String> = Vec::new();
    // Depth at which the annotation being read opened, if one is in flight.
    let mut annot_depth: Option<usize> = None;
    let mut param_name = String::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) => {
                let name = n.local().to_string();
                let a = n.attributes();
                if name == "Annot" {
                    let annot = Annot {
                        id: a.get("ID").unwrap_or_default().to_string(),
                        kind: a.get("Type").unwrap_or_default().to_string(),
                        visible: a
                            .get("Visible")
                            .map(|v| !v.eq_ignore_ascii_case("false"))
                            .unwrap_or(true),
                        ..Default::default()
                    };
                    // A self-closing Annot is complete as it opens; there is no
                    // End event coming to close it.
                    if n.is_self_closing() {
                        page.items.push(annot);
                    } else {
                        current = Some(annot);
                        annot_depth = Some(open.len());
                    }
                } else if name == "Parameter" {
                    param_name = a.get("Name").unwrap_or_default().to_string();
                } else if name == "Appearance" {
                    // Boundary rides on the Appearance element itself, so even
                    // a self-closing Appearance carries its rectangle.
                    if let (Some(cur), Some(text)) = (current.as_mut(), a.get("Boundary")) {
                        cur.boundary = parse_box(text);
                    }
                }
                if !n.is_self_closing() {
                    open.push(name);
                }
            }
            Event::Text(t) => {
                let text = t.trim();
                if text.is_empty() {
                    continue;
                }
                let Some(cur) = current.as_mut() else {
                    continue;
                };
                match open.last().map(String::as_str) {
                    Some("Remark") => cur.remark = Some(text.to_string()),
                    Some("Parameter") => {
                        cur.parameters.push((param_name.clone(), text.to_string()))
                    }
                    Some("Boundary") => cur.boundary = parse_box(text),
                    _ => {}
                }
            }
            Event::End(_) => {
                open.pop();
                // The Annot element closing means that annotation is complete,
                // which is exactly when the stack returns to the depth it had
                // before the annotation opened.
                if annot_depth == Some(open.len()) {
                    annot_depth = None;
                    if let Some(done) = current.take() {
                        page.items.push(done);
                    }
                }
            }
            Event::Eof => break,
        }
    }
    Ok(page)
}

/// Read the custom tag index.
pub fn read_custom_tags(package: &mut Package, loc: &str) -> Result<CustomTags> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut tags = CustomTags::default();
    let mut current: Option<CustomTag> = None;
    let mut in_schema = false;
    let mut in_file = false;
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("CustomTag") => {
                current = Some(CustomTag {
                    namespace: n
                        .attributes()
                        .get("NameSpace")
                        .unwrap_or_default()
                        .to_string(),
                    ..Default::default()
                });
            }
            Event::Start(n) if n.is("SchemaLoc") => in_schema = true,
            Event::Start(n) if n.is("FileLoc") => in_file = true,
            Event::Start(_) => {}
            Event::Text(t) => {
                let t = t.trim();
                if let Some(cur) = current.as_mut() {
                    if in_schema {
                        cur.schema_loc = Some(t.to_string());
                    } else if in_file {
                        cur.file_loc = t.to_string();
                    }
                }
            }
            Event::End(_) => {
                if in_schema {
                    in_schema = false;
                } else if in_file {
                    in_file = false;
                } else if let Some(done) = current.take()
                    && !done.file_loc.is_empty()
                {
                    tags.tags.push(done);
                }
            }
            Event::Eof => break,
        }
    }
    Ok(tags)
}

/// Read the extension index.
pub fn read_extensions(package: &mut Package, loc: &str) -> Result<Extensions> {
    let xml = package.read_string(loc)?;
    let mut cursor = Cursor::new(&xml);
    let mut out = Extensions::default();
    let mut current: Option<Extension> = None;
    let mut in_property: Option<String> = None;
    let mut in_extend_data = false;
    let mut depth = 0usize;
    loop {
        match cursor.next_event()? {
            Event::Start(n) => {
                depth += 1;
                let a = n.attributes();
                if n.is("Extension") {
                    current = Some(Extension {
                        ref_id: a.get("RefId").unwrap_or_default().to_string(),
                        app_name: a.get("AppName").unwrap_or_default().to_string(),
                        company: a.get("Company").map(str::to_string),
                        app_version: a.get("AppVersion").map(str::to_string),
                        ..Default::default()
                    });
                } else if n.is("Property") {
                    in_property = Some(a.get("Name").unwrap_or_default().to_string());
                } else if n.is("ExtendData") {
                    in_extend_data = true;
                } else if n.is("Data")
                    && let Some(cur) = current.as_mut()
                {
                    cur.has_data = true;
                }
            }
            Event::Text(t) => {
                let t = t.trim();
                if let Some(name) = in_property.clone() {
                    if let Some(cur) = current.as_mut() {
                        cur.properties.push((name, t.to_string()));
                    }
                } else if in_extend_data && let Some(cur) = current.as_mut() {
                    cur.extend_data.push(t.to_string());
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if in_property.is_some() {
                    in_property = None;
                } else if in_extend_data {
                    in_extend_data = false;
                } else if let Some(done) = current.take() {
                    out.items.push(done);
                }
                if depth == 0 {
                    break;
                }
            }
            Event::Eof => break,
        }
    }
    Ok(out)
}

/// `ST_Box` written as `left top width height` in element text.
fn parse_box(text: &str) -> Option<sofd_common::Rect> {
    let v: Vec<f64> = text
        .split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    if v.len() < 4 {
        return None;
    }
    Some(sofd_common::Rect::new(v[0], v[1], v[2], v[3]))
}

/// Kept so the error type stays reachable if a reader needs to reject a shape.
#[allow(dead_code)]
fn unsupported(kind: &str) -> OfdError {
    OfdError::Xml(XmlError::UnsupportedExtension(kind.to_string()))
}
