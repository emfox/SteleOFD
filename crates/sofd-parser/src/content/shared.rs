//! Parsing helpers more than one reader needs.

use super::*;
use crate::content::document::{read_color, read_dash, read_layer};

/// A value outside the enumeration its element allows.
pub(crate) fn enum_error(node: &Node, value: &str) -> OfdError {
    OfdError::Xml(XmlError::InvalidAttribute {
        element: node.local().to_string(),
        attr: node.local().to_string(),
        value: value.to_string(),
    })
}

pub(crate) fn read_draw_param_entries(cursor: &mut Cursor<'_>) -> Result<DrawParamList> {
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

pub(crate) fn read_draw_param(cursor: &mut Cursor<'_>, node: &Node) -> Result<DrawParam> {
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
/// Read the direct children of an element, collecting its `Layer` children.
/// Other children are skipped; the first end tag closes the element itself.
pub(crate) fn read_layers_strict(cursor: &mut Cursor<'_>) -> Result<Vec<Layer>> {
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

pub(crate) fn read_child_text(cursor: &mut Cursor<'_>, name: &str) -> Result<String> {
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
pub(crate) fn read_element_text(cursor: &mut Cursor<'_>, node: &Node) -> Result<String> {
    if node.is_self_closing() {
        Ok(String::new())
    } else {
        let text = cursor.read_text()?;
        Ok(text.trim().to_string())
    }
}
