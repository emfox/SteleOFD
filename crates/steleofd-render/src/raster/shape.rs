//! Painting shapes: state, transforms, paints and geometry.

use super::*;

#[derive(Clone)]
pub(crate) struct State {
    pub(crate) ctm: Affine,
    pub(crate) clip: Option<Mask>,
}

/// The mask a clip intersects into, starting opaque on the first use and
/// reusing the current one so nested clips compose.
pub(crate) fn clip_mask(state: &State, width: u32, height: u32) -> Result<Mask> {
    state
        .clip
        .clone()
        .filter(|m| m.width() == width && m.height() == height)
        .or_else(|| {
            Mask::new(width, height).map(|mut m| {
                m.data_mut().fill(255);
                m
            })
        })
        .ok_or(OfdError::Render(RenderError::InvalidSize {
            w: width,
            h: height,
        }))
}

/// Per-glyph outlines in page pixel space, in draw order. Outlines are
/// normalized to 1/1000 em with y up, so scale before positioning. A code the
/// run's own face lacks comes from the fallback chain.
pub(crate) fn glyph_outlines(
    fonts: &steleofd_font::FontManager,
    run: &GlyphRun,
    face: Arc<steleofd_font::FontFace>,
    total: Transform,
) -> Result<Vec<tiny_skia::Path>> {
    let mut offset = 0.0f64;
    let mut paths = Vec::new();
    for (i, &gid) in run.glyphs.iter().enumerate() {
        if i > 0 {
            offset += run.advances.get(i - 1).copied().unwrap_or(0.0);
        }
        offset += run.letter_spacing;
        let Some((outline, _source)) = fonts.outline_for_code(&face, gid, run.codes_are_glyphs)
        else {
            continue;
        };
        let k = run.size.value() / 1000.0;
        let glyph = Affine::new(k, 0.0, 0.0, -k, 0.0, 0.0)
            .then(Affine::translate(offset, 0.0))
            .then(run.transform);
        paths.push(to_ts_path(&outline, total.pre_concat(to_ts(glyph)))?);
    }
    Ok(paths)
}

pub(crate) fn to_color(c: &Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

/// Page-space millimetres to pixmap pixels, the raster's origin included.
/// `then` applies `self` first, so the CTM acts in millimetres and the
/// pixel scale maps the result onto the bitmap.
pub(crate) fn total_transform(state: &State, origin: &Rect, dpi: f64) -> Transform {
    let s = dpi / 25.4;
    let mm_to_px = Affine::translate(-origin.x * s, -origin.y * s).then(Affine::scale(s, s));
    to_ts(state.ctm.then(mm_to_px))
}

/// Convert an affine matrix into a tiny-skia transform, pixel for pixel.
pub(crate) fn to_ts(a: Affine) -> Transform {
    Transform::from_row(
        a.a as f32, a.b as f32, a.c as f32, a.d as f32, a.e as f32, a.f as f32,
    )
}

pub(crate) fn fill_rule(rule: PaintRule) -> FillRule {
    match rule {
        PaintRule::NonZero => FillRule::Winding,
        PaintRule::EvenOdd => FillRule::EvenOdd,
    }
}

pub(crate) fn solid_paint(paint: &Paint) -> TsPaint<'static> {
    // Anything but a flat colour has no single value to paint here; a pattern
    // falls back to black so a malformed reference stays visible rather than
    // silently dropping the fill. Gradients never reach this function.
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

/// The paint for a fill or stroke, resolving a gradient to its shader. A
/// gradient is stated on the object itself, so this needs nothing from the
/// surrounding display list.
pub(crate) fn paint_for(paint: &Paint, total: Transform) -> Option<TsPaint<'static>> {
    match paint {
        Paint::Solid(_) => Some(solid_paint(paint)),
        Paint::LinearGradient(spec) => Some(shader_paint(linear_shader(
            spec.start,
            spec.end,
            &spec.stops,
            total,
        ))),
        Paint::RadialGradient(spec) => {
            let paint = match steleofd_display::radial_form(
                spec.center,
                spec.radius,
                spec.end,
                spec.end_radius,
                &spec.stops,
            ) {
                steleofd_display::RadialForm::Cone(cone) => {
                    shader_paint(focal_shader(&cone, total))
                }
                // Equal circles: the shading varies along the axis between the
                // centres, which a linear gradient paints.
                steleofd_display::RadialForm::Cylinder { start, end } => {
                    shader_paint(linear_shader(start, end, &spec.stops, total))
                }
                // Neither circle has extent: one flat colour keeps the region
                // painted instead of dropping it.
                steleofd_display::RadialForm::Flat(color) => TsPaint {
                    shader: Shader::SolidColor(to_color(&color)),
                    anti_alias: true,
                    ..TsPaint::default()
                },
            };
            Some(paint)
        }
        // A mesh is painted per pixel by `mesh::paint_mesh` and a pattern tiled
        // by `pattern::paint_pattern`; both need the pixmap, so neither is a
        // shader `paint_for` can build.
        Paint::Mesh(_) | Paint::Pattern(_) => None,
    }
}

pub(crate) fn shader_paint(shader: Shader<'static>) -> TsPaint<'static> {
    TsPaint {
        shader,
        anti_alias: true,
        ..TsPaint::default()
    }
}

pub(crate) fn to_ts_stroke(style: &StrokeStyle, state: &State, dpi: f64) -> Stroke {
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

pub(crate) fn push_segment(builder: &mut TsBuilder, seg: &PathSegment, cur: &mut (f64, f64)) {
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

pub(crate) fn to_ts_path(path: &Path, transform: Transform) -> Result<TsPath> {
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
pub type Cubic = ((f64, f64), (f64, f64), (f64, f64));

/// Convert one elliptical arc into cubic segments, following the same
/// endpoint-to-centre conversion the SVG standard prescribes.
/// Shared with the PDF writer so both backends flatten arcs identically.
#[allow(clippy::too_many_arguments)]
pub fn arc_cubics(
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
