//! Backend-agnostic replay of a page's op stream.
//! The display list keeps object-local geometry plus explicit transform ops,
//! so every backend replays one state machine instead of trusting coordinates.

use steleofd_common::{Point, Rect};
use steleofd_display::ops::{GradientStop, Paint, PaintRule, StrokeStyle};
use steleofd_display::{DrawOp, GlyphRun, ImageRef, PageDisplay};
use steleofd_error::{FormatError, OfdError, Result};
use steleofd_geometry::{Affine, Path};

/// A path with its paint, as the op stream describes it.
pub struct PathShape<'a> {
    /// Geometry in current-state coordinates.
    pub path: &'a Path,
    /// Fill paint; `None` leaves the interior untouched.
    pub fill: Option<&'a Paint>,
    /// Stroke paint; `None` leaves the outline untouched.
    pub stroke: Option<&'a Paint>,
    /// Stroke geometry parameters.
    pub stroke_style: &'a StrokeStyle,
    /// Rule deciding which regions the path covers.
    pub rule: PaintRule,
}

/// A gradient fill clipped to a path.
pub struct GradientShape<'a> {
    /// Colour stops in draw order.
    pub stops: &'a [GradientStop],
    /// Region the gradient is painted into.
    pub path: &'a Path,
}

/// Backend hooks for one page of a display list.
/// Coordinates arrive in millimetres in the coordinate system of the current
/// transform; a sink tracks that transform from the calls it receives.
pub trait PageSink {
    /// Push the graphics state.
    fn save(&mut self);

    /// Pop the graphics state.
    fn restore(&mut self);

    /// Concatenate a transform onto the current state.
    fn transform(&mut self, matrix: Affine);

    /// Intersect the clip with a path.
    fn clip(&mut self, path: &Path, rule: PaintRule);

    /// Fill and/or stroke a path.
    fn path(&mut self, shape: PathShape<'_>);

    /// Draw a glyph run.
    fn text(&mut self, run: &GlyphRun);

    /// Draw an image.
    fn image(&mut self, image: &ImageRef, dst: Rect, transform: Affine);

    /// Fill a path with a linear gradient.
    fn linear_gradient(&mut self, start: Point, end: Point, shape: GradientShape<'_>);

    /// Fill a path with a radial gradient.
    fn radial_gradient(&mut self, center: Point, radius: f64, shape: GradientShape<'_>);
}

/// Replay one page into `sink`, rejecting an unbalanced restore.
pub fn replay(page: &PageDisplay, sink: &mut impl PageSink) -> Result<()> {
    let mut depth = 0usize;
    for op in &page.ops {
        match op {
            DrawOp::Save => {
                depth += 1;
                sink.save();
            }
            DrawOp::Restore => {
                if depth == 0 {
                    return Err(OfdError::Format(FormatError::CircularReference {
                        chain: "restore without a matching save".into(),
                    }));
                }
                depth -= 1;
                sink.restore();
            }
            DrawOp::Transform(matrix) => sink.transform(*matrix),
            DrawOp::ClipPath { path, rule } => sink.clip(path, *rule),
            DrawOp::Path {
                path,
                fill,
                stroke,
                stroke_style,
                rule,
            } => sink.path(PathShape {
                path,
                fill: fill.as_ref(),
                stroke: stroke.as_ref(),
                stroke_style,
                rule: *rule,
            }),
            DrawOp::Text(run) => sink.text(run),
            DrawOp::Image {
                image,
                dst,
                transform,
            } => sink.image(image, *dst, *transform),
            DrawOp::LinearGradient {
                start,
                end,
                stops,
                path,
            } => sink.linear_gradient(*start, *end, GradientShape { stops, path }),
            DrawOp::RadialGradient {
                center,
                radius,
                stops,
                path,
            } => sink.radial_gradient(*center, *radius, GradientShape { stops, path }),
        }
    }
    Ok(())
}

