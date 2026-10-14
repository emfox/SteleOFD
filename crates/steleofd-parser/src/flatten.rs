//! Flattening of the model tree into display commands.
//! Every object is emitted with its CTM already folded into the graphics
//! state, so downstream backends see absolute page-space commands only.

use std::collections::HashMap;
use std::sync::Arc;

use steleofd_display::ops::{Color, LineCap, LineJoin};
use steleofd_display::{
    DisplayListBuilder, DrawOp, GlyphRun, ImageCodec, ImageRef, Paint, PaintRule, StrokeStyle,
};
use steleofd_error::{FormatError, OfdError, Result};
use steleofd_font::FontId;
use steleofd_geometry::{Affine, Path};
use steleofd_model::PageBlock;
use steleofd_model::color::ColorRef;
use steleofd_model::object::{DrawParam, ImageObject, Object, ObjectBase, PathObject, TextObject};
use steleofd_xml::parse_number_list_n;

/// Default stroke width in millimetres when neither the object nor its draw
/// parameter says otherwise; the standard names 0.35 mm as the default.
const DEFAULT_LINE_WIDTH: f64 = 0.35;

/// Flatten a whole page or template block onto the builder.
pub fn flatten_block(
    block: &PageBlock,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    for layer in &block.layers {
        for object in &layer.objects {
            flatten_object(object, params, builder)?;
        }
    }
    Ok(())
}

fn flatten_object(
    object: &Object,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    match object {
        Object::PathObject(p) => flatten_path(p, params, builder),
        Object::TextObject(t) => flatten_text(t, params, builder),
        Object::ImageObject(i) => flatten_image(i, builder),
        Object::Unknown(_) => Ok(()),
    }
}

fn resolve_param<'p>(
    base: &ObjectBase,
    params: &'p HashMap<String, DrawParam>,
) -> Result<Option<&'p DrawParam>> {
    match &base.draw_param {
        Some(id) => params.get(id).map(Some).ok_or_else(|| {
            OfdError::Format(FormatError::DanglingReference {
                kind: "DrawParam",
                id: id.clone(),
            })
        }),
        None => Ok(None),
    }
}

fn ctm_of(base: &ObjectBase) -> Result<Option<Affine>> {
    match &base.ctm {
        None => Ok(None),
        Some(raw) => {
            let v = parse_number_list_n::<6>("Object", "CTM", raw)?;
            Ok(Some(Affine::new(v[0], v[1], v[2], v[3], v[4], v[5])))
        }
    }
}

/// Emit an object under its own CTM, keeping the state stack balanced even
/// when the body fails so later objects start from a clean state.
fn with_ctm(
    base: &ObjectBase,
    builder: &mut DisplayListBuilder,
    body: impl FnOnce(&mut DisplayListBuilder) -> Result<()>,
) -> Result<()> {
    match ctm_of(base)? {
        Some(t) => {
            builder.save()?;
            builder.transform(t);
            let result = body(builder);
            builder.restore()?;
            result
        }
        None => body(builder),
    }
}

fn solid(color: &ColorRef) -> Result<Paint> {
    let (r, g, b) = color.to_rgb()?;
    let alpha = color.clone().alpha();
    Ok(Paint::Solid(Color::rgba(r, g, b, alpha)))
}

fn default_color() -> ColorRef {
    ColorRef::from_hex("#000000")
}

