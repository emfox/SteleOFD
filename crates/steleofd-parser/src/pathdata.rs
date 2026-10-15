//! OFD abbreviated path data to geometry paths.
//! The grammar is a flat token stream: a command letter followed by its
//! numeric operands, with whitespace (or nothing) between tokens.

use steleofd_error::{OfdError, Result, XmlError};
use steleofd_geometry::{Path, PathBuilder};

/// Parse OFD abbreviated path data into a [`Path`].
/// Unknown commands are rejected rather than skipped: silently dropping
/// geometry would produce output that renders but does not match the source.
pub fn parse_path(data: &str) -> Result<Path> {
    let tokens = tokenize(data)?;
    let mut failure: Option<OfdError> = None;
    let path = Path::build(|builder| {
        if let Err(e) = emit(&tokens, builder) {
            failure = Some(e);
        }
    });
    match failure {
        Some(e) => Err(e),
        None => Ok(path),
    }
}

fn invalid(raw: &str) -> OfdError {
    OfdError::Xml(XmlError::InvalidAttribute {
        element: "PathObject".to_string(),
        attr: "AbbreviatedData".to_string(),
        value: raw.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Cmd(char),
    Num(f64),
}

fn tokenize(data: &str) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = data.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_alphabetic() {
            chars.next();
            out.push(Token::Cmd(c));
        } else {
            let mut raw = String::new();
            while let Some(&c) = chars.peek() {
                // A sign starts a new number unless it is an exponent marker.
                let sign_break = (c == '-' || c == '+')
                    && !raw.is_empty()
                    && !raw.ends_with('e')
                    && !raw.ends_with('E');
                if !sign_break && (c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')) {
                    raw.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let value: f64 = raw.parse().map_err(|_| invalid(&raw))?;
            out.push(Token::Num(value));
        }
    }
    Ok(out)
}

fn expect_cmd(tokens: &[Token], i: &mut usize) -> Result<char> {
    match tokens.get(*i) {
        Some(Token::Cmd(c)) => {
            *i += 1;
            Ok(*c)
        }
        Some(Token::Num(n)) => Err(invalid(&n.to_string())),
        None => Err(invalid("truncated path data")),
    }
}

fn take(tokens: &[Token], i: &mut usize, n: usize) -> Result<Vec<f64>> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        match tokens.get(*i) {
            Some(Token::Num(v)) => {
                out.push(*v);
                *i += 1;
            }
            Some(Token::Cmd(c)) => return Err(invalid(&format!("expected number, found {c}"))),
            None => return Err(invalid("truncated path data")),
        }
    }
    Ok(out)
}

fn emit(tokens: &[Token], b: &mut PathBuilder) -> Result<()> {
    if tokens.is_empty() {
        return Ok(());
    }
    let mut i = 0usize;
    let mut cmd = expect_cmd(tokens, &mut i)?;
    if cmd != 'M' {
        return Err(invalid("path data must start with M"));
    }
    // The arc form needs the current point, which the builder does not expose.
    let mut cur = (0.0f64, 0.0f64);
    let mut subpath_start = (0.0f64, 0.0f64);

    while i < tokens.len() {
        if let Token::Cmd(c) = tokens[i] {
            i += 1;
            cmd = c;
            continue;
        }
        match cmd {
            'M' => {
                let v = take(tokens, &mut i, 2)?;
                b.move_to(v[0], v[1]);
                cur = (v[0], v[1]);
                subpath_start = cur;
                cmd = 'L';
            }
            'L' => {
                let v = take(tokens, &mut i, 2)?;
                b.line_to(v[0], v[1]);
                cur = (v[0], v[1]);
            }
            'C' => {
                let v = take(tokens, &mut i, 6)?;
                b.curve_to((v[0], v[1]), (v[2], v[3]), (v[4], v[5]));
                cur = (v[4], v[5]);
            }
            'Q' => {
                let v = take(tokens, &mut i, 4)?;
                b.quad_to((v[0], v[1]), (v[2], v[3]));
                cur = (v[2], v[3]);
            }
            // `B` is a cubic Bezier from the current point, so six numbers
            // where `C` carries the same six plus the repeated start. A
            // Chinese invoice draws its supervisory seal with it.
            'B' => {
                let v = take(tokens, &mut i, 6)?;
                b.curve_to(cur, (v[0], v[1]), (v[2], v[3]));
                cur = (v[4], v[5]);
            }
            'A' => {
                let v = take(tokens, &mut i, 7)?;
                let shape = ArcShape {
                    rx: v[0],
                    ry: v[1],
                    rot_deg: v[2],
                    large: v[3] != 0.0,
                    sweep: v[4] != 0.0,
                };
                arc_to(b, shape, cur, (v[5], v[6]))?;
                cur = (v[5], v[6]);
            }
            'Z' => {
                b.close();
                cur = subpath_start;
                // A command letter must follow; a number here is malformed.
                cmd = ' ';
            }
            other => return Err(invalid(&format!("unknown path command {other}"))),
        }
    }
    Ok(())
}

