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
    /// How a reader should open the document, clause 7.11.
    #[serde(rename = "VPreferences", skip_serializing_if = "Option::is_none")]
    pub vpreferences: Option<VPreferences>,
    /// What the document permits, clause 7.12.
    #[serde(rename = "Permissions", skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permission>,
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

/// GB/T 33190 clause 7.11: how a reader should open the document. Every field
/// is a preference, not a constraint, so an absent value means "no opinion"
/// rather than a default this model imposes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VPreferences {
    /// Initial view mode, `PageMode` in the schema.
    #[serde(rename = "PageMode", skip_serializing_if = "Option::is_none")]
    pub page_mode: Option<PageMode>,
    /// How pages are laid out side by side.
    #[serde(rename = "PageLayout", skip_serializing_if = "Option::is_none")]
    pub page_layout: Option<PageLayout>,
    /// What the tab shows, the document title or the file name.
    #[serde(rename = "TabDisplay", skip_serializing_if = "Option::is_none")]
    pub tab_display: Option<TabDisplay>,
    /// Whether the toolbar starts hidden.
    #[serde(rename = "HideToolbar", skip_serializing_if = "Option::is_none")]
    pub hide_toolbar: Option<bool>,
    /// Whether the menu bar starts hidden.
    #[serde(rename = "HideMenubar", skip_serializing_if = "Option::is_none")]
    pub hide_menubar: Option<bool>,
    /// Whether the surrounding window chrome starts hidden.
    #[serde(rename = "HideWindowUI", skip_serializing_if = "Option::is_none")]
    pub hide_window_ui: Option<bool>,
    /// Initial zoom, either a named mode or an explicit factor.
    #[serde(flatten)]
    pub zoom: ZoomSetting,
}

/// Initial view mode, `CT_VPreferences`'s `PageMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageMode {
    /// No particular mode.
    #[serde(rename = "None")]
    None,
    /// Full-screen presentation.
    #[serde(rename = "FullScreen")]
    FullScreen,
    /// With the outline pane open.
    #[serde(rename = "UseOutlines")]
    UseOutlines,
    /// With the thumbnail pane open.
    #[serde(rename = "UseThumbs")]
    UseThumbs,
    /// With the custom-tag pane open.
    #[serde(rename = "UseCustomTags")]
    UseCustomTags,
    /// With the layer pane open.
    #[serde(rename = "UseLayers")]
    UseLayers,
    /// With the attachment pane open. The schema spells this `UseAttatchs`.
    #[serde(rename = "UseAttatchs")]
    UseAttachments,
    /// With the bookmark pane open.
    #[serde(rename = "UseBookmarks")]
    UseBookmarks,
}

/// How pages sit next to one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageLayout {
    /// One page at a time.
    #[serde(rename = "OnePage")]
    OnePage,
    /// A single scrolling column.
    #[serde(rename = "OneColumn")]
    OneColumn,
    /// Two pages side by side, left page first.
    #[serde(rename = "TwoPageL")]
    TwoPageL,
    /// Two scrolling columns, left column first.
    #[serde(rename = "TwoColumnL")]
    TwoColumnL,
    /// Two pages side by side, right page first.
    #[serde(rename = "TwoPageR")]
    TwoPageR,
    /// Two scrolling columns, right column first.
    #[serde(rename = "TwoColumnR")]
    TwoColumnR,
}

/// What the document's tab is labelled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabDisplay {
    /// The document title.
    #[serde(rename = "DocTitle")]
    DocTitle,
    /// The file name.
    #[serde(rename = "FileName")]
    FileName,
}

/// Initial zoom, which the schema offers as either a named mode or a factor.
/// The two are exclusive, so the absent case is spelled out here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ZoomSetting {
    /// No zoom preference was stated.
    #[default]
    Unset,
    /// A named fit mode, `ZoomMode` in the schema.
    ZoomMode(ZoomMode),
    /// An explicit magnification factor, `Zoom` in the schema, where 1.0 is
    /// 100%.
    Zoom(f64),
}

/// The named zoom modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoomMode {
    /// Whatever the reader considers sensible.
    #[serde(rename = "Default")]
    Default,
    /// Fit the page height.
    #[serde(rename = "FitHeight")]
    FitHeight,
    /// Fit the page width.
    #[serde(rename = "FitWidth")]
    FitWidth,
    /// Fit the given rectangle.
    #[serde(rename = "FitRect")]
    FitRect,
}

/// GB/T 33190 clause 7.12: what the document permits a reader to do, and the
/// window in which it may be used. Each flag is independent and optional, so
/// an absent one stays absent rather than being defaulted here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Permission {
    /// Whether the content may be edited.
    #[serde(rename = "Edit", skip_serializing_if = "Option::is_none")]
    pub edit: Option<bool>,
    /// Whether annotations may be added.
    #[serde(rename = "Annot", skip_serializing_if = "Option::is_none")]
    pub annotate: Option<bool>,
    /// Whether content may be exported.
    #[serde(rename = "Export", skip_serializing_if = "Option::is_none")]
    pub export: Option<bool>,
    /// Whether the document may be signed.
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    pub signature: Option<bool>,
    /// Whether a watermark may be applied.
    #[serde(rename = "Watermark", skip_serializing_if = "Option::is_none")]
    pub watermark: Option<bool>,
    /// Whether the screen may be captured.
    #[serde(rename = "PrintScreen", skip_serializing_if = "Option::is_none")]
    pub print_screen: Option<bool>,
    /// Print permission, which carries a copy count of its own.
    #[serde(rename = "Print", skip_serializing_if = "Option::is_none")]
    pub print: Option<PrintPermission>,
    /// The window during which the document may be used.
    #[serde(rename = "ValidPeriod", skip_serializing_if = "Option::is_none")]
    pub valid_period: Option<ValidPeriod>,
}

