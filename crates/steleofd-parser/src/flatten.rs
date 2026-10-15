//! Flattening of the model tree into display commands.
//! Every object is emitted with its CTM already folded into the graphics
//! state, so downstream backends see absolute page-space commands only.

use std::collections::HashMap;
use std::sync::Arc;

use steleofd_common::Point;
use steleofd_display::ops::{Color, GradientStop, LineCap, LineJoin};
use steleofd_display::{
    DisplayListBuilder, DrawOp, GlyphRun, ImageCodec, ImageRef, Paint, PaintRule, StrokeStyle,
};
use steleofd_error::{FormatError, OfdError, Result};
use steleofd_font::FontId;
use steleofd_geometry::{Affine, Path};
use steleofd_model::PageBlock;
use steleofd_model::color::{ColorRef, Segment, Shading};
use steleofd_model::object::{
    DrawParam, ImageObject, LineCapStyle, LineJoinStyle, Object, ObjectBase, PathObject, TextObject,
};
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

/// Resolve a colour into a paint, emitting a gradient op first when the colour
/// turns out to be a shading. Clause 8.3.4 draws a shading as its own graphic
/// unit, so the gradient op carries the path and the paint returned is a marker.
fn paint_of(color: &ColorRef, path: &Path, builder: &mut DisplayListBuilder) -> Result<Paint> {
    match color.shading() {
        None => solid(color),
        Some(Shading::Axial {
            start,
            end,
            segments,
        }) => {
            let stops = stops_of(segments)?;
            let index = builder.len();
            builder.push(DrawOp::LinearGradient {
                start: Point::new(start.0, start.1),
                end: Point::new(end.0, end.1),
                stops,
                path: path.clone(),
            });
            Ok(Paint::LinearGradientRef(index))
        }
        Some(Shading::Radial {
            start,
            start_radius,
            end,
            end_radius,
            segments,
        }) => {
            let stops = stops_of(segments)?;
            let index = builder.len();
            // Only the start circle is painted; the end circle the clause also
            // gives is not folded in yet.
            let _ = end;
            let _ = end_radius;
            builder.push(DrawOp::RadialGradient {
                center: Point::new(start.0, start.1),
                radius: *start_radius,
                stops,
                path: path.clone(),
            });
            Ok(Paint::RadialGradientRef(index))
        }
    }
}

fn stops_of(segments: &[Segment]) -> Result<Vec<GradientStop>> {
    segments
        .iter()
        .map(|s| {
            let (r, g, b) = s.color.to_rgb()?;
            Ok(GradientStop {
                offset: s.position,
                color: Color::rgba(r, g, b, s.color.clone().alpha()),
            })
        })
        .collect()
}

/// GB/T 33190 clause 8.4: the clip areas an object narrows its drawing to.
/// The first area of the first clip wins; areas beyond it are reported rather
/// than silently dropped.
fn clip_of(
    base: &ObjectBase,
    params: &HashMap<String, DrawParam>,
) -> Result<Option<(Path, PaintRule)>> {
    let Some(list) = base.clips.as_ref() else {
        return Ok(None);
    };
    if list.items.len() > 1 {
        return Err(nested_clips(list.items.len()));
    }
    let Some(area) = list.items.first() else {
        return Ok(None);
    };
    let Some(data) = area.path_data.as_deref().map(str::trim) else {
        return Ok(None);
    };
    if data.is_empty() {
        return Ok(None);
    }
    let mut path: Path = crate::pathdata::parse_path(data)?;
    // An area may carry its own CTM, applied to the clip path alone.
    if let Some(raw) = area.ctm.as_deref() {
        let v = parse_number_list_n::<6>("Area", "CTM", raw)?;
        path = path.transformed(&Affine::new(v[0], v[1], v[2], v[3], v[4], v[5]));
    }
    // A clip area's DrawParam may name the fill rule that decides which side
    // of the outline is inside the clip.
    let rule = area
        .draw_param
        .as_ref()
        .and_then(|id| params.get(id))
        .map(|_| PaintRule::NonZero)
        .unwrap_or(PaintRule::NonZero);
    Ok(Some((path, rule)))
}

/// Clip lists with more than one area are valid in principle but not read yet;
/// saying so beats drawing a page that quietly lost part of its mask.
fn nested_clips(count: usize) -> OfdError {
    OfdError::Xml(steleofd_error::XmlError::UnsupportedExtension(format!(
        "clip list with {count} areas"
    )))
}

/// Wrap a graphic unit in its clip, so the state stack stays balanced.
fn with_clip(
    base: &ObjectBase,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
    body: impl FnOnce(&mut DisplayListBuilder) -> Result<()>,
) -> Result<()> {
    match clip_of(base, params)? {
        None => body(builder),
        Some((path, rule)) => {
            builder.save()?;
            builder.push(DrawOp::ClipPath { path, rule });
            let result = body(builder);
            builder.restore()?;
            result
        }
    }
}

fn default_color() -> ColorRef {
    ColorRef::from_hex("#000000")
}

