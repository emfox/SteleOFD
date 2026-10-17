//! Resource-level readers: the `Res` tables the document names.

use super::*;
use crate::content::shared::{
    enum_error, read_child_text, read_draw_param, read_draw_param_entries, read_element_text,
    read_layers_strict,
};

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
        // Clause 11.1 spells it `Charset`; writers in the field emit `CharSet`
        // just as often, and both name the same thing.
        charset: a
            .get("Charset")
            .or_else(|| a.get("CharSet"))
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        serif: read_optional_bool(a, "Serif")?,
        bold: read_optional_bool(a, "Bold")?,
        italic: read_optional_bool(a, "Italic")?,
        fixed_width: read_optional_bool(a, "FixedWidth")?,
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

/// Read a boolean attribute that may be absent.
fn read_optional_bool(a: &steleofd_xml::Attributes, name: &str) -> Result<Option<bool>> {
    match a.get(name) {
        Some(raw) if !raw.is_empty() => Ok(Some(parse_bool("Font", name, raw)?)),
        _ => Ok(None),
    }
}
