//! Build model structures from the XML streams of `OFD.xml`, `Document.xml`
//! and the page or template files they reference.

use steleofd_common::{Millimeter, Rect};
use steleofd_error::{FormatError, OfdError, Result, XmlError};
use steleofd_model::color::{ColorRef, Segment, Shading};
use steleofd_model::document::{
    ColorSpace, ColorSpaceList, CommonData, CompositeGraphicUnit, CompositeGraphicUnitList,
    DocBody, Document as ModelDocument, DrawParamList, Font, FontList, Layer, Location, Multimedia,
    MultimediaKind, MultimediaList, OfdRoot, Page, PageArea, PageBlock, Pages, Permission,
    PrintPermission, Template, TemplateList, TemplatePageList, TemplateRef, VPreferences,
    ValidPeriod, Version, ZoomSetting, unlimited_copies,
};
use steleofd_model::object::{
    Action, ActionEvent, ActionKind, Actions, CGTransform, Clip, ClipArea, ClipList,
    CompositeObject, DashPattern, Dest, DrawParam, FillRuleStyle, GotoTarget, GraphicsUnit,
    ImageObject, LineCapStyle, LineJoinStyle, MovieAction, MovieOperator, Object, ObjectBase,
    PathObject, Region, RegionArea, SoundAction, TextCode, TextObject, UriAction,
};
use steleofd_xml::cursor::Event;
use steleofd_xml::{Cursor, Node, parse_bool, parse_number_list, parse_number_list_n};

