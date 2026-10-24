//! Path representation and construction.
//! OFD `PathObject` uses abbreviated operators (`M` `L` `C` `Q` `A` `Z`); this
//! module normalizes them into explicit [`PathSegment`] values.

use serde::{Deserialize, Serialize};
use steleofd_common::Rect;

use crate::Affine;
use crate::arc::arc_cubics;

/// One cubic segment of a flattened arc: two control points and the endpoint.
pub type Cubic = ((f64, f64), (f64, f64), (f64, f64));

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

    /// Close every subpath that is still open. Abbreviated data often ends on
    /// a bare `C` with no coordinates to read: filling closes implicitly, but
    /// a mask built from the outline does not.
    pub fn closed(self) -> Self {
        let mut out = Vec::with_capacity(self.segments.len() + 1);
        let mut open = false;
        for s in self.segments {
            match s {
                PathSegment::MoveTo(..) => {
                    if open {
                        out.push(PathSegment::Close);
                    }
                    open = true;
                    out.push(s);
                }
                PathSegment::Close => {
                    open = false;
                    out.push(s);
                }
                _ => out.push(s),
            }
        }
        if open {
            out.push(PathSegment::Close);
        }
        Self {
            segments: out,
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
            PathSegment::Arc {
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                to,
            } => {
                if tight {
                    // An arc's endpoints say nothing about where it bulges, so
                    // the curve is sampled: four semicircle arcs put all four
                    // endpoints on the rim and would measure as a square.
                    for (c1, c2, end) in
                        arc_cubics(rx, ry, x_axis_rotation, large_arc, sweep, cur, to)
                    {
                        for i in 0..=16 {
                            let t = i as f64 / 16.0;
                            let p = cubic_at(cur, c1, c2, end, t);
                            acc(p.0, p.1);
                        }
                        cur = end;
                    }
                }
                cur = to;
                acc(to.0, to.1);
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

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn assert_rect_eq(actual: Rect, expected: Rect) {
        assert!(
            (actual.x - expected.x).abs() < EPS
                && (actual.y - expected.y).abs() < EPS
                && (actual.w - expected.w).abs() < EPS
                && (actual.h - expected.h).abs() < EPS,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn empty_path_is_empty() {
        let p = Path::new();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn builder_records_segments_in_order() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0).line_to(10.0, 0.0).close();
        });
        assert_eq!(p.len(), 3);
        assert!(matches!(p.segments[0], PathSegment::MoveTo(0.0, 0.0)));
        assert!(matches!(p.segments[1], PathSegment::LineTo(10.0, 0.0)));
        assert!(matches!(p.segments[2], PathSegment::Close));
    }

    #[test]
    fn rect_builds_a_closed_four_sided_subpath() {
        let p = Path::build(|b| {
            b.rect(1.0, 2.0, 3.0, 4.0);
        });
        // move + three lines + close
        assert_eq!(p.len(), 5);
        assert!(p.is_polygonal());
        assert_rect_eq(p.bounding_box(), Rect::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn bounding_box_of_polygon_matches_extents() {
        let p = Path::build(|b| {
            b.move_to(5.0, 5.0)
                .line_to(15.0, 5.0)
                .line_to(10.0, 20.0)
                .close();
        });
        assert_rect_eq(p.bounding_box(), Rect::new(5.0, 5.0, 10.0, 15.0));
    }

    #[test]
    fn bounding_box_includes_bezier_control_points() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .curve_to((10.0, 20.0), (30.0, 20.0), (40.0, 0.0));
        });
        // The loose box spans the control envelope, so it is wider than the curve.
        let loose = p.bounding_box();
        assert_rect_eq(loose, Rect::new(0.0, 0.0, 40.0, 20.0));
    }

    #[test]
    fn tight_bounding_box_is_no_larger_than_loose() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .curve_to((10.0, 20.0), (30.0, 20.0), (40.0, 0.0));
        });
        let loose = p.bounding_box();
        let tight = p.tight_bounding_box();
        assert!(
            tight.h <= loose.h + EPS,
            "tight {tight:?} vs loose {loose:?}"
        );
        assert!(
            tight.w <= loose.w + EPS,
            "tight {tight:?} vs loose {loose:?}"
        );
    }

    #[test]
    fn tight_bounding_box_of_flat_bezier_is_shallower_than_loose() {
        // A curve whose control points rise high but whose actual path stays low.
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .curve_to((0.0, 100.0), (10.0, 100.0), (10.0, 0.0));
        });
        let loose = p.bounding_box();
        let tight = p.tight_bounding_box();
        assert!(
            tight.h < loose.h,
            "tight {tight:?} should beat loose {loose:?}"
        );
    }

    #[test]
    fn empty_path_has_default_bounding_box() {
        let p = Path::new();
        assert_eq!(p.bounding_box(), Rect::default());
    }

    #[test]
    fn transformed_translates_every_segment() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0).line_to(1.0, 1.0);
        });
        let moved = p.transformed(&Affine::translate(10.0, 20.0));
        assert!(matches!(moved.segments[0], PathSegment::MoveTo(10.0, 20.0)));
        assert!(matches!(moved.segments[1], PathSegment::LineTo(11.0, 21.0)));
    }

    #[test]
    fn transformed_scales_bezier_control_points() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .curve_to((1.0, 2.0), (3.0, 4.0), (5.0, 6.0));
        });
        let scaled = p.transformed(&Affine::scale(2.0, 2.0));
        match scaled.segments[1] {
            PathSegment::CurveTo { c1, c2, to } => {
                assert!((c1.0 - 2.0).abs() < EPS && (c1.1 - 4.0).abs() < EPS);
                assert!((c2.0 - 6.0).abs() < EPS && (c2.1 - 8.0).abs() < EPS);
                assert!((to.0 - 10.0).abs() < EPS && (to.1 - 12.0).abs() < EPS);
            }
            ref other => panic!("expected CurveTo, got {other:?}"),
        }
    }

    #[test]
    fn transformed_scales_arc_radii_by_the_scale_factor() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0);
            b.segments.push(PathSegment::Arc {
                rx: 5.0,
                ry: 10.0,
                x_axis_rotation: 0.0,
                large_arc: false,
                sweep: true,
                to: (10.0, 0.0),
            });
        });
        let scaled = p.transformed(&Affine::scale(2.0, 2.0));
        match scaled.segments[1] {
            PathSegment::Arc { rx, ry, to, .. } => {
                assert!((rx - 10.0).abs() < EPS, "rx was {rx}");
                assert!((ry - 20.0).abs() < EPS, "ry was {ry}");
                assert!((to.0 - 20.0).abs() < EPS, "to.x was {}", to.0);
            }
            ref other => panic!("expected Arc, got {other:?}"),
        }
    }

    #[test]
    fn closed_path_is_polygonal_but_bezier_is_not() {
        let poly = Path::build(|b| {
            b.move_to(0.0, 0.0).line_to(1.0, 0.0).close();
        });
        assert!(poly.is_polygonal());

        let curved = Path::build(|b| {
            b.move_to(0.0, 0.0)
                .curve_to((1.0, 1.0), (2.0, 2.0), (3.0, 3.0));
        });
        assert!(!curved.is_polygonal());
    }

    #[test]
    fn quad_bounding_box_uses_its_control_point() {
        let p = Path::build(|b| {
            b.move_to(0.0, 0.0).quad_to((5.0, 10.0), (10.0, 0.0));
        });
        assert_rect_eq(p.bounding_box(), Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn transform_of_empty_path_stays_empty() {
        let p = Path::new().transformed(&Affine::scale(100.0, 100.0));
        assert!(p.is_empty());
    }
}