/// Print permission, `Print` inside `CT_Permission`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintPermission {
    /// Whether printing is allowed at all.
    #[serde(rename = "@Printable")]
    pub printable: bool,
    /// How many copies may be printed; `-1` means no stated limit.
    #[serde(rename = "@Copies", default = "unlimited_copies")]
    pub copies: i32,
}

/// The schema's sentinel for "no copy limit".
pub fn unlimited_copies() -> i32 {
    -1
}

/// A validity window, `ValidPeriod` inside `CT_Permission`. Both ends are
/// optional, and they are kept as written rather than parsed into a date type,
/// since the model has no calendar and a reader only needs to compare strings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidPeriod {
    /// Earliest moment the document may be used.
    #[serde(rename = "@StartDate", skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Latest moment the document may be used.
    #[serde(rename = "@EndDate", skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
}

/// GB/T 33190 clause 7.13: a colour space a document declares. An OFD image
/// may name one by resource id instead of carrying sRGB samples, so a reader
/// that ignores the table cannot interpret those images at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorSpaceList {
    /// One entry per declared colour space.
    #[serde(rename = "ColorSpace", default)]
    pub items: Vec<ColorSpace>,
}

/// One declared colour space, `CT_ColorSpace`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorSpace {
    /// Resource id, referenced from an image's `ResourceID`.
    pub id: crate::RefId,
    /// Which family of channels the space uses.
    pub kind: ColorSpaceKind,
    /// Bits used per channel, 8 unless the writer says otherwise.
    pub bits_per_component: u8,
    /// Package-relative path of an ICC profile, when one is supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Palette entries for an indexed space, each a space-separated channel
    /// list as written. Empty for a direct-colour space.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
}

/// An element's text that is not one of the values the schema allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEnumValue(pub String);

impl std::fmt::Display for UnknownEnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for UnknownEnumValue {}

/// Parse a tagged enumeration, rejecting anything the schema does not list.
macro_rules! tagged_enum {
    ($ty:ident { $($tag:literal => $variant:ident),+ $(,)? }) => {
        impl std::str::FromStr for $ty {
            type Err = UnknownEnumValue;

            fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
                match raw {
                    $($tag => Ok(Self::$variant),)+
                    other => Err(UnknownEnumValue(other.to_string())),
                }
            }
        }
    };
}

/// The three colour families `CT_ColorSpace` allows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorSpaceKind {
    /// A single grey channel.
    #[serde(rename = "GRAY")]
    Gray,
    /// Red, green and blue.
    #[default]
    #[serde(rename = "RGB")]
    Rgb,
    /// Cyan, magenta, yellow and black.
    #[serde(rename = "CMYK")]
    Cmyk,
}

tagged_enum!(ColorSpaceKind {
    "GRAY" => Gray,
    "RGB" => Rgb,
    "CMYK" => Cmyk,
});

tagged_enum!(PageMode {
    "None" => None,
    "FullScreen" => FullScreen,
    "UseOutlines" => UseOutlines,
    "UseThumbs" => UseThumbs,
    "UseCustomTags" => UseCustomTags,
    "UseLayers" => UseLayers,
    "UseAttatchs" => UseAttachments,
    "UseBookmarks" => UseBookmarks,
});

tagged_enum!(PageLayout {
    "OnePage" => OnePage,
    "OneColumn" => OneColumn,
    "TwoPageL" => TwoPageL,
    "TwoColumnL" => TwoColumnL,
    "TwoPageR" => TwoPageR,
    "TwoColumnR" => TwoColumnR,
});

tagged_enum!(TabDisplay {
    "DocTitle" => DocTitle,
    "FileName" => FileName,
});

tagged_enum!(ZoomMode {
    "Default" => Default,
    "FitHeight" => FitHeight,
    "FitWidth" => FitWidth,
    "FitRect" => FitRect,
});

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

/// GB/T 33190 clause 7.8: the fonts a document embeds. A text object names
/// one by id, and the declaration is the only thing that says which file in
/// the package that id means.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontList {
    /// One entry per declared font.
    #[serde(rename = "Font", default)]
    pub items: Vec<Font>,
}

/// One declared font.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Font {
    /// Resource id, referenced by a text object's `Font`.
    #[serde(rename = "ID")]
    pub id: crate::RefId,
    /// Name of the font as the writer knows it.
    #[serde(rename = "FontName", default)]
    pub font_name: String,
    /// Family name, when the declaration carries one.
    #[serde(rename = "FamilyName", skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
    /// Character set the face covers, such as `prc` or `unicode`.
    #[serde(rename = "CharSet", skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Whether the face is bold.
    #[serde(rename = "Bold", skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// Path of the font file, relative to the table's `BaseLoc`.
    #[serde(rename = "FontFile", default)]
    pub font_file: String,
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
            vpreferences: None,
            permissions: None,
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
            vpreferences: None,
            permissions: None,
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
