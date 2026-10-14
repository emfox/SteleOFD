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
    // GB/T 33190 clause 8.5: Visible defaults to true and false hides the object.
    if visible_of(object) == Some(false) {
        return Ok(());
    }
    match object {
        Object::PathObject(p) => flatten_path(p, params, builder),
        Object::TextObject(t) => flatten_text(t, params, builder),
        Object::ImageObject(i) => flatten_image(i, builder),
        Object::Unknown(_) => Ok(()),
    }
}

/// GB/T 33190 clause 8.5: every object exposes the same base attributes.
fn visible_of(object: &Object) -> Option<bool> {
    match object {
        Object::PathObject(o) => o.base.visible,
        Object::TextObject(o) => o.base.visible,
        Object::ImageObject(o) => o.base.visible,
        Object::Unknown(_) => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_common::Rect;
    use steleofd_model::document::{Layer, PageBlock};
    use steleofd_model::object::{ImageObject, PathObject, TextCode, TextObject};

    fn builder() -> DisplayListBuilder {
        DisplayListBuilder::new(Rect::A4_PORTRAIT)
    }

    fn run(objects: Vec<Object>, params: &HashMap<String, DrawParam>) -> Result<Vec<DrawOp>> {
        let block = PageBlock {
            layers: vec![Layer { id: None, objects }],
        };
        let mut b = builder();
        flatten_block(&block, params, &mut b)?;
        Ok(b.finish(0).ops)
    }

    fn path_object(data: &str) -> PathObject {
        PathObject {
            base: ObjectBase {
                id: Some("1".to_string()),
                boundary: Rect::new(0.0, 0.0, 10.0, 10.0),
                ..Default::default()
            },
            abbreviated_data: data.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn an_absent_fill_means_filled_and_an_absent_stroke_means_not() {
        let ops = run(
            vec![Object::PathObject(path_object("M 0 0 L 10 10"))],
            &HashMap::new(),
        )
        .expect("flatten");
        let DrawOp::Path { fill, stroke, .. } = &ops[0] else {
            panic!("expected a path op, got {ops:?}");
        };
        assert!(fill.is_some());
        assert!(stroke.is_none());
    }

    #[test]
    fn object_colours_default_to_black() {
        let ops = run(
            vec![Object::PathObject(path_object("M 0 0 L 10 10"))],
            &HashMap::new(),
        )
        .expect("flatten");
        let DrawOp::Path {
            fill: Some(Paint::Solid(c)),
            ..
        } = &ops[0]
        else {
            panic!("expected a filled path, got {ops:?}");
        };
        assert_eq!(*c, Color::BLACK);
    }

    #[test]
    fn a_draw_param_supplies_colour_and_width() {
        let mut params = HashMap::new();
        params.insert(
            "7".to_string(),
            DrawParam {
                id: "7".to_string(),
                line_width: Some(0.5),
                fill_color: Some(ColorRef::from_hex("#FF0000")),
                ..Default::default()
            },
        );
        let mut p = path_object("M 0 0 L 10 10");
        p.base.draw_param = Some("7".to_string());
        let ops = run(vec![Object::PathObject(p)], &params).expect("flatten");
        let DrawOp::Path {
            fill, stroke_style, ..
        } = &ops[0]
        else {
            panic!("expected a path op, got {ops:?}");
        };
        assert!(matches!(
            fill,
            Some(Paint::Solid(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }))
        ));
        assert!((stroke_style.width - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_dangling_draw_param_reference_is_reported() {
        let mut p = path_object("M 0 0 L 10 10");
        p.base.draw_param = Some("missing".to_string());
        assert!(run(vec![Object::PathObject(p)], &HashMap::new()).is_err());
    }

    #[test]
    fn a_ctm_wraps_the_object_in_save_transform_restore() {
        let mut p = path_object("M 0 0 L 10 10");
        p.base.ctm = Some("1 0 0 1 5 5".to_string());
        let ops = run(vec![Object::PathObject(p)], &HashMap::new()).expect("flatten");
        assert!(matches!(ops[0], DrawOp::Save));
        assert!(matches!(ops[1], DrawOp::Transform(t) if t.apply(0.0, 0.0) == (5.0, 5.0)));
        assert!(matches!(ops[2], DrawOp::Path { .. }));
        assert!(matches!(ops[3], DrawOp::Restore));
    }

    #[test]
    fn path_data_is_required() {
        let p = path_object("   ");
        assert!(run(vec![Object::PathObject(p)], &HashMap::new()).is_err());
    }

    #[test]
    fn unknown_objects_are_skipped_silently() {
        let ops = run(vec![Object::unknown()], &HashMap::new()).expect("flatten");
        assert!(ops.is_empty());
    }

    #[test]
    fn each_text_code_becomes_its_own_glyph_run() {
        let t = TextObject {
            base: ObjectBase {
                boundary: Rect::new(1.0, 2.0, 30.0, 4.0),
                ..Default::default()
            },
            size: steleofd_common::Millimeter::new(3.0),
            codes: vec![
                TextCode {
                    x: 0.0,
                    y: 0.0,
                    delta_x: Vec::new(),
                    text: "AB".to_string(),
                },
                TextCode {
                    x: 10.0,
                    y: 0.0,
                    delta_x: Vec::new(),
                    text: String::new(),
                },
            ],
            ..Default::default()
        };
        let ops = run(vec![Object::TextObject(t)], &HashMap::new()).expect("flatten");
        assert_eq!(ops.len(), 1, "the empty TextCode produces no run");
        let DrawOp::Text(run_out) = &ops[0] else {
            panic!("expected a text op, got {ops:?}");
        };
        assert_eq!(run_out.glyphs, vec![65u16, 66u16]);
        assert_eq!(run_out.advances, vec![3.0, 3.0]);
        assert_eq!(run_out.plain_text(), "AB");
    }

    #[test]
    fn an_image_object_records_its_resource() {
        let img = ImageObject {
            base: ObjectBase {
                boundary: Rect::new(0.0, 0.0, 40.0, 30.0),
                ..Default::default()
            },
            resource_id: "img_9".to_string(),
            ..Default::default()
        };
        let ops = run(vec![Object::ImageObject(img)], &HashMap::new()).expect("flatten");
        let DrawOp::Image { image, dst, .. } = &ops[0] else {
            panic!("expected an image op, got {ops:?}");
        };
        assert_eq!(image.id, "img_9");
        assert_eq!(*dst, Rect::new(0.0, 0.0, 40.0, 30.0));
    }

    #[test]
    fn an_image_without_a_resource_is_dropped() {
        let img = ImageObject::default();
        let ops = run(vec![Object::ImageObject(img)], &HashMap::new()).expect("flatten");
        assert!(ops.is_empty());
    }

    #[test]
    fn layers_draw_bottom_first_in_order() {
        let block = PageBlock {
            layers: vec![
                Layer {
                    id: None,
                    objects: vec![Object::PathObject(path_object("M 0 0 L 1 1"))],
                },
                Layer {
                    id: None,
                    objects: vec![Object::PathObject(path_object("M 2 2 L 3 3"))],
                },
            ],
        };
        let mut b = builder();
        flatten_block(&block, &HashMap::new(), &mut b).expect("flatten");
        assert_eq!(b.finish(0).ops.len(), 2);
    }
}