/// Parse the container entry point, `OFD.xml`.
pub fn read_ofd_root(xml: &str) -> Result<OfdRoot> {
    let mut cursor = Cursor::new(xml);
    let root = cursor.expect_start("OFD")?;
    if !root.is("OFD") {
        return Err(OfdError::Format(FormatError::NotOfd));
    }
    let mut ofd = OfdRoot::default();
    if let Some(raw) = root.attributes().get("Version") {
        ofd.version = version_of(raw);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(node) if node.is("DocBody") => {
                ofd.doc_bodies.push(read_doc_body(&mut cursor, &node)?)
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(ofd)
}

/// Drop a leading `/` from a package-root-relative path.
fn trim_leading_slash(raw: &str) -> &str {
    raw.trim().strip_prefix('/').unwrap_or_else(|| raw.trim())
}

fn read_doc_body(cursor: &mut Cursor<'_>, _node: &Node) -> Result<DocBody> {
    let mut body = DocBody::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("DocRoot") => {
                // `DocRoot` is package-root relative and producers write it
                // with or without a leading slash; both name the same entry.
                body.doc_root = Location::new(trim_leading_slash(&cursor.read_text()?))
            }
            Event::Start(n) if n.is("Signatures") => {
                let loc = base_loc_or_text(&n, cursor)?;
                body.signatures = Some(loc);
            }
            Event::Start(n) if n.is("Version") => {
                let loc = base_loc_or_text(&n, cursor)?;
                body.version = Some(loc);
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(body)
}

/// Some leaf elements carry the path either as an attribute or as text.
fn base_loc_or_text(node: &Node, cursor: &mut Cursor<'_>) -> Result<Location> {
    if let Some(loc) = node.attributes().get("BaseLoc") {
        cursor.skip_element(node)?;
        return Ok(Location::new(loc));
    }
    Ok(Location::new(cursor.read_text()?))
}

/// Split a dotted version such as `1.0` into major and minor parts.
fn version_of(raw: &str) -> Version {
    match raw.split_once('.') {
        Some((major, sub)) if !sub.is_empty() => Version {
            major: major.to_string(),
            sub: Some(sub.to_string()),
        },
        _ => Version {
            major: raw.to_string(),
            sub: None,
        },
    }
}

/// Parse `Document.xml` into the document model.
pub fn read_document(xml: &str) -> Result<ModelDocument> {
    let mut cursor = Cursor::new(xml);
    let root = cursor.expect_start("Document")?;
    if !root.is("Document") {
        return Err(OfdError::Format(FormatError::NotOfd));
    }
    let mut doc = ModelDocument::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("CommonData") => {
                doc.common_data = read_common_data(&mut cursor)?
            }
            Event::Start(n) if n.is("Pages") => doc.pages = read_pages(&mut cursor)?,
            Event::Start(n) if n.is("VPreferences") => {
                doc.vpreferences = Some(read_vpreferences(&mut cursor)?)
            }
            Event::Start(n) if n.is("Permissions") => {
                doc.permissions = Some(read_permissions(&mut cursor)?)
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(doc)
}

/// Read `VPreferences`, clause 7.11. Every child is optional, so what is
/// absent stays absent rather than being filled from the schema's defaults.
fn read_vpreferences(cursor: &mut Cursor<'_>) -> Result<VPreferences> {
    let mut v = VPreferences::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("PageMode") => {
                let text = read_element_text(cursor, &n)?;
                v.page_mode = Some(text.parse().map_err(|_| enum_error(&n, &text))?);
            }
            Event::Start(n) if n.is("PageLayout") => {
                let text = read_element_text(cursor, &n)?;
                v.page_layout = Some(text.parse().map_err(|_| enum_error(&n, &text))?);
            }
            Event::Start(n) if n.is("TabDisplay") => {
                let text = read_element_text(cursor, &n)?;
                v.tab_display = Some(text.parse().map_err(|_| enum_error(&n, &text))?);
            }
            Event::Start(n) if n.is("HideToolbar") => {
                v.hide_toolbar = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("HideMenubar") => {
                v.hide_menubar = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("HideWindowUI") => {
                v.hide_window_ui = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("ZoomMode") => {
                let text = read_element_text(cursor, &n)?;
                let mode = text.parse().map_err(|_| enum_error(&n, &text))?;
                v.zoom = ZoomSetting::ZoomMode(mode);
            }
            Event::Start(n) if n.is("Zoom") => {
                let text = read_element_text(cursor, &n)?;
                let factor = text.parse().map_err(|_| enum_error(&n, &text))?;
                v.zoom = ZoomSetting::Zoom(factor);
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(v)
}

/// An element's text that must read as a boolean.
fn read_bool_element(cursor: &mut Cursor<'_>, node: &Node) -> Result<bool> {
    let text = read_element_text(cursor, node)?;
    parse_bool(node.local(), node.local(), &text)
}

/// Read `Permissions`, clause 7.12. Each flag is independent, so one stated
/// flag does not imply the others; `Print` and `ValidPeriod` are attributes.
fn read_permissions(cursor: &mut Cursor<'_>) -> Result<Permission> {
    let mut p = Permission::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Edit") => p.edit = Some(read_bool_element(cursor, &n)?),
            Event::Start(n) if n.is("Annot") => p.annotate = Some(read_bool_element(cursor, &n)?),
            Event::Start(n) if n.is("Export") => p.export = Some(read_bool_element(cursor, &n)?),
            Event::Start(n) if n.is("Signature") => {
                p.signature = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("Watermark") => {
                p.watermark = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("PrintScreen") => {
                p.print_screen = Some(read_bool_element(cursor, &n)?)
            }
            Event::Start(n) if n.is("Print") => {
                let a = n.attributes();
                p.print = Some(PrintPermission {
                    printable: a.require_as::<bool>("Print", "Printable")?,
                    copies: a.or_default("Print", "Copies", unlimited_copies())?,
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("ValidPeriod") => {
                let a = n.attributes();
                p.valid_period = Some(ValidPeriod {
                    start_date: a.get("StartDate").map(str::to_string),
                    end_date: a.get("EndDate").map(str::to_string),
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(p)
}

/// A value outside the enumeration its element allows.
fn enum_error(node: &Node, value: &str) -> OfdError {
    OfdError::Xml(XmlError::InvalidAttribute {
        element: node.local().to_string(),
        attr: node.local().to_string(),
        value: value.to_string(),
    })
}

fn read_common_data(cursor: &mut Cursor<'_>) -> Result<CommonData> {
    let mut data = CommonData::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("PageArea") => data.page_area = read_page_area(cursor)?,
            // The standard puts `TemplatePage` straight into `CommonData`
            // with no wrapper; only the wrapped form was accepted before.
            Event::Start(n) if n.is("TemplatePage") => {
                let a = n.attributes();
                data.templates
                    .get_or_insert_with(TemplateList::default)
                    .items
                    .push(Template {
                        id: a.require("TemplatePage", "ID")?.to_string(),
                        base_loc: a.require("TemplatePage", "BaseLoc")?.to_string(),
                    });
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("Templates") => data.templates = Some(read_templates(cursor)?),
            Event::Start(n) if n.is("DrawParams") => {
                data.draw_params = Some(read_draw_param_entries(cursor)?);
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(data)
}

fn read_page_area(cursor: &mut Cursor<'_>) -> Result<PageArea> {
    let mut area = PageArea::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("PhysicalBox") => area.physical_box = rect_of(&n, cursor)?,
            Event::Start(n) if n.is("ApplicationBox") => {
                area.application_box = Some(rect_of(&n, cursor)?)
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(area)
}

/// Boxes follow `ST_Box`: four numbers as element text.
/// The attribute form is tolerated for writers that put the box on the element.
fn rect_of(node: &Node, cursor: &mut Cursor<'_>) -> Result<Rect> {
    let a = node.attributes();
    let attr = |names: &[&str]| -> Result<Option<f64>> {
        for name in names {
            if let Some(raw) = a.get(name) {
                return raw.parse().map(Some).map_err(|_| {
                    OfdError::Xml(XmlError::InvalidAttribute {
                        element: node.local().to_string(),
                        attr: (*name).to_string(),
                        value: raw.to_string(),
                    })
                });
            }
        }
        Ok(None)
    };
    let from_attrs = (
        attr(&["Left", "x"])?,
        attr(&["Top", "y"])?,
        attr(&["Width", "w"])?,
        attr(&["Height", "h"])?,
    );
    if let (Some(x), Some(y), Some(w), Some(h)) = from_attrs {
        cursor.skip_element(node)?;
        return Ok(Rect::new(x, y, w, h));
    }
    let raw = cursor.read_text()?;
    let values = parse_number_list_n::<4>(node.local(), "box", &raw)?;
    Ok(Rect::new(values[0], values[1], values[2], values[3]))
}

fn read_templates(cursor: &mut Cursor<'_>) -> Result<TemplateList> {
    let mut list = TemplateList::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Template") || n.is("TemplatePage") => {
                let a = n.attributes();
                list.items.push(Template {
                    id: a.require(n.local(), "ID")?.to_string(),
                    base_loc: a.require(n.local(), "BaseLoc")?.to_string(),
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(list)
}

fn read_draw_param_entries(cursor: &mut Cursor<'_>) -> Result<DrawParamList> {
    let mut list = DrawParamList::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("DrawParam") => {
                list.items.push(read_draw_param(&mut *cursor, &n)?);
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(list)
}

fn read_draw_param(cursor: &mut Cursor<'_>, node: &Node) -> Result<DrawParam> {
    let a = node.attributes();
    let mut param = DrawParam {
        id: a.require("DrawParam", "ID")?.to_string(),
        relative_units: match a.get("Relative") {
            Some(v) if v == "true" || v == "1" => GraphicsUnit::Relative,
            _ => GraphicsUnit::Millimetre,
        },
        line_width: a.optional_as::<f64>("DrawParam", "LineWidth")?,
        ..Default::default()
    };
    // The standard names these `Cap` and `Join`; `LineCap`/`LineJoin` are the
    // names some writers emit, so both are accepted.
    if let Some(raw) = a.get("Cap").or_else(|| a.get("LineCap")) {
        param.line_cap = Some(match raw {
            "Round" => LineCapStyle::Round,
            "Square" => LineCapStyle::Square,
            _ => LineCapStyle::Butt,
        });
    }
    if let Some(raw) = a.get("Join").or_else(|| a.get("LineJoin")) {
        param.line_join = Some(match raw {
            "Round" => LineJoinStyle::Round,
            "Bevel" => LineJoinStyle::Bevel,
            _ => LineJoinStyle::Miter,
        });
    }
    // `DashPattern` is an ST_Array attribute and `DashOffset` its phase, not a
    // child element.
    let dash = a
        .get("DashPattern")
        .map(|raw| parse_number_list::<f64>("DrawParam", "DashPattern", raw))
        .transpose()?
        .unwrap_or_default();
    if !dash.is_empty() || a.get("DashOffset").is_some() {
        param.dash_pattern = Some(DashPattern {
            phase: a
                .optional_as::<f64>("DrawParam", "DashOffset")?
                .unwrap_or(0.0),
            pattern: dash,
        });
    }
    if let Some(raw) = a.get("Fill") {
        param.fill = Some(parse_bool("DrawParam", "Fill", raw)?);
    }
    if let Some(raw) = a.get("Stroke") {
        param.stroke = Some(parse_bool("DrawParam", "Stroke", raw)?);
    }
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("FillColor") => {
                    param.fill_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("StrokeColor") => {
                    param.stroke_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("DashPattern") => {
                    param.dash_pattern = Some(read_dash(&n, cursor)?)
                }
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(param)
}

/// `<FillColor Value="#FF0000" Alpha="255"/>`; `Alpha` is `0..=255`.
/// Clause 8.3.4 also allows the colour to be a shading, written as a child
/// element rather than a `Value` attribute.
fn read_color(node: &Node, cursor: &mut Cursor<'_>) -> Result<ColorRef> {
    let a = node.attributes();
    let value = a.get("Value").unwrap_or("").to_string();
    let alpha = a.optional_as::<u8>("FillColor", "Alpha")?.unwrap_or(255);
    let element = node.local();
    if node.is_self_closing() {
        return Ok(ColorRef::from_hex(value).with_alpha(alpha));
    }
    let mut shading = None;
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("AxialShd") => shading = Some(read_axial(&n, cursor)?),
            Event::Start(n) if n.is("RadialShd") => shading = Some(read_radial(&n, cursor)?),
            Event::Start(other) if other.is("Pattern") || other.is("GouraudShd") => {
                return Err(unsupported_shading(other.local()));
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    let mut color = ColorRef::from_hex(value).with_alpha(alpha);
    color.shading = shading;
    let _ = element;
    Ok(color)
}

/// A shading the standard defines but this parser does not yet read. Returning
/// an error keeps the gap visible; dropping the element would render a document
/// that silently lost its fill.
fn unsupported_shading(name: &str) -> OfdError {
    OfdError::Xml(XmlError::UnsupportedExtension(name.to_string()))
}

/// `AxialShd`: a linear gradient axis with its colour stops.
fn read_axial(node: &Node, cursor: &mut Cursor<'_>) -> Result<Shading> {
    let a = node.attributes();
    let start = parse_position("AxialShd", "StartPoint", a.get("StartPoint"))?;
    let end = parse_position("AxialShd", "EndPoint", a.get("EndPoint"))?;
    let segments = read_segments(cursor)?;
    Ok(Shading::Axial {
        start,
        end,
        segments,
    })
}

/// `RadialShd`: a cone described by two circles and its colour stops.
fn read_radial(node: &Node, cursor: &mut Cursor<'_>) -> Result<Shading> {
    let a = node.attributes();
    let start = parse_position("RadialShd", "StartPoint", a.get("StartPoint"))?;
    let end = parse_position("RadialShd", "EndPoint", a.get("EndPoint"))?;
    let start_radius = a
        .optional_as::<f64>("RadialShd", "StartRadius")?
        .unwrap_or(0.0);
    let end_radius = a
        .optional_as::<f64>("RadialShd", "EndRadius")?
        .unwrap_or(0.0);
    let segments = read_segments(cursor)?;
    Ok(Shading::Radial {
        start,
        start_radius,
        end,
        end_radius,
        segments,
    })
}

/// `<Segment Position="0.5"><Color Value="#808080"/></Segment>` in sequences of
/// at least two.
fn read_segments(cursor: &mut Cursor<'_>) -> Result<Vec<Segment>> {
    let mut segments = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Segment") => {
                let a = n.attributes();
                let position = a.optional_as::<f64>("Segment", "Position")?.unwrap_or(0.0);
                let mut color = ColorRef::default();
                if n.is_self_closing() {
                    segments.push(Segment { position, color });
                    continue;
                }
                loop {
                    match cursor.next_event()? {
                        Event::Start(c) if c.is("Color") => color = read_color(&c, cursor)?,
                        Event::Start(other) => cursor.skip_element(&other)?,
                        Event::End(_) | Event::Eof => break,
                        Event::Text(_) => {}
                    }
                }
                segments.push(Segment { position, color });
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(segments)
}

/// `StartPoint="0 10"`, a space-separated pair.
fn parse_position(element: &str, attribute: &str, raw: Option<&str>) -> Result<(f64, f64)> {
    let raw = raw.ok_or_else(|| {
        OfdError::Xml(XmlError::MissingAttribute {
            element: element.to_string(),
            attr: attribute.to_string(),
        })
    })?;
    let v = parse_number_list_n::<2>(element, attribute, raw)?;
    Ok((v[0], v[1]))
}

/// `<Clips><Clip><Area><Path .../></Area></Clip></Clips>`. Clause 8.4: an area
/// is one part of a clip, and both a clip and the list of them cover the union
/// of what they name.
fn read_clips(cursor: &mut Cursor<'_>, node: &Node) -> Result<ClipList> {
    let mut list = ClipList {
        trans_flag: node
            .attributes()
            .optional_as::<bool>("Clips", "TransFlag")?,
        ..Default::default()
    };
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Clip") => list.items.push(read_clip(cursor)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(list)
}

/// One `Clip`: every `Area` it names, in order.
fn read_clip(cursor: &mut Cursor<'_>) -> Result<Clip> {
    let mut clip = Clip::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Area") => clip.areas.push(read_area(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(clip)
}

/// One `Area`: its transform, its draw parameter and the path bounding it.
fn read_area(cursor: &mut Cursor<'_>, node: &Node) -> Result<ClipArea> {
    let a = node.attributes();
    let mut area = ClipArea {
        ctm: a.get("CTM").map(str::to_string),
        draw_param: a.get("DrawParam").map(str::to_string),
        ..ClipArea::default()
    };
    if node.is_self_closing() {
        return Ok(area);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(p) if p.is("Path") => {
                area.rule = p.attributes().get("Rule").map(str::to_string);
                area.path_data = read_clip_path(cursor, &p)?;
            }
            Event::Start(t) if t.is("Text") => {
                return Err(unsupported_shading("text clip area"));
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(area)
}

/// The abbreviated data inside an `Area`'s `Path`.
fn read_clip_path(cursor: &mut Cursor<'_>, node: &Node) -> Result<Option<String>> {
    if node.is_self_closing() {
        return Ok(None);
    }
    let mut data = None;
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("AbbreviatedData") => data = Some(cursor.read_text()?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(data)
}

fn read_dash(node: &Node, cursor: &mut Cursor<'_>) -> Result<DashPattern> {
    let a = node.attributes();
    let pattern = match a.get("Value") {
        Some(raw) => parse_number_list::<f64>("DashPattern", "Value", raw)?,
        None => Vec::new(),
    };
    let phase = a
        .optional_as::<f64>("DashPattern", "Offset")?
        .unwrap_or(0.0);
    cursor.skip_element(node)?;
    Ok(DashPattern { phase, pattern })
}

fn read_pages(cursor: &mut Cursor<'_>) -> Result<Pages> {
    let mut pages = Pages::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Page") => pages.pages.push(read_page(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(pages)
}

fn read_page(cursor: &mut Cursor<'_>, node: &Node) -> Result<Page> {
    let a = node.attributes();
    let mut page = Page {
        id: a.get("ID").unwrap_or("").to_string(),
        base_loc: a.get("BaseLoc").map(str::to_string),
        ..Default::default()
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("Area") => page.area = Some(read_page_area(cursor)?),
                Event::Start(n) if n.is("Content") => {
                    page.content = Some(PageBlock {
                        layers: read_layers_strict(cursor)?,
                    });
                }
                Event::Start(n) if n.is("Template") => {
                    let refs = page.templates.get_or_insert_with(TemplatePageList::default);
                    refs.items.push(TemplateRef {
                        template_id: n
                            .attributes()
                            .require("Template", "TemplateID")?
                            .to_string(),
                        z_order: None,
                    });
                    cursor.skip_element(&n)?;
                }
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(page)
}

/// Parse a standalone page file referenced through `BaseLoc`.
pub fn read_page_file(xml: &str) -> Result<Page> {
    let mut cursor = Cursor::new(xml);
    let root = cursor.expect_start("Page")?;
    let page = read_page(&mut cursor, &root)?;
    Ok(page)
}

/// Parse a template content file into its layers.
/// Template roots vary between writers, so layers are collected wherever they
/// appear under the root instead of assuming one fixed shape.
pub fn read_template_block(xml: &str) -> Result<PageBlock> {
    let mut cursor = Cursor::new(xml);
    let mut layers: Vec<Layer> = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Content") => layers.extend(read_layers_strict(&mut cursor)?),
            Event::Start(_) => {}
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(PageBlock { layers })
}

/// Read the direct children of an element, collecting its `Layer` children.
/// Other children are skipped; the first end tag closes the element itself.
fn read_layers_strict(cursor: &mut Cursor<'_>) -> Result<Vec<Layer>> {
    let mut layers = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Layer") => layers.push(read_layer(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => return Ok(layers),
            Event::Eof => return Ok(layers),
            Event::Text(_) => {}
        }
    }
}

fn read_layer(cursor: &mut Cursor<'_>, node: &Node) -> Result<Layer> {
    let a = node.attributes();
    let mut layer = Layer {
        id: a.get("ID").map(str::to_string),
        objects: Vec::new(),
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("PathObject") => {
                    layer
                        .objects
                        .push(Object::PathObject(read_path_object(cursor, &n)?));
                }
                Event::Start(n) if n.is("TextObject") => {
                    layer
                        .objects
                        .push(Object::TextObject(read_text_object(cursor, &n)?));
                }
                Event::Start(n) if n.is("ImageObject") => {
                    layer
                        .objects
                        .push(Object::ImageObject(read_image_object(cursor, &n)?));
                }
                Event::Start(n) if n.is("CompositeObject") => {
                    layer
                        .objects
                        .push(Object::CompositeObject(read_composite_object(cursor, &n)?));
                }
                // Vendor extensions are recorded as unknown and skipped at
                // render time instead of failing the whole page.
                Event::Start(other) => {
                    cursor.skip_element(&other)?;
                    layer.objects.push(Object::unknown());
                }
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(layer)
}

fn parse_base(node: &Node) -> Result<ObjectBase> {
    let a = node.attributes();
    let element = node.local();
    let boundary = match a.get("Boundary") {
        Some(raw) => {
            let v = parse_number_list_n::<4>(element, "Boundary", raw)?;
            Rect::new(v[0], v[1], v[2], v[3])
        }
        None => Rect::default(),
    };
    Ok(ObjectBase {
        id: a.get("ID").map(str::to_string),
        boundary,
        ctm: a.get("CTM").map(str::to_string),
        draw_param: a.get("DrawParam").map(str::to_string),
        visible: match a.get("Visible") {
            Some(raw) => Some(parse_bool(element, "Visible", raw)?),
            None => None,
        },
        name: a.get("Name").map(str::to_string),
        // Clause 8.2 fixes these on the graphic unit itself; a draw parameter
        // only supplies them when the object does not.
        line_width: a.optional_as::<f64>(element, "LineWidth")?,
        cap: a.get("Cap").map(|raw| match raw {
            "Round" => LineCapStyle::Round,
            "Square" => LineCapStyle::Square,
            _ => LineCapStyle::Butt,
        }),
        join: a.get("Join").map(|raw| match raw {
            "Round" => LineJoinStyle::Round,
            "Bevel" => LineJoinStyle::Bevel,
            _ => LineJoinStyle::Miter,
        }),
        miter_limit: a.optional_as::<f64>(element, "MiterLimit")?,
        clips: None,
        actions: None,
    })
}

/// GB/T 33190 clause 13: a composite object places a vector resource on the
/// page, so it only carries a resource reference on top of the object base.
fn read_composite_object(cursor: &mut Cursor<'_>, node: &Node) -> Result<CompositeObject> {
    let mut c = CompositeObject {
        base: parse_base(node)?,
        // `CT_Composite` names its resource on the element itself.
        resource_id: node
            .attributes()
            .get("ResourceID")
            .unwrap_or_default()
            .to_string(),
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("Actions") => c.base.actions = Some(read_actions(cursor)?),
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) | Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(c)
}

/// The `Actions` element a graphic unit may carry, clause 14.1.
fn read_actions(cursor: &mut Cursor<'_>) -> Result<Actions> {
    let mut actions = Actions::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Action") => actions.items.push(read_action(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(actions)
}

/// One `Action`: the event that fires it, an optional region, and the thing it
/// does. `Event` is required by the schema, so an unknown spelling is an error
/// rather than a silent default.
fn read_action(cursor: &mut Cursor<'_>, node: &Node) -> Result<Action> {
    let raw = node.attributes().require("Action", "Event")?;
    let event = ActionEvent::parse(raw).ok_or_else(|| {
        OfdError::Xml(XmlError::InvalidAttribute {
            element: "Action".to_string(),
            attr: "Event".to_string(),
            value: raw.to_string(),
        })
    })?;
    let mut action = Action {
        event,
        region: None,
        kind: ActionKind::Unknown,
    };
    if node.is_self_closing() {
        return Ok(action);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Region") => action.region = Some(read_region(cursor)?),
            Event::Start(n) if n.is("Goto") => action.kind = read_goto(cursor)?,
            Event::Start(n) if n.is("URI") => {
                let a = n.attributes();
                action.kind = ActionKind::Uri(UriAction {
                    uri: a.get("URI").unwrap_or_default().to_string(),
                    base: a.get("Base").map(str::to_string),
                    target: a.get("Target").map(str::to_string),
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("GotoA") => {
                let a = n.attributes();
                action.kind = ActionKind::GotoA {
                    attach_id: a.get("AttachID").unwrap_or_default().to_string(),
                    new_window: match a.get("NewWindow") {
                        Some(raw) => parse_bool("GotoA", "NewWindow", raw)?,
                        None => true,
                    },
                };
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("Sound") => {
                let a = n.attributes();
                action.kind = ActionKind::Sound(SoundAction {
                    resource_id: a.get("ResourceID").unwrap_or_default().to_string(),
                    volume: a.optional_as::<i32>("Sound", "Volume")?,
                    repeat: match a.get("Repeat") {
                        Some(raw) => Some(parse_bool("Sound", "Repeat", raw)?),
                        None => None,
                    },
                    synchronous: match a.get("Synchronous") {
                        Some(raw) => Some(parse_bool("Sound", "Synchronous", raw)?),
                        None => None,
                    },
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("Movie") => {
                let a = n.attributes();
                action.kind = ActionKind::Movie(MovieAction {
                    resource_id: a.get("ResourceID").unwrap_or_default().to_string(),
                    operator: MovieOperator::parse(a.get("Operator").unwrap_or("Play")),
                });
                cursor.skip_element(&n)?;
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(action)
}

/// The region an action is sensitive in, `CT_Region`.
fn read_region(cursor: &mut Cursor<'_>) -> Result<Region> {
    let mut region = Region::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Area") => region.areas.push(read_region_area(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(region)
}

/// One `Area` of a region, flattened into a path so a hit test can use it.
fn read_region_area(cursor: &mut Cursor<'_>, node: &Node) -> Result<RegionArea> {
    let mut area = RegionArea::default();
    if node.is_self_closing() {
        return Ok(area);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Move") => {
                area.path_data.push_str(&command("M", &n, &["Point1"])?);
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("Line") => {
                area.path_data.push_str(&command("L", &n, &["Point1"])?);
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("QuadraticBezier") => {
                area.path_data
                    .push_str(&command("Q", &n, &["Point1", "Point2"])?);
                cursor.skip_element(&n)?;
            }
            Event::Start(n) if n.is("CubicBezier") => {
                area.path_data
                    .push_str(&command("C", &n, &["Point1", "Point2", "Point3"])?);
                cursor.skip_element(&n)?;
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(area)
}

/// Turn one region command into its abbreviated form, `M x y`, `L x y`, and so on.
fn command(letter: &str, node: &Node, points: &[&str]) -> Result<String> {
    let mut out = String::from(letter);
    for name in points {
        let raw = node.attributes().get(name).unwrap_or("0 0");
        let v = steleofd_xml::parse_number_list::<f64>(node.local(), name, raw)?;
        for n in v {
            out.push(' ');
            out.push_str(&n.to_string());
        }
    }
    out.push(' ');
    Ok(out)
}

/// What a `Goto` jumps to: a destination on a page, or a bookmark.
fn read_goto(cursor: &mut Cursor<'_>) -> Result<ActionKind> {
    let mut target = None;
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Dest") => {
                let a = n.attributes();
                target = Some(GotoTarget::Dest(Dest {
                    kind: a.get("Type").unwrap_or_default().to_string(),
                    page_id: a.get("PageID").unwrap_or_default().to_string(),
                    left: a.get("Left").map(str::to_string),
                    top: a.get("Top").map(str::to_string),
                    right: a.get("Right").map(str::to_string),
                    bottom: a.get("Bottom").map(str::to_string),
                    zoom: a.get("Zoom").map(str::to_string),
                }));
            }
            Event::Start(n) if n.is("Bookmark") => {
                target = Some(GotoTarget::Bookmark {
                    name: n.attributes().get("Name").unwrap_or_default().to_string(),
                });
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(match target {
        Some(t) => ActionKind::Goto(t),
        None => ActionKind::Unknown,
    })
}

fn read_path_object(cursor: &mut Cursor<'_>, node: &Node) -> Result<PathObject> {
    let a = node.attributes();
    let mut p = PathObject {
        base: parse_base(node)?,
        fill: match a.get("Fill") {
            Some(raw) => Some(parse_bool("PathObject", "Fill", raw)?),
            None => None,
        },
        stroke: match a.get("Stroke") {
            Some(raw) => Some(parse_bool("PathObject", "Stroke", raw)?),
            None => None,
        },
        rule: match a.get("Rule") {
            Some("Even-Odd") => Some(FillRuleStyle::EvenOdd),
            Some(_) => Some(FillRuleStyle::NonZero),
            None => None,
        },
        ..Default::default()
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("AbbreviatedData") => {
                    p.abbreviated_data = cursor.read_text()?
                }
                Event::Start(n) if n.is("FillColor") => {
                    p.fill_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("StrokeColor") => {
                    p.stroke_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("Clips") => p.base.clips = Some(read_clips(cursor, &n)?),
                Event::Start(n) if n.is("Actions") => p.base.actions = Some(read_actions(cursor)?),
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(p)
}

fn read_text_object(cursor: &mut Cursor<'_>, node: &Node) -> Result<TextObject> {
    let a = node.attributes();
    let mut t = TextObject {
        base: parse_base(node)?,
        font: a.get("Font").unwrap_or("").to_string(),
        size: Millimeter::new(a.optional_as::<f64>("TextObject", "Size")?.unwrap_or(3.0)),
        char_space: a.optional_as::<f64>("TextObject", "CharSpace")?,
        h_scale: a.optional_as::<f64>("TextObject", "HScale")?,
        stroke: match a.get("Stroke") {
            Some(raw) => Some(parse_bool("TextObject", "Stroke", raw)?),
            None => None,
        },
        fill: match a.get("Fill") {
            Some(raw) => Some(parse_bool("TextObject", "Fill", raw)?),
            None => None,
        },
        ..Default::default()
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("TextCode") => t.codes.push(read_text_code(cursor, &n)?),
                Event::Start(n) if n.is("CGTransform") => {
                    t.cg_transforms.push(read_cg_transform(cursor, &n)?)
                }
                Event::Start(n) if n.is("StrokeColor") => {
                    t.stroke_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("FillColor") => {
                    t.fill_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("Clips") => t.base.clips = Some(read_clips(cursor, &n)?),
                Event::Start(n) if n.is("Actions") => t.base.actions = Some(read_actions(cursor)?),
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(t)
}

/// GB/T 33190 clause 11.4: a `CGTransform` covers `CodeCount` characters from
/// `CodePosition` and names the glyphs that replace them; both default to one.
fn read_cg_transform(cursor: &mut Cursor<'_>, node: &Node) -> Result<CGTransform> {
    let a = node.attributes();
    let mut t = CGTransform {
        code_position: a
            .optional_as::<usize>("CGTransform", "CodePosition")?
            .unwrap_or(0),
        code_count: a
            .optional_as::<usize>("CGTransform", "CodeCount")?
            .unwrap_or(1),
        glyph_count: a
            .optional_as::<usize>("CGTransform", "GlyphCount")?
            .unwrap_or(1),
        glyphs: Vec::new(),
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("Glyphs") => {
                    let raw = cursor.read_text()?;
                    t.glyphs = parse_number_list("CGTransform", "Glyphs", &raw)?;
                }
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(t)
}

/// Parse a `DeltaX`/`DeltaY` per-glyph adjustment list, where `g` stands for
/// "the adjustment the glyph before had"; a leading `g` means none at all.
fn parse_delta_list(element: &str, attr: &str, raw: &str) -> Result<Vec<f64>> {
    let mut out: Vec<f64> = Vec::new();
    for token in raw.split_whitespace() {
        if token.eq_ignore_ascii_case("g") {
            out.push(out.last().copied().unwrap_or(0.0));
            continue;
        }
        out.push(token.parse::<f64>().map_err(|_| {
            OfdError::Xml(steleofd_error::XmlError::InvalidAttribute {
                element: element.to_string(),
                attr: attr.to_string(),
                value: raw.to_string(),
            })
        })?);
    }
    Ok(out)
}

fn read_text_code(cursor: &mut Cursor<'_>, node: &Node) -> Result<TextCode> {
    let a = node.attributes();
    let code = TextCode {
        x: a.optional_as::<f64>("TextCode", "X")?.unwrap_or(0.0),
        y: a.optional_as::<f64>("TextCode", "Y")?.unwrap_or(0.0),
        delta_x: match a.get("DeltaX") {
            Some(raw) => parse_delta_list("TextCode", "DeltaX", raw)?,
            None => Vec::new(),
        },
        delta_y: match a.get("DeltaY") {
            Some(raw) => parse_delta_list("TextCode", "DeltaY", raw)?,
            None => Vec::new(),
        },
        text: if node.is_self_closing() {
            String::new()
        } else {
            cursor.read_text()?
        },
    };
    Ok(code)
}

fn read_image_object(cursor: &mut Cursor<'_>, node: &Node) -> Result<ImageObject> {
    let a = node.attributes();
    let mut i = ImageObject {
        base: parse_base(node)?,
        resource_id: a.get("ResourceID").unwrap_or("").to_string(),
        ..Default::default()
    };
    if !node.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("Substitution") => {
                    i.substitution = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("Clips") => i.base.clips = Some(read_clips(cursor, &n)?),
                Event::Start(n) if n.is("Actions") => i.base.actions = Some(read_actions(cursor)?),
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(i)
}

/// The `BaseLoc` a `Res` table declares, defaulting to `Res` itself.
/// A scanner, not a full parse: a resource table may be written in Chinese,
/// so the head is taken by character boundary rather than by byte count.
pub fn res_base_loc(xml: &str) -> &str {
    let head = xml.get(..xml.len().min(4096)).unwrap_or(xml);
    let Some(at) = head.find("BaseLoc=\"") else {
        return "Res";
    };
    let rest = &head[at + 9..];
    let end = rest.find('"').unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() { "Res" } else { value }
}

/// GB/T 33190 clause 7.8: read a `Res` file's font table. The declaration is
/// what ties a document font id to a file inside the package, which is the
/// only place that pairing exists.
pub fn read_fonts(xml: &str) -> Result<FontList> {
    let mut cursor = Cursor::new(xml);
    let mut list = FontList::default();
    let root = cursor.expect_start("Res")?;
    if !root.is_self_closing() {
        // The entries sit under a `Fonts` container; other resource tables
        // are skipped whole.
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("Fonts") => read_font_entries(&mut cursor, &n, &mut list)?,
                Event::Start(n) if n.is("Font") => push_font(&mut cursor, &n, &mut list)?,
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) | Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(list)
}

fn read_font_entries(cursor: &mut Cursor<'_>, node: &Node, list: &mut FontList) -> Result<()> {
    if node.is_self_closing() {
        return Ok(());
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Font") => push_font(cursor, &n, list)?,
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(())
}

fn push_font(cursor: &mut Cursor<'_>, node: &Node, list: &mut FontList) -> Result<()> {
    let a = node.attributes();
    let Some(id) = a.get("ID").filter(|id| !id.is_empty()) else {
        // A font nothing can name is not a resource; skipping it whole keeps
        // the rest of the table loadable.
        cursor.skip_element(node)?;
        return Ok(());
    };
    let font_file = if node.is_self_closing() {
        String::new()
    } else {
        read_child_text(cursor, "FontFile")?
    };
    list.items.push(Font {
        id: id.to_string(),
        font_name: a.get("FontName").unwrap_or("").to_string(),
        family_name: a.get("FamilyName").map(str::to_string),
        charset: a.get("CharSet").map(str::to_string),
        bold: match a.get("Bold") {
            Some(raw) => Some(parse_bool("Font", "Bold", raw)?),
            None => None,
        },
        font_file,
    });
    Ok(())
}

/// GB/T 33190 clause 7.13: read a `Res` file's colour-space table. Each space
/// is `CT_ColorSpace`: a `Type`, an optional `Profile` and an optional
/// `Palette`, plus the resource `ID` the `Res` wrapper adds.
pub fn read_color_spaces(xml: &str) -> Result<ColorSpaceList> {
    let mut cursor = Cursor::new(xml);
    let mut list = ColorSpaceList::default();
    let root = cursor.expect_start("Res")?;
    if root.is_self_closing() {
        return Ok(list);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("ColorSpaces") => {
                read_color_space_entries(&mut cursor, &n, &mut list)?
            }
            Event::Start(n) if n.is("ColorSpace") => {
                list.items.push(read_color_space(&mut cursor, &n)?)
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(list)
}

fn read_color_space_entries(
    cursor: &mut Cursor<'_>,
    node: &Node,
    list: &mut ColorSpaceList,
) -> Result<()> {
    if node.is_self_closing() {
        return Ok(());
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("ColorSpace") => list.items.push(read_color_space(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(())
}

fn read_color_space(cursor: &mut Cursor<'_>, node: &Node) -> Result<ColorSpace> {
    let a = node.attributes();
    let type_name = a.require("ColorSpace", "Type")?.to_string();
    let mut space = ColorSpace {
        id: a.require("ColorSpace", "ID")?.to_string(),
        kind: type_name
            .parse()
            .map_err(|_| enum_error(node, &type_name))?,
        bits_per_component: a.or_default("ColorSpace", "BitsPerComponent", 8u8)?,
        profile: a.get("Profile").map(str::to_string),
        palette: Vec::new(),
    };
    if node.is_self_closing() {
        return Ok(space);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Palette") => space.palette = read_palette(cursor)?,
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(space)
}

/// A palette is a list of `CV` entries, each a space-separated channel list.
fn read_palette(cursor: &mut Cursor<'_>) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("CV") => entries.push(read_element_text(cursor, &n)?),
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(entries)
}

/// Read `MultiMedia` tables from a `Res` file, which is where video lives.
/// GB/T 33190 clause 12: read a `Res` file's multimedia table. The table is
/// where video lives, since the schema gives no video graphic unit of its own.
pub fn read_multimedias(xml: &str) -> Result<MultimediaList> {
    let mut cursor = Cursor::new(xml);
    let mut list = MultimediaList::default();
    let root = cursor.expect_start("Res")?;
    if !root.is_self_closing() {
        // The entries sit under a `MultiMedias` container; other resource
        // tables are skipped whole.
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("MultiMedias") => {
                    read_multimedia_entries(&mut cursor, &n, &mut list)?
                }
                Event::Start(n) if n.is("MultiMedia") => {
                    push_multimedia(&mut cursor, &n, &mut list)?
                }
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) | Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(list)
}

/// GB/T 33190 clause 7.9: read a `Res` file's drawing parameter table. The
/// schema keeps the table in the resource file, not in `CommonData`, which is
/// where every producer in the corpus puts it.
pub fn read_draw_params(xml: &str) -> Result<DrawParamList> {
    let mut cursor = Cursor::new(xml);
    let mut list = DrawParamList::default();
    let root = cursor.expect_start("Res")?;
    if !root.is_self_closing() {
        // The entries sit under a `DrawParams` container; other resource
        // tables are skipped whole.
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("DrawParams") => {
                    list = read_draw_param_entries(&mut cursor)?;
                }
                Event::Start(n) if n.is("DrawParam") => {
                    list.items.push(read_draw_param(&mut cursor, &n)?);
                }
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) | Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(list)
}

fn read_multimedia_entries(
    cursor: &mut Cursor<'_>,
    node: &Node,
    list: &mut MultimediaList,
) -> Result<()> {
    if node.is_self_closing() {
        return Ok(());
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("MultiMedia") => push_multimedia(cursor, &n, list)?,
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(())
}

fn push_multimedia(cursor: &mut Cursor<'_>, node: &Node, list: &mut MultimediaList) -> Result<()> {
    let a = node.attributes();
    let media_file = if node.is_self_closing() {
        String::new()
    } else {
        read_child_text(cursor, "MediaFile")?
    };
    list.items.push(Multimedia {
        id: a.require("MultiMedia", "ID")?.to_string(),
        kind: MultimediaKind::parse(a.get("Type").unwrap_or("Image")),
        format: a.get("Format").map(str::to_string),
        media_file,
    });
    Ok(())
}

/// GB/T 33190 clause 13: read a `Res` file's vector graphic table. A page's
/// `CompositeObject` names one of these by `ResourceID`.
pub fn read_composite_graphic_units(xml: &str) -> Result<CompositeGraphicUnitList> {
    let mut cursor = Cursor::new(xml);
    let mut list = CompositeGraphicUnitList::default();
    let root = cursor.expect_start("Res")?;
    if !root.is_self_closing() {
        loop {
            match cursor.next_event()? {
                Event::Start(n) if n.is("CompositeGraphicUnits") => {
                    read_composite_graphic_entries(&mut cursor, &n, &mut list)?
                }
                Event::Start(n) if n.is("CompositeGraphicUnit") => list
                    .items
                    .push(read_composite_graphic_unit(&mut cursor, &n)?),
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) | Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(list)
}

fn read_composite_graphic_entries(
    cursor: &mut Cursor<'_>,
    node: &Node,
    list: &mut CompositeGraphicUnitList,
) -> Result<()> {
    if node.is_self_closing() {
        return Ok(());
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("CompositeGraphicUnit") => {
                list.items.push(read_composite_graphic_unit(cursor, &n)?)
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(())
}

fn read_composite_graphic_unit(
    cursor: &mut Cursor<'_>,
    node: &Node,
) -> Result<CompositeGraphicUnit> {
    let a = node.attributes();
    let mut unit = CompositeGraphicUnit {
        id: a.require("CompositeGraphicUnit", "ID")?.to_string(),
        width: a
            .optional_as::<f64>("CompositeGraphicUnit", "Width")?
            .unwrap_or(0.0),
        height: a
            .optional_as::<f64>("CompositeGraphicUnit", "Height")?
            .unwrap_or(0.0),
        ..Default::default()
    };
    if node.is_self_closing() {
        return Ok(unit);
    }
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("Thumbnail") => {
                unit.thumbnail = Some(read_element_text(cursor, &n)?)
            }
            Event::Start(n) if n.is("Substitution") => {
                unit.substitution = Some(read_element_text(cursor, &n)?)
            }
            Event::Start(n) if n.is("Content") => {
                unit.content = Some(PageBlock {
                    layers: read_layers_strict(cursor)?,
                })
            }
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(unit)
}

/// Read the text of the first `name` child of the element the cursor is inside.
fn read_child_text(cursor: &mut Cursor<'_>, name: &str) -> Result<String> {
    let mut found = String::new();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is(name) => found = read_element_text(cursor, &n)?,
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) | Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(found)
}

/// Read the text of one element, tolerating a self-closing form.
fn read_element_text(cursor: &mut Cursor<'_>, node: &Node) -> Result<String> {
    if node.is_self_closing() {
        Ok(String::new())
    } else {
        let text = cursor.read_text()?;
        Ok(text.trim().to_string())
    }
}