/// Operand bundle of one `A` command, in the order the standard writes them.
struct ArcShape {
    rx: f64,
    ry: f64,
    rot_deg: f64,
    large: bool,
    sweep: bool,
}

/// Append one elliptical arc as cubic curves, using the endpoint parametrization.
/// Follows the standard conversion shared with SVG: endpoints to centre, then
/// splitting the sweep into segments of at most a quarter turn each.
fn arc_to(b: &mut PathBuilder, shape: ArcShape, from: (f64, f64), to: (f64, f64)) -> Result<()> {
    let ArcShape {
        rx: rx_in,
        ry: ry_in,
        rot_deg,
        large,
        sweep,
    } = shape;
    let (x1, y1) = from;
    let (x2, y2) = to;
    if rx_in == 0.0 || ry_in == 0.0 {
        b.line_to(x2, y2);
        return Ok(());
    }
    let (mut rx, mut ry) = (rx_in.abs(), ry_in.abs());
    let phi = rot_deg.to_radians();
    let (cos_p, sin_p) = (phi.cos(), phi.sin());
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let x1p = cos_p * dx + sin_p * dy;
    let y1p = -sin_p * dx + cos_p * dy;

    // Radii too small for the endpoints are scaled up, as the standard requires.
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
        let dot = ux * vx + uy * vy;
        let lens = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = if lens == 0.0 {
            0.0
        } else {
            (dot / lens).clamp(-1.0, 1.0).acos()
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

    let segments = ((delta.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize).max(1);
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
    for _ in 0..segments {
        let t1 = t0 + step;
        let (p0x, p0y) = point(t0);
        let (p3x, p3y) = point(t1);
        let (d1x, d1y) = deriv(t0);
        let (d3x, d3y) = deriv(t1);
        b.curve_to(
            (p0x + kappa * d1x, p0y + kappa * d1y),
            (p3x - kappa * d3x, p3y - kappa * d3y),
            (p3x, p3y),
        );
        t0 = t1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_geometry::Affine;

    fn bbox_of(data: &str) -> steleofd_common::Rect {
        parse_path(data).expect("parse").bounding_box()
    }

    #[test]
    fn a_rectangle_round_trips_its_extent() {
        let bbox = bbox_of("M 10 10 L 30 10 L 30 30 L 10 30 Z");
        assert!((bbox.x - 10.0).abs() < 1e-9 && (bbox.y - 10.0).abs() < 1e-9);
        assert!((bbox.w - 20.0).abs() < 1e-9 && (bbox.h - 20.0).abs() < 1e-9);
    }

    #[test]
    fn move_to_continues_as_line_to() {
        // SVG-style implicit continuation: extra coordinate pairs after M
        // are treated as line segments, and OFD inherits that rule.
        let path = parse_path("M 0 0 10 0 10 10").expect("parse");
        assert!((path.bounding_box().w - 10.0).abs() < 1e-9);
        assert!((path.bounding_box().h - 10.0).abs() < 1e-9);
    }

    #[test]
    fn curves_contribute_their_control_points_to_the_bbox() {
        // The conservative bounding box includes control points, so it is
        // allowed to be larger than the tight hull of the drawn curve.
        let path = parse_path("M 0 0 C 10 -20 20 -20 30 0").expect("parse");
        let bbox = path.bounding_box();
        assert!((bbox.y + 20.0).abs() < 1e-9, "control points raise the top");
        assert!((bbox.w - 30.0).abs() < 1e-9);
    }

    #[test]
    fn a_quarter_arc_spans_a_square_bbox() {
        let path = parse_path("M 0 0 A 5 5 0 0 1 10 0").expect("parse");
        let bbox = path.bounding_box();
        assert!((bbox.w - 10.0).abs() < 1e-6, "got {bbox:?}");
        assert!((bbox.h - 5.0).abs() < 1e-6, "got {bbox:?}");
        assert!(!path.is_polygonal(), "an arc must not flatten to lines");
    }

    #[test]
    fn a_semicircle_arc_is_split_into_two_curves() {
        // Segments count the opening move, so a move plus two quarter arcs.
        let path = parse_path("M 0 0 A 5 5 0 0 1 0 10").expect("parse");
        assert_eq!(path.len(), 3, "180 degrees needs two quarter arcs");
        assert!(!path.is_polygonal());
    }

    #[test]
    fn a_degenerate_arc_degrades_to_a_line() {
        let path = parse_path("M 0 0 A 0 5 0 0 1 10 0").expect("parse");
        assert!(path.is_polygonal());
        assert!((path.bounding_box().w - 10.0).abs() < 1e-9);
    }

    #[test]
    fn close_returns_to_the_subpath_start() {
        let path = parse_path("M 10 10 L 20 10 L 20 20 Z M 2 2 L 4 4").expect("parse");
        let bbox = path.bounding_box();
        assert!(
            (bbox.x - 2.0).abs() < 1e-9,
            "the second subpath is leftmost"
        );
    }

    #[test]
    fn signed_and_scientific_numbers_are_accepted() {
        let bbox = bbox_of("M -1.5 0 L 1e1 0 L 10 2.5E+1 Z");
        assert!((bbox.x + 1.5).abs() < 1e-9);
        assert!((bbox.w - 11.5).abs() < 1e-9);
        assert!((bbox.h - 25.0).abs() < 1e-9);
    }

    #[test]
    fn an_unknown_command_is_rejected() {
        let err = parse_path("M 0 0 X 1 2").expect_err("X is not an OFD command");
        assert!(err.to_string().contains("AbbreviatedData"), "{err}");
    }

    #[test]
    fn truncated_operands_are_rejected() {
        assert!(parse_path("M 0 0 L 10").is_err());
        assert!(parse_path("L 1 1").is_err(), "must start with M");
        assert!(parse_path("M 0 0 L 10 10 20").is_err());
    }

    #[test]
    fn empty_data_yields_an_empty_path() {
        let path = parse_path("   ").expect("parse");
        assert!(path.is_empty());
    }

    #[test]
    fn arc_points_track_the_current_position() {
        // The arc endpoint form needs the current point; verify continuation
        // after an arc lands exactly on the requested endpoint.
        let path = parse_path("M 0 0 L 5 0 A 5 5 0 0 1 15 0 L 20 0").expect("parse");
        let bbox = path.bounding_box();
        assert!((bbox.w - 20.0).abs() < 1e-6, "got {bbox:?}");
    }

    #[test]
    fn transforms_map_the_parsed_geometry() {
        let path = parse_path("M 0 0 L 10 0 L 10 10 Z").expect("parse");
        let moved = path.transformed(&Affine::translate(5.0, 7.0));
        let bbox = moved.bounding_box();
        assert!((bbox.x - 5.0).abs() < 1e-9 && (bbox.y - 7.0).abs() < 1e-9);
    }
}
