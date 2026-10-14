//! Raster rendering backend.
//! Consumes a [`PageDisplay`] and rasterizes its op stream with tiny-skia;
//! coordinates arrive in millimetres and are scaled by the target DPI.

use std::sync::Arc;

use steleofd_common::Rect;
use steleofd_display::ops::{Color, GradientStop, LineCap, LineJoin};
use steleofd_display::{DrawOp, GlyphRun, ImageRef, PageDisplay, Paint, PaintRule, StrokeStyle};
use steleofd_error::{OfdError, RenderError, Result};
use steleofd_font::FontManager;
use steleofd_geometry::{Affine, Path, PathSegment};
use tiny_skia::{
    FillRule, IntSize, LineCap as TsLineCap, LineJoin as TsLineJoin, Mask, Paint as TsPaint,
    Path as TsPath, PathBuilder as TsBuilder, Pixmap, PixmapPaint, Point, Shader, SpreadMode,
    Stroke, StrokeDash, Transform,
};

/// Rasterization parameters.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Output resolution; 96 DPI matches typical screen previews.
    pub dpi: f64,
    /// Colour painted before the page content; `None` keeps transparency.
    pub background: Option<Color>,
    /// Hard ceiling on total pixels, guarding against hostile page boxes.
    pub max_pixels: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            background: Some(Color::WHITE),
            max_pixels: 64 * 1024 * 1024,
        }
    }
}

/// A finished rasterization of one page.
pub struct RasterOutput {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    pixmap: Pixmap,
}

impl RasterOutput {
    /// Access the underlying pixmap for encoding or inspection.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Take the underlying pixmap.
    pub fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    /// Straight (non-premultiplied) RGBA8 bytes, the exchange format most
    /// encoders and UI toolkits expect.
    pub fn rgba_bytes(&self) -> Vec<u8> {
        self.pixmap
            .data()
            .chunks_exact(4)
            .flat_map(unpremultiply)
            .collect()
    }
}

fn unpremultiply(px: &[u8]) -> [u8; 4] {
    let a = px[3] as u32;
    if a == 0 || a == 255 {
        return [px[0], px[1], px[2], px[3]];
    }
    let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
    [un(px[0]), un(px[1]), un(px[2]), px[3]]
}

/// Rasterize one page of a display list.
pub fn render_page(
    page: &PageDisplay,
    fonts: &FontManager,
    options: &RenderOptions,
) -> Result<RasterOutput> {
    let (width, height) = page.page_box.size_px(options.dpi);
    if (width as usize).saturating_mul(height as usize) > options.max_pixels {
        return Err(OfdError::Render(RenderError::BudgetExceeded(format!(
            "page raster {width}x{height} exceeds {} pixels",
            options.max_pixels
        ))));
    }
    let mut pixmap =
        Pixmap::new(width, height).ok_or(OfdError::Render(RenderError::InvalidSize {
            w: width,
            h: height,
        }))?;
    if let Some(bg) = options.background {
        pixmap.fill(to_color(&bg));
    }

    let mut state = State {
        ctm: Affine::IDENTITY,
        clip: None,
    };
    let mut stack: Vec<State> = Vec::new();

    for op in &page.ops {
        match op {
            DrawOp::Save => stack.push(state.clone()),
            DrawOp::Restore => {
                state = stack.pop().ok_or_else(|| {
                    OfdError::Render(RenderError::BudgetExceeded(
                        "restore without a matching save".to_string(),
                    ))
                })?;
            }
            DrawOp::Transform(t) => state.ctm = state.ctm.then(*t),
            DrawOp::ClipPath { path, rule } => {
                let total = total_transform(&state, &page.page_box, options.dpi);
                let ts_path = to_ts_path(path, total)?;
                let mut mask =
                    Mask::new(width, height).ok_or(OfdError::Render(RenderError::InvalidSize {
                        w: width,
                        h: height,
                    }))?;
                mask.intersect_path(&ts_path, fill_rule(*rule), true, Transform::identity());
                state.clip = Some(mask);
            }
            DrawOp::Path {
                path,
                fill,
                stroke,
                stroke_style,
                rule,
            } => {
                let total = total_transform(&state, &page.page_box, options.dpi);
                let ts_path = to_ts_path(path, total)?;
                if let Some(paint) = fill {
                    let p = solid_paint(paint);
                    pixmap.fill_path(
                        &ts_path,
                        &p,
                        fill_rule(*rule),
                        Transform::identity(),
                        state.clip.as_ref(),
                    );
                }
                if let Some(paint) = stroke {
                    let p = solid_paint(paint);
                    let ts_stroke = to_ts_stroke(stroke_style, &state, options.dpi);
                    pixmap.stroke_path(
                        &ts_path,
                        &p,
                        &ts_stroke,
                        Transform::identity(),
                        state.clip.as_ref(),
                    );
                }
            }
            DrawOp::Text(run) => {
                draw_text(&mut pixmap, run, &state, page.page_box, fonts, options)?
            }
            DrawOp::Image {
                image,
                dst,
                transform,
            } => draw_image(
                &mut pixmap,
                image,
                *dst,
                *transform,
                &state,
                page.page_box,
                options,
            )?,
            DrawOp::LinearGradient {
                start,
                end,
                stops,
                path,
            } => {
                let total = total_transform(&state, &page.page_box, options.dpi);
                let shader = linear_shader(*start, *end, stops, total)?;
                fill_with_shader(&mut pixmap, path, shader, state.clip.as_ref())?;
            }
            DrawOp::RadialGradient {
                center,
                radius,
                stops,
                path,
            } => {
                let total = total_transform(&state, &page.page_box, options.dpi);
                let shader = radial_shader(*center, *radius, stops, total)?;
                fill_with_shader(&mut pixmap, path, shader, state.clip.as_ref())?;
            }
        }
    }

    Ok(RasterOutput {
        width,
        height,
        pixmap,
    })
}

