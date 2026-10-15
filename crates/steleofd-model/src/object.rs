//! Drawing objects of a page or template.
//! These are the authored forms; the parser flattens them into display commands,
//! but they stay here so writing and editing work on the original structure.

use serde::{Deserialize, Serialize};
use steleofd_common::{Millimeter, Rect};

use crate::{RefId, Valid, color::ColorRef};

/// Any object permitted inside a `Layer`.
/// Untagged because `serde(other)` only covers unit variants; each shape denies
/// unknown fields, so anything else falls through to `Unknown` and round trips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Object {
    /// Vector geometry.
    PathObject(PathObject),
    /// Text laid out as runs.
    TextObject(TextObject),
    /// A raster image placement.
    ImageObject(ImageObject),
    /// A group of objects that move, hide and clip as one unit.
    CompositeObject(CompositeObject),
    /// Something unrecognized, skipped at render time.
    Unknown(serde_json::Value),
}

/// GB/T 33190 clause 13: a composite object places a vector resource on the
/// page, the way an image object places a raster one. `CT_Composite` adds only
/// a resource reference to the graphic unit base, so there is no child block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeObject {
    /// Attributes common to every object.
    #[serde(flatten)]
    pub base: ObjectBase,
    /// Id of the vector resource inside the document.
    #[serde(rename = "ResourceID", default)]
    pub resource_id: RefId,
}
impl Object {
    /// Reference id common to every authored object.
    pub fn id(&self) -> Option<&RefId> {
        match self {
            Self::PathObject(o) => o.base.id.as_ref(),
            Self::TextObject(o) => o.base.id.as_ref(),
            Self::ImageObject(o) => o.base.id.as_ref(),
            Self::CompositeObject(o) => o.base.id.as_ref(),
            Self::Unknown(_) => None,
        }
    }

    /// Whether this object actually draws something.
    pub fn is_renderable(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// A placeholder for an element this build does not recognize.
    pub fn unknown() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}

/// Drawing parameters shared by several objects, referenced by index.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DrawParam {
    /// Reference id.
    #[serde(rename = "ID")]
    pub id: RefId,
    /// Unit used by relative commands.
    #[serde(rename = "Relative", default)]
    pub relative_units: GraphicsUnit,
    /// Stroke width in the chosen unit.
    #[serde(rename = "LineWidth", skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    /// How open subpaths terminate.
    #[serde(rename = "LineCap", skip_serializing_if = "Option::is_none")]
    pub line_cap: Option<LineCapStyle>,
    /// How segments meet.
    #[serde(rename = "LineJoin", skip_serializing_if = "Option::is_none")]
    pub line_join: Option<LineJoinStyle>,
    /// Dash pattern with phase.
    #[serde(rename = "DashPattern", skip_serializing_if = "Option::is_none")]
    pub dash_pattern: Option<DashPattern>,
    /// Fill colour.
    #[serde(rename = "FillColor", skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<ColorRef>,
    /// Stroke colour.
    #[serde(rename = "StrokeColor", skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<ColorRef>,
    /// Whether the path is stroked at all.
    #[serde(rename = "Stroke", skip_serializing_if = "Option::is_none")]
    pub stroke: Option<bool>,
    /// Whether the path is filled at all.
    #[serde(rename = "Fill", skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
}

impl Valid for DrawParam {
    fn validate(&self) -> steleofd_error::Result<()> {
        for c in [&self.fill_color, &self.stroke_color].into_iter().flatten() {
            c.validate()?;
        }
        Ok(())
    }
}

/// Unit of an object's coordinate system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsUnit {
    /// Absolute millimetres on the page.
    #[default]
    Millimetre,
    /// Relative to this object's own box.
    Relative,
}

/// How an open subpath terminates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCapStyle {
    /// Stop at the endpoint.
    #[default]
    Butt,
    /// Rounded extension.
    Round,
    /// Square extension.
    Square,
}

/// How two segments meet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineJoinStyle {
    /// Sharp corner.
    #[default]
    Miter,
    /// Rounded corner.
    Round,
    /// Flattened corner.
    Bevel,
}

/// How a path decides which points are inside it, from the `Rule` attribute.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillRuleStyle {
    /// Non-zero winding; the standard's default.
    #[default]
    NonZero,
    /// Even-odd, spelled `Even-Odd` in the standard.
    EvenOdd,
}

/// Dash pattern in units of line width, starting at `phase`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DashPattern {
    /// Offset into the pattern.
    #[serde(rename = "Offset", default)]
    pub phase: f64,
    /// Alternating dash and gap lengths.
    #[serde(rename = "Pattern", default)]
    pub pattern: Vec<f64>,
}

