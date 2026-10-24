//! Elliptical arcs as the cubic segments every backend draws them with.

use crate::Cubic;

/// Convert one elliptical arc into cubic segments, following the
/// endpoint-to-centre conversion the SVG standard prescribes. Every backend and
/// the bounding box read arcs through this, so all three agree on the curve.
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