#[derive(Clone)]
struct State {
    ctm: Affine,
    clip: Option<Mask>,
}

fn to_color(c: &Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

/// Page-space millimetres to pixmap pixels, page box origin included.
fn total_transform(state: &State, page_box: &Rect, dpi: f64) -> Transform {
    let s = dpi / 25.4;
    let mm_to_px = Affine::translate(-page_box.x * s, -page_box.y * s).then(Affine::scale(s, s));
    to_ts(mm_to_px.then(state.ctm))
}

/// Convert an affine matrix into a tiny-skia transform, pixel for pixel.
fn to_ts(a: Affine) -> Transform {
    Transform::from_row(
        a.a as f32, a.b as f32, a.c as f32, a.d as f32, a.e as f32, a.f as f32,
    )
}

fn fill_rule(rule: PaintRule) -> FillRule {
    match rule {
        PaintRule::NonZero => FillRule::Winding,
        PaintRule::EvenOdd => FillRule::EvenOdd,
    }
}

fn solid_paint(paint: &Paint) -> TsPaint<'static> {
    // Gradient and pattern references are emitted as dedicated gradient ops;
    // a solid fallback keeps malformed references visible but harmless.
    let color = match paint {
        Paint::Solid(c) => to_color(c),
        _ => tiny_skia::Color::BLACK,
    };
    TsPaint {
        shader: Shader::SolidColor(color),
        anti_alias: true,
        ..TsPaint::default()
    }
}

fn to_ts_stroke(style: &StrokeStyle, state: &State, dpi: f64) -> Stroke {
    // Dash lengths and widths scale with the CTM, so a zoomed page keeps
    // dash proportions instead of compressing them.
    let s = dpi / 25.4 * state.ctm.scale_factor();
    let dash = if style.dash.is_empty() {
        None
    } else {
        let pattern: Vec<f32> = style.dash.iter().map(|d| (*d * s) as f32).collect();
        StrokeDash::new(pattern, (style.dash_offset * s) as f32)
    };
    Stroke {
        width: (style.width.max(0.0) * s) as f32,
        miter_limit: style.miter_limit as f32,
        line_cap: match style.cap {
            LineCap::Butt => TsLineCap::Butt,
            LineCap::Round => TsLineCap::Round,
            LineCap::Square => TsLineCap::Square,
        },
        line_join: match style.join {
            LineJoin::Miter => TsLineJoin::Miter,
            LineJoin::Round => TsLineJoin::Round,
            LineJoin::Bevel => TsLineJoin::Bevel,
        },
        dash,
    }
}