impl DashPattern {
    /// Solid stroke, an empty pattern.
    pub fn solid() -> Self {
        Self::default()
    }

    /// Whether this draws solid.
    pub fn is_solid(&self) -> bool {
        self.pattern.is_empty()
    }
}

/// Attributes every authored object carries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ObjectBase {
    /// Reference id.
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<RefId>,
    /// Object extents.
    #[serde(rename = "Boundary", default)]
    pub boundary: Rect,
    /// Placement transform, `a b c d e f`.
    #[serde(rename = "CTM", skip_serializing_if = "Option::is_none")]
    pub ctm: Option<String>,
    /// Index into the document's draw parameter table.
    #[serde(rename = "DrawParam", skip_serializing_if = "Option::is_none")]
    pub draw_param: Option<RefId>,
    /// Whether the object participates in rendering.
    #[serde(rename = "Visible", skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Replacement colour applied to one-bit images.
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Clip areas that narrow where this object draws, clause 8.4.
    #[serde(rename = "Clips", skip_serializing_if = "Option::is_none")]
    pub clips: Option<ClipList>,
    /// Stroke width written on the object itself, overriding any draw parameter.
    #[serde(rename = "LineWidth", skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    /// End cap written on the object itself.
    #[serde(rename = "Cap", skip_serializing_if = "Option::is_none")]
    pub cap: Option<LineCapStyle>,
    /// Segment join written on the object itself.
    #[serde(rename = "Join", skip_serializing_if = "Option::is_none")]
    pub join: Option<LineJoinStyle>,
    /// Miter limit written on the object itself.
    #[serde(rename = "MiterLimit", skip_serializing_if = "Option::is_none")]
    pub miter_limit: Option<f64>,
    /// What the object does when clicked, hovered or loaded, clause 14.1.
    #[serde(rename = "Actions", skip_serializing_if = "Option::is_none")]
    pub actions: Option<Actions>,
}

/// The `Clips` container of clause 8.4.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipList {
    /// Clips; the region they confine the drawing to is their union.
    #[serde(rename = "Clip", default)]
    pub items: Vec<Clip>,
    /// `TransFlag`: whether the clip follows the object's CTM, true by
    /// default. Real producers write `false` on nearly every clip, since their
    /// areas are already millimetres relative to the boundary.
    #[serde(rename = "TransFlag", default)]
    pub trans_flag: Option<bool>,
}

/// One `Clip`: the areas it is made of, whose union it covers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// Areas in document order.
    #[serde(rename = "Area", default)]
    pub areas: Vec<ClipArea>,
}

/// One `Area`: the geometry one part of a clip is bounded by.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipArea {
    /// The path the clip is bounded by, in abbreviated form.
    #[serde(rename = "PathData", skip_serializing_if = "Option::is_none")]
    pub path_data: Option<String>,
    /// Placement transform for this area, `a b c d e f`.
    #[serde(rename = "CTM", skip_serializing_if = "Option::is_none")]
    pub ctm: Option<String>,
    /// Fill rule read off the area's `Path`, `NonZero` unless it says otherwise.
    #[serde(rename = "Rule", skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// Drawing parameters referenced by this area.
    #[serde(rename = "DrawParam", skip_serializing_if = "Option::is_none")]
    pub draw_param: Option<RefId>,
}

/// One reference from an object to something else.
pub type ObjectRef = RefId;

/// Vector geometry: a stroke and/or fill over a path.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathObject {
    /// Attributes shared by every object.
    #[serde(flatten)]
    pub base: ObjectBase,
    /// Abbreviated path commands (`M`, `L`, `C`, `Q`, `A`, `Z`).
    #[serde(rename = "AbbreviatedData", default)]
    pub abbreviated_data: String,
    /// Fill colour, overriding any draw parameter.
    #[serde(rename = "FillColor", skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<ColorRef>,
    /// Stroke colour, overriding any draw parameter.
    #[serde(rename = "StrokeColor", skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<ColorRef>,
    /// Whether the interior is filled.
    #[serde(rename = "Fill", skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
    /// Whether the outline is stroked.
    #[serde(rename = "Stroke", skip_serializing_if = "Option::is_none")]
    pub stroke: Option<bool>,
    /// Fill rule, `NonZero` (the default) or `Even-Odd`.
    #[serde(rename = "Rule", skip_serializing_if = "Option::is_none")]
    pub rule: Option<FillRuleStyle>,
}

