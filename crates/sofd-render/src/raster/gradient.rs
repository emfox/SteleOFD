//! Gradient shaders.

use super::*;

/// Paint a linear gradient, degrading to the first stop when the axis
/// vanishes: one fill refusing to place must not fail the whole page.
pub(crate) fn linear_shader(
    start: sofd_common::Point,
    end: sofd_common::Point,
    stops: &[GradientStop],
    total: Transform,
) -> Shader<'static> {
    let ts_stops: Vec<tiny_skia::GradientStop> = stops
        .iter()
        .map(|s| tiny_skia::GradientStop::new(s.offset as f32, to_color(&s.color)))
        .collect();
    let flat = Shader::SolidColor(
        stops
            .first()
            .map(|s| to_color(&s.color))
            .unwrap_or(tiny_skia::Color::BLACK),
    );
    tiny_skia::LinearGradient::new(
        Point::from_xy(start.x as f32, start.y as f32),
        Point::from_xy(end.x as f32, end.y as f32),
        ts_stops,
        SpreadMode::Pad,
        total,
    )
    .unwrap_or(flat)
}

/// Paint a reduced cone: a point at the apex, a circle at the far end.
/// A gradient tiny-skia refuses to place degrades to the first stop's colour:
/// a shading is one fill among many, and losing it must not lose the page.
pub(crate) fn focal_shader(cone: &sofd_display::FocalCone, total: Transform) -> Shader<'static> {
    let ts_stops: Vec<tiny_skia::GradientStop> = cone
        .stops
        .iter()
        .map(|s| tiny_skia::GradientStop::new(s.offset as f32, to_color(&s.color)))
        .collect();
    let flat = Shader::SolidColor(
        cone.stops
            .first()
            .map(|s| to_color(&s.color))
            .unwrap_or(tiny_skia::Color::BLACK),
    );
    tiny_skia::RadialGradient::new(
        Point::from_xy(cone.apex.x as f32, cone.apex.y as f32),
        Point::from_xy(cone.circle.x as f32, cone.circle.y as f32),
        cone.radius as f32,
        ts_stops,
        SpreadMode::Pad,
        total,
    )
    .unwrap_or(flat)
}
