//! Reducing a radial shading to the forms gradient backends can paint.
//! GB/T 33190 clause 8.3.4 draws a cone between two circles; a backend paints
//! one whose first stop is a point. Both are the same cone once its apex is known.

use steleofd_common::Point;

use crate::ops::{Color, GradientStop};

/// Two radii this close are the same circle for reduction purposes.
/// Coordinates arrive in millimetres, so a nanometre is noise.
const SAME_RADIUS: f64 = 1e-9;

/// A cone whose first stop sits at a point and whose last stop sits on a
/// circle: the shape a focal-point gradient paints.
#[derive(Debug, Clone, PartialEq)]
pub struct FocalCone {
    /// The cone's apex, where the first stop now sits.
    pub apex: Point,
    /// Centre of the circle the last stop sits on.
    pub circle: Point,
    /// Radius of that circle.
    pub radius: f64,
    /// Colour stops repositioned along the cone's axis.
    pub stops: Vec<GradientStop>,
}

/// What a radial shading reduces to on a backend that paints focal cones.
#[derive(Debug, Clone, PartialEq)]
pub enum RadialForm {
    /// A cone running from a point out to a circle.
    Cone(FocalCone),
    /// Both circles are the same size, so the cone has no apex: colour
    /// varies along the axis between the two centres, which a linear
    /// gradient paints exactly.
    Cylinder {
        /// Centre of the start circle.
        start: Point,
        /// Centre of the end circle.
        end: Point,
    },
    /// Neither circle has any extent, so the shading is one flat colour.
    Flat(Color),
}

/// Reduce the two circles of a radial shading into a paintable form.
/// The apex is where the radius interpolated between the circles reaches zero.
pub fn radial_form(
    center: Point,
    radius: f64,
    end: Point,
    end_radius: f64,
    stops: &[GradientStop],
) -> RadialForm {
    let flat = || stops.first().map(|s| s.color).unwrap_or(Color::BLACK);
    if radius <= SAME_RADIUS && end_radius <= SAME_RADIUS {
        return RadialForm::Flat(flat());
    }
    if (radius - end_radius).abs() <= SAME_RADIUS {
        // Equal circles put the apex at infinity, so colour varies along the
        // axis and nowhere else; with one centre there is no axis to vary
        // along and the shading is uniform.
        if center == end {
            return RadialForm::Flat(flat());
        }
        return RadialForm::Cylinder { start: center, end };
    }
    let growing = end_radius > radius;
    let (near, far) = if growing {
        (radius, end_radius)
    } else {
        (end_radius, radius)
    };
    // The apex sits where the interpolated radius vanishes: beyond the start
    // circle when the cone grows, beyond the end circle when it shrinks.
    let apex = lerp(center, end, radius / (radius - end_radius));
    let (circle, far_radius) = if growing {
        (end, end_radius)
    } else {
        (center, radius)
    };
    // A stop at `t` between the circles sits at this fraction of the way from
    // the apex to the far circle; the cone reaches the near circle at
    // `near / far`, which is where the first stop lands.
    let mut remapped: Vec<GradientStop> = stops
        .iter()
        .map(|s| {
            let between = if growing { s.offset } else { 1.0 - s.offset };
            GradientStop {
                offset: ((near + between * (far - near)) / far).clamp(0.0, 1.0),
                color: s.color,
            }
        })
        .collect();
    // A shrinking cone walks its circles in the opposite order, so the stops
    // come out reversed; backends read them ascending.
    remapped.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    RadialForm::Cone(FocalCone {
        apex,
        circle,
        radius: far_radius,
        stops: remapped,
    })
}

