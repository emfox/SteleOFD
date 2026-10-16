//! Flattening of the model tree into display commands.
//! Every object is emitted with its CTM already folded into the graphics
//! state, so downstream backends see absolute page-space commands only.

use std::collections::HashMap;
use std::sync::Arc;

use steleofd_common::{Point, Rect};
use steleofd_display::ops::{Color, GradientStop, LineCap, LineJoin};
use steleofd_display::{
    DisplayListBuilder, DrawOp, GlyphRun, ImageCodec, ImageRef, LinearGradientSpec, MeshSpec,
    MeshVertex, Paint, PaintRule, RadialGradientSpec, StrokeStyle,
};
use steleofd_error::{FormatError, OfdError, Result};
use steleofd_font::FontId;
use steleofd_geometry::{Affine, Path};
use steleofd_model::PageBlock;
use steleofd_model::color::{ColorRef, MeshPoint, PatternSpec, ReflectMethod, Segment, Shading};
use steleofd_model::object::{
    CGTransform, DrawParam, ImageObject, LineCapStyle, LineJoinStyle, Object, ObjectBase,
    PathObject, TextCode, TextObject,
};
use steleofd_xml::parse_number_list_n;

/// Default stroke width in millimetres when neither the object nor its draw
/// parameter says otherwise; the standard names 0.35 mm as the default.
const DEFAULT_LINE_WIDTH: f64 = 0.35;

/// The vector graphics a document embeds, keyed by resource id. A
/// `CompositeObject` names one of these through its `ResourceID`.
pub type CompositeTable = HashMap<String, steleofd_model::CompositeGraphicUnit>;

/// An empty composite table, for callers that have no document behind them.
pub fn no_composites() -> CompositeTable {
    CompositeTable::new()
}

/// Flatten a whole page or template block onto the builder.
pub fn flatten_block(
    block: &PageBlock,
    params: &HashMap<String, DrawParam>,
    composites: &CompositeTable,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    for layer in &block.layers {
        for object in &layer.objects {
            flatten_object(object, params, composites, builder)?;
        }
    }
    Ok(())
}

fn flatten_object(
    object: &Object,
    params: &HashMap<String, DrawParam>,
    composites: &CompositeTable,
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
        Object::CompositeObject(c) => flatten_composite(c, params, composites, builder),
        Object::Unknown(_) => Ok(()),
    }
}