fn push_segment(builder: &mut TsBuilder, seg: &PathSegment, cur: &mut (f64, f64)) {
    match seg {
        PathSegment::MoveTo(x, y) => {
            builder.move_to(*x as f32, *y as f32);
            *cur = (*x, *y);
        }
        PathSegment::LineTo(x, y) => {
            builder.line_to(*x as f32, *y as f32);
            *cur = (*x, *y);
        }
        PathSegment::CurveTo { c1, c2, to } => {
            builder.cubic_to(
                c1.0 as f32,
                c1.1 as f32,
                c2.0 as f32,
                c2.1 as f32,
                to.0 as f32,
                to.1 as f32,
            );
            *cur = *to;
        }
        PathSegment::QuadTo { c, to } => {
            // Elevate the quadratic to a cubic; an exact transform exists.
            let c1 = (
                cur.0 + 2.0 / 3.0 * (c.0 - cur.0),
                cur.1 + 2.0 / 3.0 * (c.1 - cur.1),
            );
            let c2 = (
                to.0 + 2.0 / 3.0 * (c.0 - to.0),
                to.1 + 2.0 / 3.0 * (c.1 - to.1),
            );
            builder.cubic_to(
                c1.0 as f32,
                c1.1 as f32,
                c2.0 as f32,
                c2.1 as f32,
                to.0 as f32,
                to.1 as f32,
            );
            *cur = *to;
        }
        PathSegment::Arc {
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            to,
        } => {
            for (c1, c2, end) in
                arc_cubics(*rx, *ry, *x_axis_rotation, *large_arc, *sweep, *cur, *to)
            {
                builder.cubic_to(
                    c1.0 as f32,
                    c1.1 as f32,
                    c2.0 as f32,
                    c2.1 as f32,
                    end.0 as f32,
                    end.1 as f32,
                );
            }
            *cur = *to;
        }
        PathSegment::Close => builder.close(),
    }
}

fn to_ts_path(path: &Path, transform: Transform) -> Result<TsPath> {
    let mut builder = TsBuilder::new();
    let mut cur = (0.0f64, 0.0f64);
    for seg in &path.segments {
        push_segment(&mut builder, seg, &mut cur);
    }
    builder
        .finish()
        .and_then(|p| p.transform(transform))
        .ok_or_else(|| {
            OfdError::Render(RenderError::BudgetExceeded(
                "path transform was not invertible".to_string(),
            ))
        })
}

/// One cubic segment of a flattened arc: two control points and the endpoint.
type Cubic = ((f64, f64), (f64, f64), (f64, f64));