impl PathObject {
    /// The effective presence of each paint, once defaults are applied.
    /// OFD says an absent `Fill` means filled, while an absent `Stroke` means
    /// unstroked, which is asymmetric and easy to get wrong.
    pub fn paints(&self) -> (bool, bool) {
        let fill = self.fill.unwrap_or(true);
        let stroke = self.stroke.unwrap_or(false);
        (fill, stroke)
    }
}

impl Valid for PathObject {
    fn validate(&self) -> steleofd_error::Result<()> {
        if !self.abbreviated_data.trim().is_empty() {
            return Ok(());
        }
        Err(steleofd_error::OfdError::Format(
            steleofd_error::FormatError::MissingPathData,
        ))
    }
}

/// One line of glyphs within a text object.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextCode {
    /// Horizontal origin within the object.
    #[serde(rename = "X", default)]
    pub x: f64,
    /// Vertical origin within the object.
    #[serde(rename = "Y", default)]
    pub y: f64,
    /// Extra advance applied after each glyph.
    #[serde(rename = "DeltaX", default)]
    pub delta_x: Vec<f64>,
    /// Vertical offset applied after each glyph.
    #[serde(rename = "DeltaY", default)]
    pub delta_y: Vec<f64>,
    /// Raw text; either glyph ids or Unicode, depending on `CGTransform`.
    #[serde(rename = "$text", default)]
    pub text: String,
}

/// Text laid out as one or more runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextObject {
    /// Attributes shared by every object.
    #[serde(flatten)]
    pub base: ObjectBase,
    /// Font referenced by this text.
    #[serde(rename = "Font", default)]
    pub font: RefId,
    /// Font magnitude in millimetres.
    #[serde(rename = "Size", default)]
    pub size: Millimeter,
    /// Whether text reads top to bottom.
    #[serde(rename = "Weight", skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
    /// Whether the text is reversed for vertical writing.
    #[serde(rename = "Italic", skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Extra spacing between glyphs.
    #[serde(rename = "CharSpace", skip_serializing_if = "Option::is_none")]
    pub char_space: Option<f64>,
    /// Horizontal glyph scale; `1.0` is unscaled.
    #[serde(rename = "HScale", skip_serializing_if = "Option::is_none")]
    pub h_scale: Option<f64>,
    /// Whether the glyph outlines are stroked.
    #[serde(rename = "Stroke", skip_serializing_if = "Option::is_none")]
    pub stroke: Option<bool>,
    /// Whether the glyph interiors are filled.
    #[serde(rename = "Fill", skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
    /// Stroke colour, overriding any draw parameter.
    #[serde(rename = "StrokeColor", skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<ColorRef>,
    /// Fill colour of the glyphs.
    #[serde(rename = "FillColor", skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<ColorRef>,
    /// Runs of glyphs in this object.
    #[serde(rename = "TextCode", default)]
    pub codes: Vec<TextCode>,
}

impl TextObject {
    /// Whether this object holds anything to draw.
    pub fn has_text(&self) -> bool {
        self.codes.iter().any(|c| !c.text.is_empty())
    }
}