/// The point a fraction `t` of the way from `from` to `to`.
fn lerp(from: Point, to: Point, t: f64) -> Point {
    Point::new(from.x + t * (to.x - from.x), from.y + t * (to.y - from.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::Color;

    fn stop(offset: f64, color: Color) -> GradientStop {
        GradientStop { offset, color }
    }

    fn two_stops() -> Vec<GradientStop> {
        vec![stop(0.0, Color::WHITE), stop(1.0, Color::BLACK)]
    }

    #[test]
    fn a_point_start_circle_reduces_to_the_plain_radial() {
        // The form every writer emits: the clause's optional StartRadius
        // absent, so the first stop sits at the centre itself.
        let form = radial_form(
            Point::new(4.0, 4.0),
            0.0,
            Point::new(4.0, 4.0),
            5.0,
            &two_stops(),
        );
        let RadialForm::Cone(cone) = form else {
            panic!("expected a cone, got {form:?}");
        };
        assert_eq!(cone.apex, Point::new(4.0, 4.0));
        assert_eq!(cone.circle, Point::new(4.0, 4.0));
        assert!((cone.radius - 5.0).abs() < 1e-9);
        assert_eq!(cone.stops, two_stops(), "the stops travel unchanged");
    }

    #[test]
    fn a_growing_cone_puts_its_apex_before_the_start_circle() {
        // Circle 1 has radius 1 at the origin, circle 2 radius 3 at x=10, so
        // the cone's apex sits at x=-5 and circle 1 is one third of the way
        // out to circle 2.
        let form = radial_form(
            Point::new(0.0, 0.0),
            1.0,
            Point::new(10.0, 0.0),
            3.0,
            &two_stops(),
        );
        let RadialForm::Cone(cone) = form else {
            panic!("expected a cone, got {form:?}");
        };
        assert!((cone.apex.x - -5.0).abs() < 1e-9, "apex at {cone:?}");
        assert_eq!(cone.circle, Point::new(10.0, 0.0));
        assert!((cone.radius - 3.0).abs() < 1e-9);
        assert!((cone.stops[0].offset - 1.0 / 3.0).abs() < 1e-9);
        assert!((cone.stops[1].offset - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_shrinking_cone_reverses_its_stops() {
        // The same two circles with the radii swapped: the apex now sits
        // beyond circle 2 and the far circle is circle 1, so the stop order
        // comes out reversed.
        let form = radial_form(
            Point::new(0.0, 0.0),
            3.0,
            Point::new(10.0, 0.0),
            1.0,
            &two_stops(),
        );
        let RadialForm::Cone(cone) = form else {
            panic!("expected a cone, got {form:?}");
        };
        assert!((cone.apex.x - 15.0).abs() < 1e-9, "apex at {cone:?}");
        assert_eq!(cone.circle, Point::new(0.0, 0.0));
        assert!((cone.radius - 3.0).abs() < 1e-9);
        assert!((cone.stops[0].offset - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(cone.stops[0].color, Color::BLACK, "the colours reversed");
        assert!((cone.stops[1].offset - 1.0).abs() < 1e-9);
        assert_eq!(cone.stops[1].color, Color::WHITE);
    }

    #[test]
    fn equal_circles_reduce_to_a_cylinder() {
        let form = radial_form(
            Point::new(0.0, 0.0),
            4.0,
            Point::new(9.0, 0.0),
            4.0,
            &two_stops(),
        );
        assert_eq!(
            form,
            RadialForm::Cylinder {
                start: Point::new(0.0, 0.0),
                end: Point::new(9.0, 0.0),
            }
        );
    }

    #[test]
    fn one_centre_and_equal_circles_have_no_axis_to_vary_along() {
        let form = radial_form(
            Point::new(2.0, 2.0),
            4.0,
            Point::new(2.0, 2.0),
            4.0,
            &two_stops(),
        );
        assert_eq!(form, RadialForm::Flat(Color::WHITE));
    }

    #[test]
    fn circles_without_extent_are_one_flat_colour() {
        let form = radial_form(
            Point::new(1.0, 1.0),
            0.0,
            Point::new(6.0, 1.0),
            0.0,
            &two_stops(),
        );
        assert_eq!(form, RadialForm::Flat(Color::WHITE));
    }
}