/// GB/T 33190 clause 9.1: a graphic unit may state the stroke parameters
/// itself, and those win over the ones its `DrawParam` carries.
fn stroke_style_of(base: &ObjectBase, param: Option<&DrawParam>) -> StrokeStyle {
    let dash = param
        .and_then(|p| p.dash_pattern.as_ref())
        .map(|d| d.pattern.clone())
        .unwrap_or_default();
    let dash_offset = param
        .and_then(|p| p.dash_pattern.as_ref())
        .map(|d| d.phase)
        .unwrap_or(0.0);
    StrokeStyle {
        width: base
            .line_width
            .or_else(|| param.and_then(|p| p.line_width))
            .unwrap_or(DEFAULT_LINE_WIDTH),
        dash,
        dash_offset,
        cap: match base.cap.or_else(|| param.and_then(|p| p.line_cap)) {
            Some(LineCapStyle::Round) => LineCap::Round,
            Some(LineCapStyle::Square) => LineCap::Square,
            _ => LineCap::Butt,
        },
        join: match base.join.or_else(|| param.and_then(|p| p.line_join)) {
            Some(LineJoinStyle::Round) => LineJoin::Round,
            Some(LineJoinStyle::Bevel) => LineJoin::Bevel,
            _ => LineJoin::Miter,
        },
        miter_limit: base.miter_limit.unwrap_or(4.0),
    }
}

fn fill_rule(rule: Option<steleofd_model::object::FillRuleStyle>) -> PaintRule {
    match rule {
        Some(steleofd_model::object::FillRuleStyle::EvenOdd) => PaintRule::EvenOdd,
        _ => PaintRule::NonZero,
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
    let style = stroke_style_of(&p.base, param);
    let black = default_color();
    let fill_color = p
        .fill_color
        .as_ref()
        .or(param.and_then(|d| d.fill_color.as_ref()))
        .unwrap_or(&black);
    let stroke_color = p
        .stroke_color
        .as_ref()
        .or(param.and_then(|d| d.stroke_color.as_ref()))
        .unwrap_or(&black);
    with_ctm(&p.base, builder, |b| {
        with_clip(&p.base, params, b, |b| {
            let fill = if fill_on {
                Some(paint_of(fill_color, &path, b)?)
            } else {
                None
            };
            let stroke = if stroke_on {
                Some(paint_of(stroke_color, &path, b)?)
            } else {
                None
            };
            b.push(DrawOp::Path {
                path,
                fill,
                stroke,
                stroke_style: style,
                rule: fill_rule(p.rule),
            });
            Ok(())
        })
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
    let fill_on = t.fill.or(param.and_then(|d| d.fill)).unwrap_or(true);
    // Text is filled by default and stroked only when asked, which is the
    // opposite of a path's defaults.
    let stroke_on = t.stroke.or(param.and_then(|d| d.stroke)).unwrap_or(false);
    let fill = if fill_on {
        Some(solid(
            t.fill_color
                .as_ref()
                .or(param.and_then(|d| d.fill_color.as_ref()))
                .unwrap_or(&default_color()),
        )?)
    } else {
        None
    };
    let stroke = if stroke_on {
        Some(solid(
            t.stroke_color
                .as_ref()
                .or(param.and_then(|d| d.stroke_color.as_ref()))
                .unwrap_or(&default_color()),
        )?)
    } else {
        None
    };
    let size = t.size.value();
    // HScale widens or narrows the glyphs without changing their advance.
    let h_scale = t.h_scale.unwrap_or(1.0);
    let letter_spacing = t.char_space.unwrap_or(0.0);
    let font = FontId(t.font.parse::<u32>().unwrap_or(0));
    with_ctm(&t.base, builder, |b| {
        with_clip(&t.base, params, b, |b| {
            for code in &t.codes {
                if code.text.is_empty() {
                    continue;
                }
                // Glyph ids are placeholders until font shaping lands; the raw
                // code points keep text extraction working in the meantime.
                let glyphs: Vec<u16> = code.text.chars().map(|c| u32::from(c) as u16).collect();
                let advances = advances_of(code, size);
                let place = Affine::translate(code.x, code.y);
                let transform = if h_scale == 1.0 {
                    place
                } else {
                    Affine::new(h_scale, 0.0, 0.0, 1.0, 0.0, 0.0).then(place)
                };
                b.push(DrawOp::Text(GlyphRun {
                    font,
                    size: t.size,
                    glyphs,
                    advances,
                    text: Some(code.text.clone()),
                    cluster_map: None,
                    transform,
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                    letter_spacing,
                    word_spacing: 0.0,
                    bbox: t.base.boundary,
                }));
            }
            Ok(())
        })
    })
}

/// Per-glyph advances: the font size, plus the `DeltaX`/`DeltaY` adjustments
/// the standard lets a run carry.
fn advances_of(code: &steleofd_model::object::TextCode, size: f64) -> Vec<f64> {
    let count = code.text.chars().count();
    (0..count)
        .map(|i| {
            let dx = code.delta_x.get(i).copied().unwrap_or(0.0);
            let dy = code.delta_y.get(i).copied().unwrap_or(0.0);
            size + dx + dy
        })
        .collect()
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
        with_clip(&i.base, &HashMap::new(), b, |b| {
            b.push(DrawOp::Image {
                image,
                dst: i.base.boundary,
                transform: Affine::IDENTITY,
            });
            Ok(())
        })
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
                    text: "AB".to_string(),
                    ..Default::default()
                },
                TextCode {
                    x: 10.0,
                    y: 0.0,
                    text: String::new(),
                    ..Default::default()
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
