//! Mesh shaders: the Gouraud fill of clauses 8.3.4.4 and 8.3.4.5.

use super::*;

/// Paint a Gouraud mesh bounded by `path`. tiny-skia has no vertex-colour
/// shader, so each triangle's colours go per pixel into a layer the path then
/// masks: a pixel no triangle covers keeps that layer's clear colour.
pub(crate) fn paint_mesh(
    pixmap: &mut Pixmap,
    spec: &steleofd_display::MeshSpec,
    path: &tiny_skia::Path,
    clip: Option<&Mask>,
    total: Transform,
) {
    let width = pixmap.width();
    let height = pixmap.height();
    let Some(mut layer) = Pixmap::new(width, height) else {
        return;
    };
    // Colours are interpolated in page space, so each mesh vertex is mapped
    // through the same total transform the clip path was built with.
    let mapped: Vec<(f32, f32, [f32; 4])> = spec
        .points
        .iter()
        .map(|v| {
            let mut p = Point::from_xy(v.x as f32, v.y as f32);
            total.map_point(&mut p);
            let c = v.color;
            (
                p.x,
                p.y,
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ],
            )
        })
        .collect();
    let pixels = layer.pixels_mut();
    for tri in &spec.triangles {
        let (Some(a), Some(b), Some(c)) =
            (mapped.get(tri[0]), mapped.get(tri[1]), mapped.get(tri[2]))
        else {
            continue;
        };
        let (min_x, max_x, min_y, max_y) = bounds(a, b, c);
        let x0 = min_x.floor().max(0.0) as u32;
        let x1 = (max_x.ceil() as i64).clamp(0, width as i64) as u32;
        let y0 = min_y.floor().max(0.0) as u32;
        let y1 = (max_y.ceil() as i64).clamp(0, height as i64) as u32;
        let area = edge(a, b, &(c.0, c.1));
        if area == 0.0 {
            continue;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                let p = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = edge(b, c, &p);
                let w1 = edge(c, a, &p);
                let w2 = edge(a, b, &p);
                if !inside(w0, w1, w2, area) {
                    continue;
                }
                let (b0, b1, b2) = (w0 / area, w1 / area, w2 / area);
                let mut blended = [0.0f32; 4];
                for (i, out) in blended.iter_mut().enumerate() {
                    *out = b0 * a.2[i] + b1 * b.2[i] + b2 * c.2[i];
                }
                let idx = (y * width + x) as usize;
                // tiny-skia's Pixmap is premultiplied RGBA, so each channel is
                // scaled by alpha before it is written.
                let alpha = blended[3];
                let channel =
                    |v: f32| ((v * alpha * 255.0).round().clamp(0.0, alpha * 255.0)) as u8;
                if let Some(px) = PremultipliedColorU8::from_rgba(
                    channel(blended[0]),
                    channel(blended[1]),
                    channel(blended[2]),
                    (alpha * 255.0).round() as u8,
                ) {
                    pixels[idx] = px;
                }
            }
        }
    }
    // The path is in page space, so a mask built from it bounds the mesh.
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

/// Twice the signed area of a triangle, or a barycentric weight when the third
/// argument is the point being tested.
fn edge(a: &(f32, f32, [f32; 4]), b: &(f32, f32, [f32; 4]), c: &(f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Whether a point lies inside the triangle, allowing either winding.
fn inside(w0: f32, w1: f32, w2: f32, area: f32) -> bool {
    if area > 0.0 {
        w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
    } else {
        w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
    }
}

/// The pixel-space bounding box of a triangle.
fn bounds(
    a: &(f32, f32, [f32; 4]),
    b: &(f32, f32, [f32; 4]),
    c: &(f32, f32, [f32; 4]),
) -> (f32, f32, f32, f32) {
    (
        a.0.min(b.0).min(c.0),
        a.0.max(b.0).max(c.0),
        a.1.min(b.1).min(c.1),
        a.1.max(b.1).max(c.1),
    )
}