/// Convert one elliptical arc into cubic segments, following the same
/// endpoint-to-centre conversion the SVG standard prescribes.
#[allow(clippy::too_many_arguments)]
fn arc_cubics(
    rx_in: f64,
    ry_in: f64,
    rot_deg: f64,
    large: bool,
    sweep: bool,
    from: (f64, f64),
    to: (f64, f64),
) -> Vec<Cubic> {
    let (x1, y1) = from;
    let (x2, y2) = to;
    if rx_in == 0.0 || ry_in == 0.0 {
        return vec![(
            ((x1 + x2) / 2.0, (y1 + y2) / 2.0),
            ((x1 + x2) / 2.0, (y1 + y2) / 2.0),
            (x2, y2),
        )];
    }
    let (mut rx, mut ry) = (rx_in.abs(), ry_in.abs());
    let phi = rot_deg.to_radians();
    let (cos_p, sin_p) = (phi.cos(), phi.sin());
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let x1p = cos_p * dx + sin_p * dy;
    let y1p = -sin_p * dx + cos_p * dy;
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let sign = if large != sweep { 1.0 } else { -1.0 };
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let co = if den == 0.0 {
        0.0
    } else {
        sign * (num / den).max(0.0).sqrt()
    };
    let cxp = co * rx * y1p / ry;
    let cyp = -co * ry * x1p / rx;
    let cx = cos_p * cxp - sin_p * cyp + (x1 + x2) / 2.0;
    let cy = sin_p * cxp + cos_p * cyp + (y1 + y2) / 2.0;

    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let lens = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = if lens == 0.0 {
            0.0
        } else {
            ((ux * vx + uy * vy) / lens).clamp(-1.0, 1.0).acos()
        };
        if ux * vy - uy * vx < 0.0 {
            a = 2.0 * std::f64::consts::PI - a;
        }
        a
    };
    let (ux, uy) = ((x1p - cxp) / rx, (y1p - cyp) / ry);
    let (vx, vy) = ((-x1p - cxp) / rx, (-y1p - cyp) / ry);
    let theta1 = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy);
    if !sweep && delta > 0.0 {
        delta -= 2.0 * std::f64::consts::PI;
    }
    if sweep && delta < 0.0 {
        delta += 2.0 * std::f64::consts::PI;
    }

    let segments = ((delta.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = delta / segments as f64;
    let kappa = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |t: f64| {
        (
            cx + rx * t.cos() * cos_p - ry * t.sin() * sin_p,
            cy + rx * t.cos() * sin_p + ry * t.sin() * cos_p,
        )
    };
    let deriv = |t: f64| {
        (
            -rx * t.sin() * cos_p - ry * t.cos() * sin_p,
            -rx * t.sin() * sin_p + ry * t.cos() * cos_p,
        )
    };

    let mut t0 = theta1;
    let mut out = Vec::with_capacity(segments);
    for _ in 0..segments {
        let t1 = t0 + step;
        let (p0x, p0y) = point(t0);
        let (p3x, p3y) = point(t1);
        let (d1x, d1y) = deriv(t0);
        let (d3x, d3y) = deriv(t1);
        out.push((
            (p0x + kappa * d1x, p0y + kappa * d1y),
            (p3x - kappa * d3x, p3y - kappa * d3y),
            (p3x, p3y),
        ));
        t0 = t1;
    }
    out
}

fn draw_text(
    pixmap: &mut Pixmap,
    run: &GlyphRun,
    state: &State,
    page_box: Rect,
    fonts: &FontManager,
    _options: &RenderOptions,
) -> Result<()> {
    if run.glyphs.is_empty() {
        return Ok(());
    }
    let face = fonts.face(run.font)?;
    let total = total_transform(state, &page_box, 0.0).to_owned().to_owned();
    let _ = total;
    let total = total_transform(state, &page_box, {
        // The DPI is folded into `total_transform`; recover it from the run
        // size instead of threading the option down here.
        96.0
    });
    let _ = total;
    draw_text_inner(pixmap, run, state, page_box, face, _options)
}

fn draw_text_inner(
    pixmap: &mut Pixmap,
    run: &GlyphRun,
    state: &State,
    page_box: Rect,
    face: Arc<steleofd_font::FontFace>,
    options: &RenderOptions,
) -> Result<()> {
    let mut offset = 0.0f64;
    for (i, &gid) in run.glyphs.iter().enumerate() {
        if i > 0 {
            offset += run.advances.get(i - 1).copied().unwrap_or(0.0);
        }
        offset += run.letter_spacing;
        let Some(outline) = face.outline(gid) else {
            continue;
        };
        // Outlines are normalized to 1/1000 em with y pointing up, so the
        // glyph matrix scales by size/1000 and flips the y axis.
        let k = run.size.value() / 1000.0;
        // Relative to the current state: `total_transform` already carries
        // the CTM, so the glyph matrix only positions within it.
        let glyph = run
            .transform
            .then(Affine::translate(offset, 0.0))
            .then(Affine::new(k, 0.0, 0.0, -k, 0.0, 0.0));
        let total = total_transform(state, &page_box, options.dpi);
        let ts_path = to_ts_path(&outline, total.pre_concat(to_ts(glyph)))?;
        if let Some(paint) = &run.fill {
            let p = solid_paint(paint);
            pixmap.fill_path(&ts_path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }
    Ok(())
}

fn draw_image(
    pixmap: &mut Pixmap,
    image: &ImageRef,
    dst: Rect,
    transform: Affine,
    state: &State,
    page_box: Rect,
    options: &RenderOptions,
) -> Result<()> {
    if image.data.is_empty() {
        // A missing resource must not abort the page; leave the area empty.
        return Ok(());
    }
    let decoded = image::load_from_memory(&image.data).map_err(|e| {
        OfdError::Render(RenderError::ImageDecode {
            page: 0,
            reason: format!("resource {}: {e}", image.id),
        })
    })?;
    let rgba = decoded.to_rgba8();
    let (iw, ih) = (rgba.width(), rgba.height());
    let premul: Vec<u8> = rgba.pixels().flat_map(|p| premultiply(p.0)).collect();
    let Some(size) = IntSize::from_wh(iw, ih) else {
        return Err(OfdError::Render(RenderError::ImageDecode {
            page: 0,
            reason: format!("resource {}: size {iw}x{ih} rejected", image.id),
        }));
    };
    let Some(img) = Pixmap::from_vec(premul, size) else {
        return Err(OfdError::Render(RenderError::ImageDecode {
            page: 0,
            reason: format!("resource {}: pixmaps of {}x{} rejected", image.id, iw, ih),
        }));
    };

    // Pixels map into the destination rect, which the placement transform
    // then positions in page space.
    let pixel_to_page = Affine::new(dst.w / iw as f64, 0.0, 0.0, dst.h / ih as f64, dst.x, dst.y);
    let total = total_transform(state, &page_box, options.dpi)
        .pre_concat(to_ts(transform.then(pixel_to_page)));
    pixmap.draw_pixmap(
        0,
        0,
        img.as_ref(),
        &PixmapPaint::default(),
        total,
        state.clip.as_ref(),
    );
    Ok(())
}

fn premultiply(px: [u8; 4]) -> [u8; 4] {
    let a = px[3] as u32;
    if a == 255 {
        return px;
    }
    let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
    [m(px[0]), m(px[1]), m(px[2]), px[3]]
}

fn linear_shader(
    start: steleofd_common::Point,
    end: steleofd_common::Point,
    stops: &[GradientStop],
    total: Transform,
) -> Result<Shader<'static>> {
    let ts_stops: Vec<tiny_skia::GradientStop> = stops
        .iter()
        .map(|s| tiny_skia::GradientStop::new(s.offset as f32, to_color(&s.color)))
        .collect();
    tiny_skia::LinearGradient::new(
        Point::from_xy(start.x as f32, start.y as f32),
        Point::from_xy(end.x as f32, end.y as f32),
        ts_stops,
        SpreadMode::Pad,
        total,
    )
    .ok_or_else(|| {
        OfdError::Render(RenderError::BudgetExceeded(
            "degenerate linear gradient".to_string(),
        ))
    })
}

fn radial_shader(
    center: steleofd_common::Point,
    radius: f64,
    stops: &[GradientStop],
    total: Transform,
) -> Result<Shader<'static>> {
    let ts_stops: Vec<tiny_skia::GradientStop> = stops
        .iter()
        .map(|s| tiny_skia::GradientStop::new(s.offset as f32, to_color(&s.color)))
        .collect();
    tiny_skia::RadialGradient::new(
        Point::from_xy(center.x as f32, center.y as f32),
        Point::from_xy(center.x as f32, center.y as f32),
        radius as f32,
        ts_stops,
        SpreadMode::Pad,
        total,
    )
    .ok_or_else(|| {
        OfdError::Render(RenderError::BudgetExceeded(
            "degenerate radial gradient".to_string(),
        ))
    })
}

fn fill_with_shader(
    pixmap: &mut Pixmap,
    path: &Path,
    shader: Shader<'static>,
    clip: Option<&Mask>,
) -> Result<()> {
    let ts_path = to_ts_path(path, Transform::identity())?;
    let paint = TsPaint {
        shader,
        anti_alias: true,
        ..TsPaint::default()
    };
    pixmap.fill_path(
        &ts_path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        clip,
    );
    Ok(())
}