/// A raster image placed on the page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageObject {
    /// Attributes shared by every object.
    #[serde(flatten)]
    pub base: ObjectBase,
    /// Id of the image resource inside the document.
    #[serde(rename = "ResourceID", default)]
    pub resource_id: RefId,
    /// Replacement colour for one-bit masks.
    #[serde(rename = "Substitution", skip_serializing_if = "Option::is_none")]
    pub substitution: Option<ColorRef>,
    /// Whether the image is treated as a transparency mask.
    #[serde(rename = "ImageMask", skip_serializing_if = "Option::is_none")]
    pub image_mask: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ObjectBase {
        ObjectBase {
            id: Some("7".to_string()),
            boundary: Rect::new(0.0, 0.0, 10.0, 10.0),
            ctm: None,
            draw_param: None,
            visible: Some(true),
            name: None,
            ..Default::default()
        }
    }

    #[test]
    fn object_ids_are_reachable_across_variants() {
        let path = PathObject {
            base: base(),
            ..Default::default()
        };
        assert_eq!(Object::PathObject(path).id(), Some(&"7".to_string()));

        let img = ImageObject {
            base: ObjectBase {
                id: Some("2".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(Object::ImageObject(img).id(), Some(&"2".to_string()));

        assert_eq!(Object::unknown().id(), None);
    }

    #[test]
    fn only_known_objects_are_renderable() {
        let path = PathObject {
            abbreviated_data: "M 0 0 L 10 10".to_string(),
            ..Default::default()
        };
        assert!(Object::PathObject(path).is_renderable());
        assert!(!Object::unknown().is_renderable());
    }

    #[test]
    fn an_absent_fill_means_filled_but_an_absent_stroke_means_unstroked() {
        // This asymmetry is mandated by the standard and is easy to invert.
        let p = PathObject {
            base: base(),
            abbreviated_data: "M 0 0 L 10 10".to_string(),
            ..Default::default()
        };
        assert_eq!(p.paints(), (true, false));

        let both = PathObject {
            fill: Some(false),
            stroke: Some(true),
            ..p.clone()
        };
        assert_eq!(both.paints(), (false, true));
    }

    #[test]
    fn a_path_without_data_is_invalid() {
        let p = PathObject {
            base: base(),
            abbreviated_data: "".to_string(),
            ..Default::default()
        };
        assert!(p.validate().is_err());

        let ok = PathObject {
            abbreviated_data: "M 0 0 L 1 1".to_string(),
            ..p
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn dash_patterns_know_whether_they_are_solid() {
        assert!(DashPattern::solid().is_solid());
        let dashed = DashPattern {
            phase: 0.0,
            pattern: vec![3.0, 2.0],
        };
        assert!(!dashed.is_solid());
    }

    #[test]
    fn graphics_units_default_to_millimetres() {
        let p = DrawParam {
            id: "1".to_string(),
            ..Default::default()
        };
        assert_eq!(p.relative_units, GraphicsUnit::Millimetre);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn draw_params_validate_their_colours() {
        let p = DrawParam {
            id: "1".to_string(),
            fill_color: Some(ColorRef::from_hex("nonsense")),
            ..Default::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn text_objects_report_whether_they_hold_text() {
        let empty = TextObject::default();
        assert!(!empty.has_text());

        let t = TextObject {
            codes: vec![TextCode {
                text: "hello".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(t.has_text());
    }

    #[test]
    fn line_styles_have_sensible_defaults() {
        assert_eq!(LineCapStyle::default(), LineCapStyle::Butt);
        assert_eq!(LineJoinStyle::default(), LineJoinStyle::Miter);
        assert_eq!(
            FillRuleStyle::default(),
            FillRuleStyle::NonZero,
            "the standard defaults a path to the non-zero rule"
        );
    }

    #[test]
    fn objects_survive_a_serde_round_trip() {
        let obj = Object::PathObject(PathObject {
            base: base(),
            abbreviated_data: "M 0 0 L 10 10 Z".to_string(),
            fill_color: Some(ColorRef::from_hex("#FF0000")),
            stroke_color: None,
            fill: Some(true),
            stroke: Some(true),
            rule: Some(FillRuleStyle::EvenOdd),
        });

        let json = serde_json::to_string(&obj).expect("serialize");
        let back: Object = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, obj);
    }

    #[test]
    fn image_objects_carry_their_resource_reference() {
        let img = ImageObject {
            base: base(),
            resource_id: "img_1".to_string(),
            substitution: None,
            image_mask: Some(false),
        };
        assert_eq!(img.resource_id, "img_1");
    }
}

#[cfg(test)]
mod model_round_trip {
    use super::*;

    #[test]
    fn object_variants_are_distinguished_by_their_fields() {
        // The enum tag must match the element name the writer produced.
        let json =
            r#"{"ID":"1","Boundary":{"x":0.0,"y":0.0,"w":1.0,"h":1.0},"AbbreviatedData":"M 0 0"}"#;
        let obj: Object = serde_json::from_str(json).expect("deserialize");
        match obj {
            Object::PathObject(p) => {
                assert_eq!(p.base.id.as_deref(), Some("1"));
                assert_eq!(p.abbreviated_data, "M 0 0");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn an_unknown_element_deserializes_as_unknown_not_an_error() {
        // A vendor extension must not break loading the page.
        let json = r#"{"VendorWidget":{"ID":"9"}}"#;
        let obj: Object = serde_json::from_str(json).expect("deserialize");
        assert!(!obj.is_renderable());
        assert_eq!(obj.id(), None);
    }

    #[test]
    fn absent_object_level_flags_stay_none() {
        let json = r#"{"ID":"1","Boundary":{"x":0.0,"y":0.0,"w":1.0,"h":1.0},"Font":"1","Size":3.0,"TextCode":[]}"#;
        let obj: Object = serde_json::from_str(json).expect("deserialize");
        match obj {
            Object::TextObject(t) => {
                assert_eq!(t.size.value(), 3.0);
                assert_eq!(t.weight, None);
                assert!(!t.has_text());
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn image_objects_round_trip_their_mask_flag() {
        let json = r#"{"ID":"1","Boundary":{"x":0.0,"y":0.0,"w":1.0,"h":1.0},"ResourceID":"r1","ImageMask":true}"#;
        let obj: Object = serde_json::from_str(json).expect("deserialize");
        match obj {
            Object::ImageObject(i) => {
                assert_eq!(i.resource_id, "r1");
                assert_eq!(i.image_mask, Some(true));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}

/// GB/T 33190 clause 14.1: what a click, hover or load does.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actions {
    /// The actions, in the order they are listed.
    #[serde(rename = "Action", default)]
    pub items: Vec<Action>,
}

/// One action, with the event that triggers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// What triggers the action.
    pub event: ActionEvent,
    /// The region the action is sensitive in, when one is stated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// What the action does.
    pub kind: ActionKind,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            event: ActionEvent::Click,
            region: None,
            kind: ActionKind::Unknown,
        }
    }
}

/// The event an action listens for; `Event` is required by the schema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionEvent {
    /// Fires when the object is loaded.
    Load,
    /// Fires when the pointer hovers over the object.
    Hover,
    /// Fires when the object is clicked.
    #[default]
    Click,
}

impl ActionEvent {
    /// The spelling the standard uses for this event.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "DO",
            Self::Hover => "PO",
            Self::Click => "CLICK",
        }
    }

    /// Read one of the three spellings the standard defines.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "DO" => Some(Self::Load),
            "PO" => Some(Self::Hover),
            "CLICK" => Some(Self::Click),
            _ => None,
        }
    }
}

/// The thing an action does, per the clause.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    /// Jump to a destination inside the document.
    Goto(GotoTarget),
    /// Open a URI.
    Uri(UriAction),
    /// Open an attachment.
    GotoA {
        /// Attachment the action opens.
        attach_id: String,
        /// Whether a new window is wanted; true unless stated otherwise.
        new_window: bool,
    },
    /// Play a sound resource.
    Sound(SoundAction),
    /// Play a movie resource.
    Movie(MovieAction),
    /// An action this build does not model.
    #[default]
    Unknown,
}

/// What a `Sound` action does.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundAction {
    /// Resource id of the sound.
    pub resource_id: String,
    /// Playback volume as a percentage, when one is stated.
    pub volume: Option<i32>,
    /// Whether the sound repeats.
    pub repeat: Option<bool>,
    /// Whether playback blocks the reader.
    pub synchronous: Option<bool>,
}

/// What a `Movie` action does, and what it does to the movie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovieAction {
    /// Resource id of the movie.
    pub resource_id: String,
    /// What to do: `Play`, `Stop`, `Pause` or `Resume`.
    pub operator: MovieOperator,
}

impl Default for MovieAction {
    fn default() -> Self {
        Self {
            resource_id: String::new(),
            operator: MovieOperator::Play,
        }
    }
}

/// The four things a `Movie` action may ask for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovieOperator {
    /// Start playback.
    #[default]
    Play,
    /// Stop playback.
    Stop,
    /// Pause playback.
    Pause,
    /// Resume playback.
    Resume,
}

impl MovieOperator {
    /// The spelling the standard uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
            Self::Resume => "Resume",
        }
    }

    /// Read one of the four spellings, defaulting to `Play` like the schema.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "Stop" => Self::Stop,
            "Pause" => Self::Pause,
            "Resume" => Self::Resume,
            _ => Self::Play,
        }
    }
}

/// Where a `Goto` action jumps to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GotoTarget {
    /// A position on a named page.
    Dest(Dest),
    /// A named bookmark.
    Bookmark {
        /// Bookmark name.
        name: String,
    },
}

/// A destination inside the document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dest {
    /// How the view fits the target: `XYZ`, `Fit`, `FitH`, `FitV` or `FitR`.
    pub kind: String,
    /// Page the destination is on.
    pub page_id: String,
    /// Left edge, when the fit uses one.
    pub left: Option<String>,
    /// Top edge, when the fit uses one.
    pub top: Option<String>,
    /// Right edge, for `FitR`.
    pub right: Option<String>,
    /// Bottom edge, for `FitR`.
    pub bottom: Option<String>,
    /// Zoom factor.
    pub zoom: Option<String>,
}

/// The region an action is limited to, `CT_Region`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Areas making up the region.
    pub areas: Vec<RegionArea>,
}

/// One area of a region: a clip path expressed as draw commands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionArea {
    /// The commands that outline the area, in abbreviated form.
    pub path_data: String,
}

/// What a `URI` action opens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UriAction {
    /// Target URI.
    pub uri: String,
    /// Base URI that `uri` is resolved against.
    pub base: Option<String>,
    /// Where the URI opens.
    pub target: Option<String>,
}
