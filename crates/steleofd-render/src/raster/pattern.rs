//! Pattern fills: the tiling of clause 8.3.5.

use super::*;

/// Paint a pattern across the boundary of `path`. The cell is flattened
/// content rather than an image, so it is rasterized once and the finished tile
/// copied across the area, the result masked to the path.
pub(crate) fn paint_pattern(
    pixmap: &mut Pixmap,
    spec: &steleofd_display::PatternSpec,
    path: &tiny_skia::Path,
    clip: Option<&Mask>,
    fonts: &FontManager,
    total: Transform,
) {
    let width = pixmap.width();
    let height = pixmap.height();
    let scale = scale_of(total);
    let step_x = (spec.step_x * scale) as f32;
    let step_y = (spec.step_y * scale) as f32;
    // A cell with no step would repeat in place forever, so leaving the region
    // unpainted is better than looping.
    if step_x.abs() < 1.0 || step_y.abs() < 1.0 {
        return;
    }
    let (Some(mut layer), Some(tile)) = (
        Pixmap::new(width, height),
        crate::raster::render_cell(spec, scale, fonts),
    ) else {
        return;
    };
    let mut origin = Point::from_xy(0.0, 0.0);
    total.map_point(&mut origin);
    // Cells start before the origin when the step does not divide it, so the
    // range runs from the first cell that can reach the raster to the last.
    let first_col = (origin.x / step_x).floor() as i64;
    let last_col = ((width as f32 - origin.x) / step_x).ceil() as i64;
    let first_row = (origin.y / step_y).floor() as i64;
    let last_row = ((height as f32 - origin.y) / step_y).ceil() as i64;
    let cell_w = (spec.width * scale) as f32;
    let cell_h = (spec.height * scale) as f32;
    for row in first_row..=last_row {
        for col in first_col..=last_col {
            let x = origin.x + step_x * col as f32;
            let y = origin.y + step_y * row as f32;
            let mirror = mirror_transform(spec.reflect, col, row, cell_w, cell_h);
            let place = mirror.post_concat(Transform::from_translate(x, y));
            layer.draw_pixmap(0, 0, tile.as_ref(), &PixmapPaint::default(), place, None);
        }
    }
    if let Some(mut mask) = Mask::new(width, height) {
        mask.fill_path(path, FillRule::Winding, true, Transform::identity());
        layer.apply_mask(&mask);
    }
    if let Some(outer) = clip {
        layer.apply_mask(outer);
    }
    pixmap.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

/// The mirror a cell's column and row call for, about the cell's own centre.
fn mirror_transform(
    reflect: steleofd_display::Reflect,
    col: i64,
    row: i64,
    cell_w: f32,
    cell_h: f32,
) -> Transform {
    let (mx, my) = reflect.mirrors(col, row);
    if !mx && !my {
        return Transform::identity();
    }
    let sx = if mx { -1.0 } else { 1.0 };
    let sy = if my { -1.0 } else { 1.0 };
    let flip = Transform::from_row(sx, 0.0, 0.0, sy, 0.0, 0.0);
    let shift =
        Transform::from_translate(if mx { cell_w } else { 0.0 }, if my { cell_h } else { 0.0 });
    flip.post_concat(shift)
}

/// The uniform scale a total transform applies, in pixels per millimetre.
fn scale_of(total: Transform) -> f64 {
    let mut p = Point::from_xy(1.0, 0.0);
    total.map_point(&mut p);
    let mut q = Point::from_xy(0.0, 1.0);
    total.map_point(&mut q);
    ((p.x.hypot(p.y) + q.x.hypot(q.y)) / 2.0) as f64
}
