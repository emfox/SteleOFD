//! Build model structures from the XML streams of `OFD.xml`, `Document.xml`
//! and the page or template files they reference.

use steleofd_common::{Millimeter, Rect};
use steleofd_error::{FormatError, OfdError, Result, XmlError};
use steleofd_model::color::ColorRef;
use steleofd_model::document::{
    CommonData, DocBody, Document as ModelDocument, DrawParamList, Layer, Location, OfdRoot, Page,
    PageArea, PageBlock, Pages, Template, TemplateList, TemplatePageList, TemplateRef, Version,
};
use steleofd_model::object::{
    DashPattern, DrawParam, FillRuleStyle, GraphicsUnit, ImageObject, LineCapStyle, LineJoinStyle,
    Object, ObjectBase, PathObject, TextCode, TextObject,
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

fn read_doc_body(cursor: &mut Cursor<'_>, _node: &Node) -> Result<DocBody> {
    let mut body = DocBody::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("DocRoot") => {
                body.doc_root = Location::new(cursor.read_text()?)
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
            Event::Start(other) => cursor.skip_element(&other)?,
            Event::End(_) => break,
            Event::Eof => break,
            Event::Text(_) => {}
        }
    }
    Ok(doc)
}

fn read_common_data(cursor: &mut Cursor<'_>) -> Result<CommonData> {
    let mut data = CommonData::default();
    loop {
        match cursor.next_event()? {
            Event::Start(n) if n.is("PageArea") => data.page_area = read_page_area(cursor)?,
            Event::Start(n) if n.is("Templates") => data.templates = Some(read_templates(cursor)?),
            Event::Start(n) if n.is("DrawParams") => {
                data.draw_params = Some(read_draw_params(cursor)?);
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
            Event::Start(n) if n.is("Template") => {
                let a = n.attributes();
                list.items.push(Template {
                    id: a.require("Template", "ID")?.to_string(),
                    base_loc: a.require("Template", "BaseLoc")?.to_string(),
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

fn read_draw_params(cursor: &mut Cursor<'_>) -> Result<DrawParamList> {
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
fn read_color(node: &Node, cursor: &mut Cursor<'_>) -> Result<ColorRef> {
    let a = node.attributes();
    let value = a.get("Value").unwrap_or("").to_string();
    let alpha = a.optional_as::<u8>("FillColor", "Alpha")?.unwrap_or(255);
    cursor.skip_element(node)?;
    Ok(ColorRef::from_hex(value).with_alpha(alpha))
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
                Event::Start(n) if n.is("StrokeColor") => {
                    t.stroke_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("FillColor") => {
                    t.fill_color = Some(read_color(&n, cursor)?)
                }
                Event::Start(n) if n.is("FillColor") => {
                    t.fill_color = Some(read_color(&n, cursor)?)
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

fn read_text_code(cursor: &mut Cursor<'_>, node: &Node) -> Result<TextCode> {
    let a = node.attributes();
    let code = TextCode {
        x: a.optional_as::<f64>("TextCode", "X")?.unwrap_or(0.0),
        y: a.optional_as::<f64>("TextCode", "Y")?.unwrap_or(0.0),
        delta_x: match a.get("DeltaX") {
            Some(raw) => parse_number_list::<f64>("TextCode", "DeltaX", raw)?,
            None => Vec::new(),
        },
        delta_y: match a.get("DeltaY") {
            Some(raw) => parse_number_list::<f64>("TextCode", "DeltaY", raw)?,
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
                Event::Start(other) => cursor.skip_element(&other)?,
                Event::End(_) => break,
                Event::Eof => break,
                Event::Text(_) => {}
            }
        }
    }
    Ok(i)
}
