//! Build model structures from `OFD.xml`, `Document.xml` and the pages.

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
    PathObject, Region, RegionArea, SoundAction, TextClip, TextCode, TextObject, UriAction,
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

/// Normalise a package-root-relative path: `DocRoot` may be written as
/// `/Doc_0/Document.xml` or `./Doc_0/Document.xml`, and all three name the
/// same entry, so the leading slash and any `./` segments are dropped.
fn trim_leading_slash(raw: &str) -> String {
    let trimmed = raw.trim().replace('\\', "/");
    let mut out = trimmed.as_str();
    loop {
        if let Some(rest) = out.strip_prefix('/') {
            out = rest;
        } else if let Some(rest) = out.strip_prefix("./") {
            out = rest;
        } else {
            break;
        }
    }
    out.to_string()
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

mod document;
mod resource;
mod shared;

pub use document::{read_document, read_page_file, read_template_block};
pub use resource::{
    read_color_spaces, read_composite_graphic_units, read_draw_params, read_fonts,
    read_multimedias, res_base_loc,
};