fn stroke_style_of(param: Option<&DrawParam>) -> StrokeStyle {
    let dash = param
        .and_then(|p| p.dash_pattern.as_ref())
        .map(|d| d.pattern.clone())
        .unwrap_or_default();
    let dash_offset = param
        .and_then(|p| p.dash_pattern.as_ref())
        .map(|d| d.phase)
        .unwrap_or(0.0);
    StrokeStyle {
        width: param
            .and_then(|p| p.line_width)
            .unwrap_or(DEFAULT_LINE_WIDTH),
        dash,
        dash_offset,
        cap: match param.and_then(|p| p.line_cap) {
            Some(steleofd_model::object::LineCapStyle::Round) => LineCap::Round,
            Some(steleofd_model::object::LineCapStyle::Square) => LineCap::Square,
            _ => LineCap::Butt,
        },
        join: match param.and_then(|p| p.line_join) {
            Some(steleofd_model::object::LineJoinStyle::Round) => LineJoin::Round,
            Some(steleofd_model::object::LineJoinStyle::Bevel) => LineJoin::Bevel,
            _ => LineJoin::Miter,
        },
        miter_limit: 4.0,
    }
}

fn flatten_path(
    p: &PathObject,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    let param = resolve_param(&p.base, params)?;
    let data = p.abbreviated_data.trim();
    if data.is_empty() {
        return Err(OfdError::Format(FormatError::MissingPathData));
    }
    let path: Path = crate::pathdata::parse_path(data)?;
    let fill_on = p.fill.or(param.and_then(|d| d.fill)).unwrap_or(true);
    let stroke_on = p.stroke.or(param.and_then(|d| d.stroke)).unwrap_or(false);
    let style = stroke_style_of(param);
    let black = default_color();
    let fill = if fill_on {
        Some(solid(
            p.fill_color
                .as_ref()
                .or(param.and_then(|d| d.fill_color.as_ref()))
                .unwrap_or(&black),
        )?)
    } else {
        None
    };
    let stroke = if stroke_on {
        Some(solid(
            p.stroke_color
                .as_ref()
                .or(param.and_then(|d| d.stroke_color.as_ref()))
                .unwrap_or(&black),
        )?)
    } else {
        None
    };
    with_ctm(&p.base, builder, |b| {
        b.push(DrawOp::Path {
            path,
            fill,
            stroke,
            stroke_style: style,
            rule: PaintRule::NonZero,
        });
        Ok(())
    })
}

fn flatten_text(
    t: &TextObject,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    if !t.has_text() {
        return Ok(());
    }
    let param = resolve_param(&t.base, params)?;
    let fill = solid(
        t.fill_color
            .as_ref()
            .or(param.and_then(|d| d.fill_color.as_ref()))
            .unwrap_or(&default_color()),
    )?;
    let size = t.size.value();
    let letter_spacing = t.char_space.unwrap_or(0.0);
    let font = FontId(t.font.parse::<u32>().unwrap_or(0));
    with_ctm(&t.base, builder, |b| {
        for code in &t.codes {
            if code.text.is_empty() {
                continue;
            }
            // Glyph ids are placeholders until font shaping lands; the raw
            // code points keep text extraction working in the meantime.
            let glyphs: Vec<u16> = code.text.chars().map(|c| u32::from(c) as u16).collect();
            let advances = vec![size; glyphs.len()];
            b.push(DrawOp::Text(GlyphRun {
                font,
                size: t.size,
                glyphs,
                advances,
                text: Some(code.text.clone()),
                cluster_map: None,
                transform: Affine::translate(code.x, code.y),
                fill: Some(fill.clone()),
                stroke: None,
                letter_spacing,
                word_spacing: 0.0,
                bbox: t.base.boundary,
            }));
        }
        Ok(())
    })
}

fn flatten_image(i: &ImageObject, builder: &mut DisplayListBuilder) -> Result<()> {
    if i.resource_id.is_empty() {
        return Ok(());
    }
    // Bytes are attached later by the resource resolver; the display list
    // only records which resource each placement refers to.
    let image = ImageRef::new(
        i.resource_id.clone(),
        Arc::new(Vec::new()),
        (1, 1),
        ImageCodec::Png,
    );
    with_ctm(&i.base, builder, |b| {
        b.push(DrawOp::Image {
            image,
            dst: i.base.boundary,
            transform: Affine::IDENTITY,
        });
        Ok(())
    })
}
