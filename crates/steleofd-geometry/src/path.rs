//! Path representation and construction.
//! OFD `PathObject` uses abbreviated operators (`M` `L` `C` `Q` `A` `Z`); this
//! module normalizes them into explicit [`PathSegment`] values.

use serde::{Deserialize, Serialize};
use steleofd_common::Rect;

use crate::Affine;

/// One segment of a path.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PathSegment {
    /// Start a subpath at an absolute point in millimetres.
    MoveTo(f64, f64),
    /// Straight line to an absolute point.
    LineTo(f64, f64),
    /// Cubic Bezier curve.
    CurveTo {
        /// First control point.
        c1: (f64, f64),
        /// Second control point.
        c2: (f64, f64),
        /// End point.
        to: (f64, f64),
    },
    /// Quadratic Bezier curve, OFD operator `Q`.
    QuadTo {
        /// Control point.
        c: (f64, f64),
        /// End point.
        to: (f64, f64),
    },
    /// Elliptical arc, OFD operator `A`; backends convert it to Beziers.
    Arc {
        /// Radius along the ellipse x axis.
        rx: f64,
        /// Radius along the ellipse y axis.
        ry: f64,
        /// Rotation of the ellipse axes, in degrees.
        x_axis_rotation: f64,
        /// Whether the arc spans more than 180 degrees.
        large_arc: bool,
        /// Whether the arc is drawn clockwise.
        sweep: bool,
        /// End point.
        to: (f64, f64),
    },
    /// Close the current subpath.
    Close,
}

/// A complete path.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Path {
    /// Segments in draw order.
    pub segments: Vec<PathSegment>,
    bbox: Option<Rect>,
}

impl Path {
    /// An empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a path through a closure.
    pub fn build(f: impl FnOnce(&mut PathBuilder)) -> Self {
        let mut b = PathBuilder::new();
        f(&mut b);
        b.finish()
    }

    /// Number of segments.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Conservative bounding box.
    /// Bezier control points form an envelope larger than the true box but much
    /// cheaper, fine for culling; use [`Path::tight_bounding_box`] for accuracy.
    pub fn bounding_box(&self) -> Rect {
        if let Some(b) = self.bbox {
            return b;
        }
        let r = bbox_of_segments(&self.segments, false);
        if r.w <= 0.0 && r.h <= 0.0 {
            Rect::default()
        } else {
            r
        }
    }

    /// Accurate bounding box, sampling curve segments.
    pub fn tight_bounding_box(&self) -> Rect {
        bbox_of_segments(&self.segments, true)
    }

    /// Apply a transform, producing a new path.
    pub fn transformed(&self, t: &Affine) -> Self {
        let segments = self
            .segments
            .iter()
            .map(|s| transform_segment(s, t))
            .collect::<Vec<_>>();
        Self {
            segments,
            bbox: None,
        }
    }

    /// Whether the path is made only of straight segments.
    pub fn is_polygonal(&self) -> bool {
        self.segments.iter().all(|s| {
            matches!(
                s,
                PathSegment::MoveTo(..) | PathSegment::LineTo(..) | PathSegment::Close
            )
        })
    }
}

/// Incremental path construction.
#[derive(Debug, Default)]
pub struct PathBuilder {
    segments: Vec<PathSegment>,
}

impl PathBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a subpath.
    pub fn move_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.segments.push(PathSegment::MoveTo(x, y));
        self
    }

    /// Add a straight segment.
    pub fn line_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.segments.push(PathSegment::LineTo(x, y));
        self
    }

    /// Add a cubic Bezier segment.
    pub fn curve_to(&mut self, c1: (f64, f64), c2: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.segments.push(PathSegment::CurveTo { c1, c2, to });
        self
    }

    /// Add a quadratic Bezier segment.
    pub fn quad_to(&mut self, c: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.segments.push(PathSegment::QuadTo { c, to });
        self
    }

    /// Add a closed rectangle.
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close()
    }

    /// Close the current subpath.
    pub fn close(&mut self) -> &mut Self {
        self.segments.push(PathSegment::Close);
        self
    }

    /// Finish and produce the path.
    pub fn finish(self) -> Path {
        Path {
            segments: self.segments,
            bbox: None,
        }
    }
}

fn transform_segment(s: &PathSegment, t: &Affine) -> PathSegment {
    match *s {
        PathSegment::MoveTo(x, y) => {
            let (x, y) = t.apply(x, y);
            PathSegment::MoveTo(x, y)
        }
        PathSegment::LineTo(x, y) => {
            let (x, y) = t.apply(x, y);
            PathSegment::LineTo(x, y)
        }
        PathSegment::CurveTo { c1, c2, to } => {
            let (x1, y1) = t.apply(c1.0, c1.1);
            let (x2, y2) = t.apply(c2.0, c2.1);
            let (xt, yt) = t.apply(to.0, to.1);
            PathSegment::CurveTo {
                c1: (x1, y1),
                c2: (x2, y2),
                to: (xt, yt),
            }
        }
        PathSegment::QuadTo { c, to } => {
            let (xc, yc) = t.apply(c.0, c.1);
            let (xt, yt) = t.apply(to.0, to.1);
            PathSegment::QuadTo {
                c: (xc, yc),
                to: (xt, yt),
            }
        }
        PathSegment::Arc {
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            to,
        } => {
            let (xt, yt) = t.apply(to.0, to.1);
            let scale = t.scale_factor();
            PathSegment::Arc {
                rx: rx * scale,
                ry: ry * scale,
                x_axis_rotation,
                large_arc,
                sweep,
                to: (xt, yt),
            }
        }
        PathSegment::Close => PathSegment::Close,
    }
}

fn bbox_of_segments(segments: &[PathSegment], tight: bool) -> Rect {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut cur = (0.0f64, 0.0f64);

    let mut acc = |x: f64, y: f64| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };

    for seg in segments {
        match *seg {
            PathSegment::MoveTo(x, y) | PathSegment::LineTo(x, y) => {
                cur = (x, y);
                acc(x, y);
            }
            PathSegment::CurveTo { c1, c2, to } => {
                if tight {
                    for i in 0..=16 {
                        let t = i as f64 / 16.0;
                        let p = cubic_at(cur, c1, c2, to, t);
                        acc(p.0, p.1);
                    }
                } else {
                    acc(c1.0, c1.1);
                    acc(c2.0, c2.1);
                }
                cur = to;
                acc(to.0, to.1);
            }
            PathSegment::QuadTo { c, to } => {
                if tight {
                    for i in 0..=16 {
                        let t = i as f64 / 16.0;
                        let p = quad_at(cur, c, to, t);
                        acc(p.0, p.1);
                    }
                } else {
                    acc(c.0, c.1);
                }
                cur = to;
                acc(to.0, to.1);
            }
            PathSegment::Arc { to, .. } => {
                acc(to.0, to.1);
                cur = to;
            }
            PathSegment::Close => {}
        }
    }

    if !min_x.is_finite() {
        return Rect::default();
    }
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn cubic_at(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

fn quad_at(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    let a = mt * mt;
    let b = 2.0 * mt * t;
    let c = t * t;
    (
        a * p0.0 + b * p1.0 + c * p2.0,
        a * p0.1 + b * p1.1 + c * p2.1,
    )
}
