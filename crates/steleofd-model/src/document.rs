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

/// GB/T 33190 clause 12: the multimedia resources a document embeds, which is
/// where video lives. `Res.xsd` types each entry as `Image`, `Audio` or `Video`
/// and points it at a file inside the package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimediaList {
    /// One entry per embedded multimedia resource.
    #[serde(rename = "MultiMedia", default)]
    pub items: Vec<Multimedia>,
}

/// One multimedia resource.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Multimedia {
    /// Resource id, referenced by a `Movie` or `Sound` action.
    pub id: crate::RefId,
    /// What kind of media this is.
    pub kind: MultimediaKind,
    /// Format hint, such as `mp4` or `wav`, when the writer records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Package-relative path of the media file.
    pub media_file: String,
}

/// The three media kinds clause 12 names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultimediaKind {
    /// A still or animated raster image.
    #[default]
    Image,
    /// A sound.
    Audio,
    /// A video, the case clause 12 exists for.
    Video,
}

impl MultimediaKind {
    /// Read the spelling the schema uses, defaulting to `Image`.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "Audio" => Self::Audio,
            "Video" => Self::Video,
            _ => Self::Image,
        }
    }

    /// The spelling the schema uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Audio => "Audio",
            Self::Video => "Video",
        }
    }
}

/// GB/T 33190 clause 13: the vector graphics a document embeds. A
/// `CompositeObject` on a page names one of these by `ResourceID`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompositeGraphicUnitList {
    /// One entry per embedded vector graphic.
    #[serde(rename = "CompositeGraphicUnit", default)]
    pub items: Vec<CompositeGraphicUnit>,
}

/// One embedded vector graphic, `CT_VectorG`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompositeGraphicUnit {
    /// Resource id, referenced by a `CompositeObject` on a page.
    pub id: crate::RefId,
    /// Natural width of the graphic.
    pub width: f64,
    /// Natural height of the graphic.
    pub height: f64,
    /// Resource id of a preview image, when one is supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<crate::RefId>,
    /// Colour to show where the graphic has no coverage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitution: Option<crate::RefId>,
    /// The drawing itself, a page block like any other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<PageBlock>,
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
    /// External content file, relative to the document root; large documents
    /// keep page bodies out of `Document.xml` and point here instead.
    #[serde(rename = "BaseLoc", skip_serializing_if = "Option::is_none")]
    pub base_loc: Option<String>,
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{DrawParam, GraphicsUnit, Object};

    #[test]
    fn an_empty_root_is_not_a_valid_ofd() {
        let root = OfdRoot::default();
        let err = root.validate().expect_err("must fail");
        assert!(matches!(err, OfdError::Format(FormatError::NotOfd)));
    }

    #[test]
    fn a_body_without_a_root_path_is_rejected() {
        let root = OfdRoot {
            version: Version::default(),
            doc_bodies: vec![DocBody::default()],
        };
        let err = root.validate().expect_err("must fail");
        assert!(matches!(err, OfdError::Format(FormatError::MissingRoot)));
    }

    #[test]
    fn a_root_with_a_document_path_is_valid() {
        let root = OfdRoot {
            version: Version::default(),
            doc_bodies: vec![DocBody {
                doc_root: DocRoot::new("Doc_0/Document.xml"),
                ..Default::default()
            }],
        };
        assert!(root.validate().is_ok());
        assert_eq!(
            root.primary_body().expect("body").doc_root.value,
            "Doc_0/Document.xml"
        );
    }

    #[test]
    fn a_missing_body_is_reported() {
        let root = OfdRoot::default();
        assert!(root.primary_body().is_err());
    }

    #[test]
    fn version_defaults_to_major_one() {
        assert_eq!(Version::default().major, "1");
        assert_eq!(Version::default().sub, None);
    }

    #[test]
    fn locations_convert_from_strings() {
        let l: Location = "Doc_0/Res/Font_0.ttf".into();
        assert_eq!(l.value, "Doc_0/Res/Font_0.ttf");
        assert_eq!(Location::new("a.xml").value, "a.xml");
    }

    #[test]
    fn page_lookups_report_out_of_range_by_index() {
        let pages = Pages {
            pages: vec![Page::default()],
        };
        assert_eq!(pages.len(), 1);
        assert!(!pages.is_empty());
        assert!(pages.get(0).is_ok());

        let err = pages.get(5).expect_err("must fail");
        match err {
            OfdError::Format(FormatError::PageOutOfRange { index, total }) => {
                assert_eq!(index, 5);
                assert_eq!(total, 1);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn an_empty_page_list_reports_empty() {
        let pages = Pages::default();
        assert!(pages.is_empty());
        assert_eq!(pages.len(), 0);
    }

    #[test]
    fn a_page_knows_whether_it_draws_anything() {
        let empty = Page::default();
        assert!(!empty.has_own_content());

        let full = Page {
            content: Some(PageBlock {
                layers: vec![Layer {
                    id: None,
                    objects: vec![Object::unknown()],
                }],
            }),
            ..Default::default()
        };
        assert!(full.has_own_content());
    }

    #[test]
    fn page_blocks_count_objects_across_layers() {
        let block = PageBlock {
            layers: vec![
                Layer {
                    id: Some("1".into()),
                    objects: vec![Object::unknown(), Object::unknown()],
                },
                Layer {
                    id: Some("2".into()),
                    objects: vec![Object::unknown()],
                },
            ],
        };
        assert_eq!(block.object_count(), 3);
        assert!(block.has_content());
    }

    #[test]
    fn a_page_area_wraps_its_box() {
        let area = PageArea::new(Rect::A4_PORTRAIT);
        assert_eq!(area.physical_box, Rect::A4_PORTRAIT);
        assert_eq!(area.application_box, None);
    }

    #[test]
    fn templates_and_draw_params_are_optional() {
        let doc = Document {
            common_data: CommonData {
                page_area: PageArea::new(Rect::A4_PORTRAIT),
                templates: None,
                draw_params: None,
            },
            pages: Pages::default(),
        };
        assert!(doc.common_data.templates.is_none());
        assert!(doc.pages.is_empty());
    }

    #[test]
    fn draw_param_lists_hold_records() {
        let list = DrawParamList {
            items: vec![DrawParam {
                id: "1".to_string(),
                relative_units: GraphicsUnit::Millimetre,
                ..Default::default()
            }],
        };
        assert_eq!(list.items.len(), 1);
    }

    #[test]
    fn documents_survive_a_serde_round_trip() {
        let doc = Document {
            common_data: CommonData {
                page_area: PageArea::new(Rect::A4_PORTRAIT),
                templates: Some(TemplateList {
                    items: vec![Template {
                        id: "1".to_string(),
                        base_loc: "Tpls/Tpl_0/Content.xml".to_string(),
                    }],
                }),
                draw_params: None,
            },
            pages: Pages {
                pages: vec![Page {
                    id: "10".to_string(),
                    base_loc: None,
                    area: None,
                    content: None,
                    templates: Some(TemplatePageList {
                        items: vec![TemplateRef {
                            template_id: "1".to_string(),
                            z_order: None,
                        }],
                    }),
                }],
            },
        };

        let json = serde_json::to_string(&doc).expect("serialize");
        let back: Document = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pages.len(), 1);
        assert_eq!(
            back.common_data.templates.expect("templates").items[0].base_loc,
            "Tpls/Tpl_0/Content.xml"
        );
    }
}
