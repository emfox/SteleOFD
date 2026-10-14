//! Document-level structures from `OFD.xml` and `Document.xml`.
//! `OFD.xml` points at one `DocBody` per document, and each body names its own
//! `Document.xml`; this module models that chain.

use serde::{Deserialize, Serialize};
use steleofd_common::Rect;
use steleofd_error::{FormatError, OfdError};

use crate::{RefId, Valid};

/// Root of `OFD.xml`, the container's entry point.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename = "OFD")]
pub struct OfdRoot {
    /// Version and profile this file conforms to.
    #[serde(rename = "Version", default)]
    pub version: Version,
    /// One entry per document in the file.
    #[serde(rename = "DocBody", default)]
    pub doc_bodies: Vec<DocBody>,
}

impl OfdRoot {
    /// The first document body, which every single-document file has.
    pub fn primary_body(&self) -> steleofd_error::Result<&DocBody> {
        self.doc_bodies.first().ok_or_else(|| {
            OfdError::Format(FormatError::DanglingReference {
                kind: "DocBody",
                id: String::new(),
            })
        })
    }
}

impl Valid for OfdRoot {
    fn validate(&self) -> steleofd_error::Result<()> {
        if self.doc_bodies.is_empty() {
            return Err(OfdError::Format(FormatError::NotOfd));
        }
        for body in &self.doc_bodies {
            if body.doc_root.value.is_empty() {
                return Err(OfdError::Format(FormatError::MissingRoot));
            }
        }
        Ok(())
    }
}

/// Version and conformance profile declared by the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// Major version number, `1` for GB/T 33190-2016.
    #[serde(rename = "Version", default = "default_one")]
    pub major: String,
    /// Minor version or sub-profile.
    #[serde(rename = "SubVersion", skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
}

fn default_one() -> String {
    "1".to_string()
}

impl Default for Version {
    fn default() -> Self {
        Self {
            major: default_one(),
            sub: None,
        }
    }
}

/// One document inside the file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocBody {
    /// Location of this document's `Document.xml`.
    #[serde(rename = "DocRoot")]
    pub doc_root: DocRoot,
    /// Signature descriptions, when the document carries any.
    #[serde(rename = "Signatures", skip_serializing_if = "Option::is_none")]
    pub signatures: Option<Location>,
    /// The document's own version declaration.
    #[serde(rename = "Version", skip_serializing_if = "Option::is_none")]
    pub version: Option<Location>,
}

/// A path to a resource inside the archive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Text content, the archive path.
    #[serde(rename = "$text")]
    pub value: String,
}

impl Location {
    /// Build from an archive path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { value: path.into() }
    }
}

impl From<&str> for Location {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Path to a document root; identical in shape to [`Location`].
pub type DocRoot = Location;

/// Root of `Document.xml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename = "Document")]
pub struct Document {
    /// Shared resources and page extents.
    #[serde(rename = "CommonData")]
    pub common_data: CommonData,
    /// Page listing.
    #[serde(rename = "Pages")]
    pub pages: Pages,
}

/// Templates, resources and default extents shared by every page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommonData {
    /// Extent of every page at index 0.
    #[serde(rename = "PageArea")]
    pub page_area: PageArea,
    /// Templates referenced by pages.
    #[serde(rename = "Templates", skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplateList>,
    /// Default drawing parameters referenced by index.
    #[serde(rename = "DrawParams", skip_serializing_if = "Option::is_none")]
    pub draw_params: Option<DrawParamList>,
}

/// Physical extent of a page, in millimetres.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PageArea {
    /// Page box.
    #[serde(rename = "PhysicalBox", default)]
    pub physical_box: Rect,
    /// Content may also be drawn outside the physical box up to this extent.
    #[serde(rename = "ApplicationBox", skip_serializing_if = "Option::is_none")]
    pub application_box: Option<Rect>,
}

impl PageArea {
    /// A page area sized exactly to its box.
    pub fn new(rect: Rect) -> Self {
        Self {
            physical_box: rect,
            application_box: None,
        }
    }
}

/// List of template resources available to pages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateList {
    /// Individual template pages.
    #[serde(rename = "Template", default)]
    pub items: Vec<Template>,
}

/// One reusable template page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Template {
    /// Reference id.
    #[serde(rename = "ID")]
    pub id: RefId,
    /// Target path relative to the document.
    #[serde(rename = "BaseLoc")]
    pub base_loc: String,
}

/// List of drawing parameter records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawParamList {
    /// Individual parameter records.
    #[serde(rename = "DrawParam", default)]
    pub items: Vec<crate::object::DrawParam>,
}

/// The ordered page tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pages {
    /// One entry per page, in reading order.
    #[serde(rename = "Page", default)]
    pub pages: Vec<Page>,
}

impl Pages {
    /// Number of pages.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether there are no pages.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Look up a page, reporting an out-of-range index by name.
    pub fn get(&self, index: usize) -> steleofd_error::Result<&Page> {
        self.pages
            .get(index)
            .ok_or(OfdError::Format(FormatError::PageOutOfRange {
                index,
                total: self.pages.len(),
            }))
    }
}

/// One page, either holding content directly or delegating to a template.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Page {
    /// Reference id.
    #[serde(rename = "ID")]
    pub id: RefId,
    /// Extent overriding `CommonData.PageArea`.
    #[serde(rename = "Area", skip_serializing_if = "Option::is_none")]
    pub area: Option<PageArea>,
    /// Inline content for pages with no template.
    #[serde(rename = "Content", skip_serializing_if = "Option::is_none")]
    pub content: Option<PageBlock>,
    /// Templates layered beneath the page's own content.
    #[serde(rename = "Template", skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplatePageList>,
}

impl Page {
    /// Whether this page draws anything on its own.
    pub fn has_own_content(&self) -> bool {
        match &self.content {
            Some(c) => c.has_content(),
            None => false,
        }
    }
}

/// Templates a page composes underneath its own content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplatePageList {
    /// One entry per template instance, in draw order.
    #[serde(rename = "Template", default)]
    pub items: Vec<TemplateRef>,
}

/// A template instantiated by a page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateRef {
    /// Id of the template being instantiated.
    #[serde(rename = "TemplateID")]
    pub template_id: RefId,
    /// Additional offset applied to the template's content.
    #[serde(rename = "ZOrder", skip_serializing_if = "Option::is_none")]
    pub z_order: Option<String>,
}

/// Content of a page or template; either inline objects or a list of them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageBlock {
    /// Drawing objects of this block.
    #[serde(rename = "Layer", default)]
    pub layers: Vec<Layer>,
}

impl PageBlock {
    /// Whether any layer holds objects.
    pub fn has_content(&self) -> bool {
        self.layers.iter().any(|l| !l.objects.is_empty())
    }

    /// Total object count across layers.
    pub fn object_count(&self) -> usize {
        self.layers.iter().map(|l| l.objects.len()).sum()
    }
}

/// One layer of drawing objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layer {
    /// Reference id.
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<RefId>,
    /// Objects in this layer, bottom first.
    #[serde(rename = "$value", default)]
    pub objects: Vec<crate::object::Object>,
}

/// Registration for annotation and note areas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Zones {
    /// Individual zones.
    #[serde(rename = "Zone", default)]
    pub items: Vec<Rect>,
}