/// GB/T 33190 clause 8.5: every object exposes the same base attributes.
fn visible_of(object: &Object) -> Option<bool> {
    match object {
        Object::PathObject(o) => o.base.visible,
        Object::TextObject(o) => o.base.visible,
        Object::ImageObject(o) => o.base.visible,
        Object::CompositeObject(o) => o.base.visible,
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

/// An object's clip, which clause 8.4 lets be a path or a stretch of text.
enum ClipSpec {
    /// A region bounded by path geometry.
    Path { path: Path, rule: PaintRule },
    /// A region bounded by glyph outlines, left for a backend to resolve.
    Text(Vec<GlyphRun>),
}

/// Place an object into the page. Content space starts at the boundary's
/// top-left corner and the CTM maps from there into page space; the corpus
/// agrees on this for every producer sampled.
fn with_object_space(
    base: &ObjectBase,
    params: &HashMap<String, DrawParam>,
    builder: &mut DisplayListBuilder,
    body: impl FnOnce(&mut DisplayListBuilder) -> Result<()>,
) -> Result<()> {
    let ctm = ctm_of(base)?;
    let origin = Affine::translate(base.boundary.x, base.boundary.y);
    let clip = clip_of(base, params)?;
    // One scope carries both matrices: the CTM takes the content into
    // millimetres from the boundary corner, the boundary then moves that
    // corner onto the page. The clip shares that space and rides the scope.
    let single = match ctm {
        Some(t) => t.then(origin),
        None => origin,
    };
    if single.is_identity() && clip.is_none() {
        return body(builder);
    }
    // Keep the state stack balanced even when the body fails so later objects
    // start from a clean state.
    builder.save()?;
    if !single.is_identity() {
        builder.transform(single);
    }
    let result = match clip {
        None => body(builder),
        Some(ClipSpec::Path { path, rule }) => {
            builder.push(DrawOp::ClipPath { path, rule });
            body(builder)
        }
        Some(ClipSpec::Text(runs)) => {
            for run in runs {
                builder.push(DrawOp::ClipText(run));
            }
            body(builder)
        }
    };
    builder.restore()?;
    result
}

fn solid(color: &ColorRef) -> Result<Paint> {
    // A colour carrying a shading has no flat value: the gradient is the
    // colour. Text reaches here without going through `paint_of`, so such a
    // run read as invalid and took the document with it.
    let (r, g, b) = if color.shading.is_some() {
        (0, 0, 0)
    } else {
        color.to_rgb()?
    };
    let alpha = color.clone().alpha();
    Ok(Paint::Solid(Color::rgba(r, g, b, alpha)))
}

/// Resolve a colour into a paint. A colour carrying a shading becomes a
/// gradient paint that travels with the object, so no backend depends on
/// having seen some earlier op to know what the fill is.
fn paint_of(color: &ColorRef, _path: &Path, _builder: &mut DisplayListBuilder) -> Result<Paint> {
    match color.shading() {
        None => solid(color),
        Some(Shading::Axial {
            start,
            end,
            segments,
        }) => Ok(Paint::LinearGradient(Box::new(LinearGradientSpec {
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
            stops: stops_of(segments)?,
        }))),
        Some(Shading::Radial {
            start,
            start_radius,
            end,
            end_radius,
            segments,
        }) => Ok(Paint::RadialGradient(Box::new(RadialGradientSpec {
            center: Point::new(start.0, start.1),
            radius: *start_radius,
            end: Point::new(end.0, end.1),
            end_radius: *end_radius,
            stops: stops_of(segments)?,
        }))),
        Some(Shading::Gouraud {
            points, triangles, ..
        }) => Ok(Paint::Mesh(Box::new(MeshSpec {
            points: mesh_vertices(points)?,
            triangles: triangles.iter().map(|t| [t.a, t.b, t.c]).collect(),
        }))),
        Some(Shading::Pattern(spec)) => Ok(Paint::Pattern(Box::new(pattern_spec(spec)?))),
    }
}

/// Flatten a pattern's cell content once, so a backend only has to tile the
/// finished ops. The cell sits in its own coordinates anchored at the origin,
/// which is what a repeating tile means.
fn pattern_spec(spec: &PatternSpec) -> Result<steleofd_display::PatternSpec> {
    let cell_box = Rect::new(0.0, 0.0, spec.width.max(0.0), spec.height.max(0.0));
    let params = HashMap::new();
    let composites = no_composites();
    let mut builder = DisplayListBuilder::new(cell_box);
    if let Some(raw) = &spec.ctm {
        let v = parse_number_list_n::<6>("Pattern", "CTM", raw)?;
        builder.transform(Affine::new(v[0], v[1], v[2], v[3], v[4], v[5]));
    }
    let block = PageBlock {
        layers: spec.layers.clone(),
    };
    flatten_block(&block, &params, &composites, &mut builder)?;
    let cell = builder.finish(0);
    Ok(steleofd_display::PatternSpec {
        cell: Box::new(cell),
        width: spec.width,
        height: spec.height,
        step_x: spec.step_x(),
        step_y: spec.step_y(),
        reflect: match spec.reflect {
            ReflectMethod::Normal => steleofd_display::Reflect::Normal,
            ReflectMethod::Row => steleofd_display::Reflect::Row,
            ReflectMethod::Column => steleofd_display::Reflect::Column,
            ReflectMethod::RowAndColumn => steleofd_display::Reflect::RowAndColumn,
        },
    })
}

/// One vertex per mesh point, its colour resolved to RGB here so the backend
/// interpolates numbers rather than parsing colour text per pixel.
fn mesh_vertices(points: &[MeshPoint]) -> Result<Vec<MeshVertex>> {
    points
        .iter()
        .map(|p| {
            let color = ColorRef::from_hex(p.color.clone()).with_alpha(p.alpha);
            let (r, g, b) = color.to_rgb()?;
            Ok(MeshVertex {
                x: p.point.0,
                y: p.point.1,
                color: Color::rgba(r, g, b, p.alpha),
            })
        })
        .collect()
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
/// The region is the union of every area of every clip, so paths are read as
/// one path and text areas become clip runs for a backend to outline.
fn clip_of(base: &ObjectBase, _params: &HashMap<String, DrawParam>) -> Result<Option<ClipSpec>> {
    let Some(list) = base.clips.as_ref() else {
        return Ok(None);
    };
    let mut joined = Path::default();
    let mut rule = PaintRule::NonZero;
    let mut rule_stated = false;
    let mut text_runs: Vec<GlyphRun> = Vec::new();
    for clip in &list.items {
        for area in &clip.areas {
            // A text area contributes glyphs, a path area contributes segments;
            // both end up intersecting the same region.
            if let Some(text) = area.text.as_ref() {
                text_runs.extend(text_clip_runs(text));
                continue;
            }
            let Some(data) = area.path_data.as_deref().map(str::trim) else {
                continue;
            };
            if data.is_empty() {
                continue;
            }
            let mut path: Path = crate::pathdata::parse_path(data)?;
            // An area may carry its own CTM, applied to the clip path alone.
            if let Some(raw) = area.ctm.as_deref() {
                let v = parse_number_list_n::<6>("Area", "CTM", raw)?;
                path = path.transformed(&Affine::new(v[0], v[1], v[2], v[3], v[4], v[5]));
            }
            // One path carries one fill rule, so where areas disagree the first
            // one that states a rule decides which side of the outline is in.
            if !rule_stated && let Some(r) = area.rule.as_deref() {
                rule = paint_rule(r);
                rule_stated = true;
            }
            joined.segments.extend(path.segments);
        }
    }
    if !joined.is_empty() {
        return Ok(Some(ClipSpec::Path { path: joined, rule }));
    }
    if !text_runs.is_empty() {
        return Ok(Some(ClipSpec::Text(text_runs)));
    }
    Ok(None)
}

/// Turn a `Text` clip area into the runs a backend draws the outlines from.
/// The shape matches a drawn text object, so the glyph mapping is applied the
/// same way, and a run with no drawable codes is dropped.
fn text_clip_runs(clip: &steleofd_model::TextClip) -> Vec<GlyphRun> {
    let size = steleofd_common::Millimeter::new(clip.size.unwrap_or(1.0));
    let font = FontId(clip.font.parse::<u32>().unwrap_or(0));
    let mut runs = Vec::new();
    for code in &clip.codes {
        if code.text.is_empty() {
            continue;
        }
        for segment in segments_of(code, &clip.cg_transforms, size.value()) {
            runs.push(GlyphRun {
                font,
                size,
                glyphs: segment.glyphs,
                advances: segment.advances,
                text: Some(segment.text),
                cluster_map: segment.cluster_map,
                codes_are_glyphs: segment.glyph_index,
                transform: Affine::translate(code.x, code.y),
                fill: None,
                stroke: None,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                bbox: Rect::new(0.0, 0.0, 0.0, 0.0),
            });
        }
    }
    runs
}

/// The fill rule an area names; clause 9.1 allows either spelling.
fn paint_rule(raw: &str) -> PaintRule {
    if raw.eq_ignore_ascii_case("even-odd") || raw.eq_ignore_ascii_case("evenodd") {
        PaintRule::EvenOdd
    } else {
        PaintRule::NonZero
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
    // `CT_Path` defaults `Stroke` true and `Fill` false, the reverse of the
    // usual pair, so an object that names only a stroke stays a stroke.
    let fill_on = p.fill.or(param.and_then(|d| d.fill)).unwrap_or(false);
    let stroke_on = p.stroke.or(param.and_then(|d| d.stroke)).unwrap_or(true);
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
    with_object_space(&p.base, params, builder, |b| {
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
    with_object_space(&t.base, params, builder, |b| {
        for code in &t.codes {
            if code.text.is_empty() {
                continue;
            }
            for segment in segments_of(code, &t.cg_transforms, size) {
                let place = Affine::translate(code.x + segment.offset, code.y);
                let transform = if h_scale == 1.0 {
                    place
                } else {
                    Affine::new(h_scale, 0.0, 0.0, 1.0, 0.0, 0.0).then(place)
                };
                b.push(DrawOp::Text(GlyphRun {
                    font,
                    size: t.size,
                    glyphs: segment.glyphs,
                    advances: segment.advances,
                    text: Some(segment.text),
                    cluster_map: segment.cluster_map,
                    codes_are_glyphs: segment.glyph_index,
                    transform,
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                    letter_spacing,
                    word_spacing: 0.0,
                    // Text codes address the object space, so the box lives
                    // at its origin too.
                    bbox: Rect::new(0.0, 0.0, t.base.boundary.w, t.base.boundary.h),
                }));
            }
        }
        Ok(())
    })
}

/// One stretch of a `TextCode` as it reaches the display list.
struct RunSegment {
    /// Codes to draw: glyph indices when `glyph_index` is set, Unicode code
    /// points otherwise.
    glyphs: Vec<u16>,
    /// Whether `glyphs` holds indices into the font's own glyph space.
    glyph_index: bool,
    /// Character index each glyph belongs to; absent when the two agree.
    cluster_map: Option<Vec<u32>>,
    /// Horizontal origin of this stretch within the text code.
    offset: f64,
    /// Per-glyph advances, in the text code's own order.
    advances: Vec<f64>,
    /// The characters this stretch covers.
    text: String,
}

/// Split a `TextCode` into the stretches a `CGTransform` carves out of it.
/// Untransformed characters keep their code points; a transformed stretch
/// carries glyph indices, so the two kinds never share a run.
fn segments_of(code: &TextCode, transforms: &[CGTransform], size: f64) -> Vec<RunSegment> {
    let chars: Vec<char> = code.text.chars().collect();
    let advance_at = |i: usize| {
        let dx = code.delta_x.get(i).copied().unwrap_or(0.0);
        let dy = code.delta_y.get(i).copied().unwrap_or(0.0);
        size + dx + dy
    };

    // A transform names the characters it covers and the glyphs that replace
    // them; one that names no glyph, or covers nothing, draws nothing.
    let mut spans: Vec<(usize, usize, &CGTransform)> = Vec::new();
    for t in transforms {
        if t.glyphs.is_empty() {
            continue;
        }
        let start = t.code_position.min(chars.len());
        let end = (start + t.code_count.max(1)).min(chars.len());
        if start < end {
            spans.push((start, end, t));
        }
    }
    spans.sort_by_key(|(start, _, _)| *start);

    let mut out: Vec<RunSegment> = Vec::new();
    let mut at = 0usize;
    let mut offset = 0.0f64;
    for (start, end, t) in spans {
        if start < at {
            // Overlapping transforms: the first to reach a character wins.
            continue;
        }
        if start > at {
            let glyphs: Vec<u16> = chars[at..start]
                .iter()
                .map(|c| u32::from(*c) as u16)
                .collect();
            let advances: Vec<f64> = (at..start).map(advance_at).collect();
            let span: f64 = advances.iter().sum();
            out.push(RunSegment {
                glyphs,
                glyph_index: false,
                cluster_map: None,
                offset,
                advances,
                text: chars[at..start].iter().collect(),
            });
            offset += span;
        }
        // A glyph maps to the first character it replaces, so a one-to-many
        // transform keeps every glyph anchored to the same character.
        let glyphs = t.glyphs.clone();
        let cluster_map: Vec<u32> = (0..glyphs.len())
            .map(|i| (start + i.min(end - start - 1)) as u32)
            .collect();
        let advances: Vec<f64> = cluster_map
            .iter()
            .map(|&c| advance_at(c as usize))
            .collect();
        let span: f64 = advances.iter().sum();
        out.push(RunSegment {
            glyphs,
            glyph_index: true,
            cluster_map: Some(cluster_map),
            offset,
            advances,
            text: chars[start..end].iter().collect(),
        });
        offset += span;
        at = end;
    }
    if at < chars.len() {
        let glyphs: Vec<u16> = chars[at..].iter().map(|c| u32::from(*c) as u16).collect();
        let advances: Vec<f64> = (at..chars.len()).map(advance_at).collect();
        out.push(RunSegment {
            glyphs,
            glyph_index: false,
            cluster_map: None,
            offset,
            advances,
            text: chars[at..].iter().collect(),
        });
    }
    out
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
    with_object_space(&i.base, &HashMap::new(), builder, |b| {
        // Image content fills the unit square of the object space; the
        // CTM inside the scope above scales it onto the boundary. Feeding
        // the boundary here as well would scale the placement twice.
        b.push(DrawOp::Image {
            image,
            dst: Rect::new(0.0, 0.0, 1.0, 1.0),
            transform: Affine::IDENTITY,
        });
        Ok(())
    })
}

/// GB/T 33190 clause 13: a composite object places a vector resource. The
/// graphic is drawn when the resource carries one; otherwise the substitution
/// image stands in for it, which is what the substitution exists for.
fn flatten_composite(
    c: &steleofd_model::object::CompositeObject,
    params: &HashMap<String, DrawParam>,
    composites: &CompositeTable,
    builder: &mut DisplayListBuilder,
) -> Result<()> {
    if c.resource_id.is_empty() {
        return Ok(());
    }
    let unit = composites.get(c.resource_id.as_str());
    with_object_space(&c.base, params, builder, |b| {
        if let Some(block) = unit.and_then(|u| u.content.as_ref()) {
            // The graphic's own objects, drawn inside the placement box.
            flatten_block(block, params, composites, b)
        } else if let Some(sub) = unit.and_then(|u| u.substitution.as_ref()) {
            // No vector content: paint the stand-in, whose bytes the image
            // resolver fills in later, exactly as an image object's are.
            b.push(DrawOp::Image {
                image: ImageRef::new(sub.clone(), Arc::new(Vec::new()), (1, 1), ImageCodec::Png),
                dst: Rect::new(0.0, 0.0, 1.0, 1.0),
                transform: Affine::IDENTITY,
            });
            Ok(())
        } else {
            // Nothing to draw and nothing to stand in: the placement is empty.
            Ok(())
        }
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
        flatten_block(&block, params, &no_composites(), &mut b)?;
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

    /// A path object that asks for a fill, so the colour defaults are visible
    /// without the object also asking for a stroke.
    fn filled_path_object(data: &str) -> PathObject {
        PathObject {
            fill: Some(true),
            ..path_object(data)
        }
    }

    #[test]
    fn an_absent_fill_means_not_filled_and_an_absent_stroke_means_stroked() {
        let ops = run(
            vec![Object::PathObject(path_object("M 0 0 L 10 10"))],
            &HashMap::new(),
        )
        .expect("flatten");
        let DrawOp::Path { fill, stroke, .. } = &ops[0] else {
            panic!("expected a path op, got {ops:?}");
        };
        assert!(fill.is_none());
        assert!(stroke.is_some());
    }

    #[test]
    fn object_colours_default_to_black() {
        let ops = run(
            vec![Object::PathObject(filled_path_object("M 0 0 L 10 10"))],
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
        let mut p = filled_path_object("M 0 0 L 10 10");
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
        p.base.boundary = Rect::new(20.0, 30.0, 10.0, 10.0);
        p.base.ctm = Some("2 0 0 2 5 5".to_string());
        let ops = run(vec![Object::PathObject(p)], &HashMap::new()).expect("flatten");
        // Origin and CTM fold into one matrix: the origin lands at the
        // boundary corner and the CTM applies inside it.
        assert!(matches!(ops[0], DrawOp::Save));
        assert!(matches!(ops[1], DrawOp::Transform(t) if t.apply(0.0, 0.0) == (25.0, 35.0)));
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
        let drawn: Vec<_> = ops
            .iter()
            .filter(|op| !matches!(op, DrawOp::Save | DrawOp::Restore | DrawOp::Transform(_)))
            .collect();
        assert_eq!(drawn.len(), 1, "the empty TextCode produces no run");
        let DrawOp::Text(run_out) = drawn[0] else {
            panic!("expected a text op, got {drawn:?}");
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
        // Images fill the unit square of the object space; the transform op
        // above carries them onto the boundary.
        assert_eq!(*dst, Rect::new(0.0, 0.0, 1.0, 1.0));
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
        flatten_block(&block, &HashMap::new(), &no_composites(), &mut b).expect("flatten");
        assert_eq!(b.finish(0).ops.len(), 2);
    }
}
