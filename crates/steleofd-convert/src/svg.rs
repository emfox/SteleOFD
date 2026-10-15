//! SVG backend.
//! Millimetres double as user units, so the viewBox carries the page box and
//! no coordinate scaling happens anywhere in the output.

use std::fmt::Write as _;

use steleofd_common::{Point, Rect};
use steleofd_display::ops::{
    Color, GradientStop, LineCap, LineJoin, Paint, PaintRule, StrokeStyle,
};
use steleofd_display::{GlyphRun, ImageCodec, ImageRef};
use steleofd_error::Result;
use steleofd_font::FontManager;
use steleofd_geometry::{Affine, Path, PathSegment};

use crate::base64;
use crate::replay::{GradientShape, PageSink, PathShape, replay};
use crate::{ConversionStats, TextExport};

/// Options for SVG export.
#[derive(Debug, Clone, Default)]
pub struct SvgOptions<'a> {
    /// How glyph runs become SVG content.
    pub text: TextExport,
    /// Font source for outlined text; without one, runs fall back to live text.
    pub fonts: Option<&'a FontManager>,
}

/// Builds one SVG page by replaying a display list into it.
pub struct SvgSink<'a> {
    body: String,
    page_box: Rect,
    text: TextExport,
    fonts: Option<&'a FontManager>,
    ctm: Affine,
    stack: Vec<Affine>,
    levels: Vec<usize>,
    depth: usize,
    linear: Vec<String>,
    radial: Vec<String>,
    seq: usize,
    stats: ConversionStats,
}

impl<'a> SvgSink<'a> {
    /// Start a page of the given size.
    pub fn new(page_box: Rect, options: &SvgOptions<'a>) -> Self {
        Self {
            // The root group lets a transform at page level nest somewhere.
            body: String::from("  <g>\n"),
            page_box,
            text: options.text,
            fonts: options.fonts,
            ctm: Affine::IDENTITY,
            stack: Vec::new(),
            levels: vec![0],
            depth: 2,
            linear: Vec::new(),
            radial: Vec::new(),
            seq: 0,
            stats: ConversionStats::default(),
        }
    }

    /// Finish the page and return the document plus what it contains.
    pub fn finish(mut self) -> (String, ConversionStats) {
        self.close_levels();
        // Closing the root group lands back inside the svg element.
        self.depth = 0;
        self.line("</svg>".to_string());
        let mut doc = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        let (x, y, w, h) = (
            self.page_box.x,
            self.page_box.y,
            self.page_box.w,
            self.page_box.h,
        );
        let _ = write!(
            doc,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" \
             width=\"{w}mm\" height=\"{h}mm\" viewBox=\"{x} {y} {w} {h}\">"
        );
        doc.push('\n');
        doc.push_str(&self.body);
        (doc, self.stats)
    }

    /// What the page contained, for callers that report conversion quality.
    pub fn stats(&self) -> &ConversionStats {
        &self.stats
    }

    fn line(&mut self, text: String) {
        for _ in 0..self.depth {
            self.body.push_str("  ");
        }
        self.body.push_str(&text);
        self.body.push('\n');
    }

    /// Close every group still open, which an unbalanced save would leave.
    fn close_levels(&mut self) {
        while let Some(open) = self.levels.pop() {
            for _ in 0..=open {
                self.depth -= 1;
                self.line("</g>".to_string());
            }
        }
    }

    /// Open a group that the current level's restore will close.
    fn open_extra(&mut self, attrs: &str) {
        if attrs.is_empty() {
            self.line("<g>".to_string());
        } else {
            self.line(format!("<g {attrs}>"));
        }
        self.depth += 1;
        if let Some(top) = self.levels.last_mut() {
            *top += 1;
        }
    }

    fn paint_ref(&self, paint: &Paint) -> String {
        match paint {
            Paint::Solid(c) => hex(c),
            Paint::LinearGradientRef(i) => match self.linear.get(*i) {
                Some(id) => format!("url(#{id})"),
                None => "#000000".to_string(),
            },
            Paint::RadialGradientRef(i) => match self.radial.get(*i) {
                Some(id) => format!("url(#{id})"),
                None => "#000000".to_string(),
            },
            Paint::Pattern(_) => "#000000".to_string(),
        }
    }
}

impl PageSink for SvgSink<'_> {
    fn save(&mut self) {
        // The group this opens is closed by the matching restore, not counted
        // as an extra of the enclosing level.
        self.line("<g>".to_string());
        self.depth += 1;
        self.stack.push(self.ctm);
        self.levels.push(0);
    }

    fn restore(&mut self) {
        self.ctm = self.stack.pop().unwrap_or(Affine::IDENTITY);
        // The root level is implicit and closed by `finish`; replay refuses an
        // unbalanced restore before it reaches here, so this only guards
        // direct use of the sink.
        let open = if self.levels.len() > 1 {
            self.levels.pop().unwrap_or(0)
        } else {
            0
        };
        for _ in 0..=open {
            self.depth -= 1;
            self.line("</g>".to_string());
        }
    }

    fn transform(&mut self, matrix: Affine) {
        self.ctm = self.ctm.then(matrix);
        self.open_extra(&format!("transform=\"matrix({})\"", matrix_attr(matrix)));
    }

    fn clip(&mut self, path: &Path, rule: PaintRule) {
        self.seq += 1;
        let id = format!("clip{}", self.seq);
        let rule_attr = match rule {
            PaintRule::NonZero => String::new(),
            PaintRule::EvenOdd => " clip-rule=\"evenodd\"".to_string(),
        };
        self.line(format!(
            "<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\"{rule_attr}/></clipPath>",
            path_data(path)
        ));
        self.open_extra(&format!("clip-path=\"url(#{id})\""));
    }

    fn path(&mut self, shape: PathShape<'_>) {
        let mut attrs = format!("d=\"{}\"", path_data(shape.path));
        match shape.fill {
            Some(paint) => {
                let _ = write!(attrs, " fill=\"{}\"", self.paint_ref(paint));
                if let Paint::Solid(c) = paint {
                    if c.a != 255 {
                        let _ = write!(attrs, " fill-opacity=\"{}\"", opacity(c));
                    }
                }
            }
            None => attrs.push_str(" fill=\"none\""),
        }
        if shape.rule == PaintRule::EvenOdd {
            attrs.push_str(" fill-rule=\"evenodd\"");
        }
        match shape.stroke {
            Some(paint) => {
                let _ = write!(attrs, " stroke=\"{}\"", self.paint_ref(paint));
                if let Paint::Solid(c) = paint {
                    if c.a != 255 {
                        let _ = write!(attrs, " stroke-opacity=\"{}\"", opacity(c));
                    }
                }
                attrs.push_str(&stroke_attrs(shape.stroke_style));
            }
            None => attrs.push_str(" stroke=\"none\""),
        }
        self.line(format!("<path {attrs}/>"));
    }

    fn text(&mut self, run: &GlyphRun) {
        if run.glyphs.is_empty() {
            return;
        }
        let outlined = match self.text {
            TextExport::Outline => self.outline_text(run),
            TextExport::LiveText => false,
        };
        if !outlined {
            self.live_text(run);
        }
    }

    fn image(&mut self, image: &ImageRef, dst: Rect, transform: Affine) {
        let Some(href) = data_uri(image) else {
            self.stats.images_skipped += 1;
            return;
        };
        self.stats.images += 1;
        let (iw, ih) = image.pixel_size;
        let (iw, ih) = (iw.max(1) as f64, ih.max(1) as f64);
        let pixel_to_page = Affine::new(dst.w / iw, 0.0, 0.0, dst.h / ih, dst.x, dst.y);
        let place = transform.then(pixel_to_page);
        // The element spans the whole bitmap, so the rectangle form needs the
        // scaled pixel extent; anything mirrored or rotated takes the matrix
        // form, where the element keeps its natural pixel size.
        if place.b.abs() < 1e-9 && place.c.abs() < 1e-9 && place.a > 0.0 && place.d > 0.0 {
            self.line(format!(
                "<image href=\"{href}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                num(place.e),
                num(place.f),
                num(place.a * iw),
                num(place.d * ih)
            ));
        } else {
            self.line(format!(
                "<image href=\"{href}\" width=\"{}\" height=\"{}\" transform=\"matrix({})\"/>",
                num(iw),
                num(ih),
                matrix_attr(place)
            ));
        }
    }

    fn linear_gradient(&mut self, start: Point, end: Point, shape: GradientShape<'_>) {
        if shape.stops.is_empty() {
            return;
        }
        self.seq += 1;
        let id = format!("grad{}", self.seq);
        self.linear.push(id.clone());
        self.stats.gradients += 1;
        self.line(format!(
            "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{}</linearGradient>",
            num(start.x),
            num(start.y),
            num(end.x),
            num(end.y),
            stops_markup(shape.stops)
        ));
        self.fill_with_gradient(shape.path, &id, PaintRule::NonZero);
    }

    fn radial_gradient(&mut self, center: Point, radius: f64, shape: GradientShape<'_>) {
        if shape.stops.is_empty() {
            return;
        }
        self.seq += 1;
        let id = format!("grad{}", self.seq);
        self.radial.push(id.clone());
        self.stats.gradients += 1;
        self.line(format!(
            "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\">{}</radialGradient>",
            num(center.x),
            num(center.y),
            num(radius.max(0.0)),
            stops_markup(shape.stops)
        ));
        self.fill_with_gradient(shape.path, &id, PaintRule::NonZero);
    }
}

impl SvgSink<'_> {
    /// Outline every glyph into vector paths, reporting whether any was drawn.
    fn outline_text(&mut self, run: &GlyphRun) -> bool {
        let Some(fonts) = self.fonts else {
            return false;
        };
        let Ok(face) = fonts.face(run.font) else {
            return false;
        };
        let Some(fill) = &run.fill else {
            return false;
        };
        let k = run.size.value() / 1000.0;
        let mut offset = 0.0;
        let mut drawn = 0usize;
        for (i, &gid) in run.glyphs.iter().enumerate() {
            if i > 0 {
                offset += run.advances.get(i - 1).copied().unwrap_or(0.0);
            }
            offset += run.letter_spacing;
            let Some(outline) = face.outline(gid) else {
                continue;
            };
            // Outlines are normalized to 1/1000 em with y pointing up, so the
            // glyph matrix scales by size/1000 and flips the y axis.
            let glyph = run
                .transform
                .then(Affine::translate(offset, 0.0))
                .then(Affine::new(k, 0.0, 0.0, -k, 0.0, 0.0));
            let path = outline.transformed(&glyph);
            let mut attrs = format!(
                "d=\"{}\" fill=\"{}\"",
                path_data(&path),
                self.paint_ref(fill)
            );
            if let Paint::Solid(c) = fill {
                if c.a != 255 {
                    let _ = write!(attrs, " fill-opacity=\"{}\"", opacity(c));
                }
            }
            self.line(format!("<path {attrs}/>"));
            drawn += 1;
        }
        if drawn > 0 {
            self.stats.outlined_glyphs += drawn;
            true
        } else {
            false
        }
    }

    /// Emit the run's Unicode text and let the viewer pick a font.
    fn live_text(&mut self, run: &GlyphRun) {
        let Some(text) = run.text.as_deref().filter(|t| !t.is_empty()) else {
            self.stats.dropped_runs += 1;
            return;
        };
        self.stats.live_text_runs += 1;
        let placement = if run.transform.is_translation_only() {
            format!(
                "x=\"{}\" y=\"{}\"",
                num(run.transform.e),
                num(run.transform.f)
            )
        } else {
            format!("transform=\"matrix({})\"", matrix_attr(run.transform))
        };
        let fill = match &run.fill {
            Some(paint) => self.paint_ref(paint),
            None => "#000000".to_string(),
        };
        self.line(format!(
            "<text {placement} font-size=\"{}\" fill=\"{fill}\">{}</text>",
            num(run.size.value()),
            escape_text(text)
        ));
    }

    /// Paint a path with a gradient, clipped to that path by construction.
    fn fill_with_gradient(&mut self, path: &Path, id: &str, rule: PaintRule) {
        let rule_attr = match rule {
            PaintRule::NonZero => String::new(),
            PaintRule::EvenOdd => " clip-rule=\"evenodd\"".to_string(),
        };
        self.seq += 1;
        let clip = format!("clip{}", self.seq);
        self.line(format!(
            "<clipPath id=\"{clip}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\"{rule_attr}/></clipPath>",
            path_data(path)
        ));
        self.line(format!(
            "<path d=\"{}\" fill=\"url(#{id})\" clip-path=\"url(#{clip})\"/>",
            path_data(path)
        ));
    }
}

/// `#rrggbb` for a colour.
fn hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Alpha as a `0..=1` fraction.
fn opacity(c: &Color) -> String {
    num(c.a as f64 / 255.0)
}

/// Stroke presentation attributes, in millimetres.
fn stroke_attrs(style: &StrokeStyle) -> String {
    let mut attrs = format!(" stroke-width=\"{}\"", num(style.width.max(0.0)));
    if !style.dash.is_empty() {
        let pattern: Vec<String> = style.dash.iter().map(|d| num(*d)).collect();
        let _ = write!(
            attrs,
            " stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\"",
            pattern.join(" "),
            num(style.dash_offset)
        );
    }
    let cap = match style.cap {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    };
    let join = match style.join {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    };
    let _ = write!(
        attrs,
        " stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{}\"",
        num(style.miter_limit)
    );
    attrs
}

/// `<stop>` children of a gradient element.
fn stops_markup(stops: &[GradientStop]) -> String {
    let mut out = String::new();
    for stop in stops {
        let _ = write!(
            out,
            "<stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
            num(stop.offset.clamp(0.0, 1.0)),
            hex(&stop.color),
            opacity(&stop.color)
        );
    }
    out
}

/// SVG path data, millimetre coordinates written verbatim.
fn path_data(path: &Path) -> String {
    let mut d = String::new();
    for seg in &path.segments {
        match seg {
            PathSegment::MoveTo(x, y) => {
                let _ = write!(d, "M{} {}", num(*x), num(*y));
            }
            PathSegment::LineTo(x, y) => {
                let _ = write!(d, "L{} {}", num(*x), num(*y));
            }
            PathSegment::CurveTo { c1, c2, to } => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    num(c1.0),
                    num(c1.1),
                    num(c2.0),
                    num(c2.1),
                    num(to.0),
                    num(to.1)
                );
            }
            PathSegment::QuadTo { c, to } => {
                let _ = write!(d, "Q{} {} {} {}", num(c.0), num(c.1), num(to.0), num(to.1));
            }
            PathSegment::Arc {
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                to,
            } => {
                // OFD path data follows SVG arc semantics in the same y-down
                // space, so the flags carry over unchanged.
                let _ = write!(
                    d,
                    "A{} {} {} {} {} {} {}",
                    num(*rx),
                    num(*ry),
                    num(*x_axis_rotation),
                    u8::from(*large_arc),
                    u8::from(*sweep),
                    num(to.0),
                    num(to.1)
                );
            }
            PathSegment::Close => d.push('Z'),
        }
        d.push(' ');
    }
    d.truncate(d.trim_end().len());
    d
}

/// Matrix operands for an SVG `matrix()` transform.
fn matrix_attr(m: Affine) -> String {
    format!(
        "{} {} {} {} {} {}",
        num(m.a),
        num(m.b),
        num(m.c),
        num(m.d),
        num(m.e),
        num(m.f)
    )
}

/// Escape text content for XML.
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Embed image bytes as a data URI, or `None` for codecs the web rejects.
fn data_uri(image: &ImageRef) -> Option<String> {
    if image.data.is_empty() {
        return None;
    }
    let mime = match image.codec {
        ImageCodec::Jpeg => "image/jpeg",
        ImageCodec::Png => "image/png",
        ImageCodec::Tiff => "image/tiff",
        ImageCodec::Bmp => "image/bmp",
        ImageCodec::Jbig2 | ImageCodec::Raw => return None,
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::encode(&image.data)
    ))
}

/// Compact number: at most four decimals, no trailing zeros, no negative zero.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let mut s = format!("{v:.4}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Convert one page into a standalone SVG document.
pub fn page_to_svg(
    page: &steleofd_display::PageDisplay,
    options: &SvgOptions<'_>,
) -> Result<String> {
    let mut sink = SvgSink::new(page.page_box, options);
    replay(page, &mut sink)?;
    Ok(sink.finish().0)
}

/// Convert a whole display list, one SVG document per page.
pub fn pages_to_svg(
    list: &steleofd_display::DisplayList,
    options: &SvgOptions<'_>,
) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(list.pages.len());
    for page in &list.pages {
        out.push(page_to_svg(page, options)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_common::Millimeter;
    use steleofd_display::DrawOp;
    use steleofd_display::ops::Paint;
    use steleofd_font::FontId;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
        Path::build(|b| {
            b.rect(x, y, w, h);
        })
    }

    fn page(ops: Vec<DrawOp>) -> steleofd_display::PageDisplay {
        steleofd_display::PageDisplay {
            index: 0,
            page_box: Rect::A4_PORTRAIT,
            ops,
            content_bbox: Rect::EMPTY,
        }
    }

    #[test]
    fn the_root_element_carries_the_page_box_in_millimetres() {
        let svg = page_to_svg(&page(Vec::new()), &SvgOptions::default()).expect("convert");
        assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(svg.contains("width=\"210mm\" height=\"297mm\""));
        assert!(svg.contains("viewBox=\"0 0 210 297\""));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn a_rect_becomes_one_path_with_its_paint() {
        let p = page(vec![DrawOp::Path {
            path: rect(10.0, 20.0, 30.0, 40.0),
            fill: Some(Paint::Solid(Color::rgba(255, 0, 0, 128))),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<path d=\"M10 20 L40 20 L40 60 L10 60 Z\" fill=\"#ff0000\" fill-opacity=\"0.502\" stroke=\"none\"/>"));
    }

    #[test]
    fn a_path_without_fill_says_so_explicitly() {
        let p = page(vec![DrawOp::Path {
            path: rect(0.0, 0.0, 1.0, 1.0),
            fill: None,
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("fill=\"none\""));
        assert!(svg.contains("stroke=\"none\""));
    }

    #[test]
    fn the_even_odd_rule_reaches_the_fill_rule_attribute() {
        let p = page(vec![DrawOp::Path {
            path: rect(0.0, 0.0, 1.0, 1.0),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::EvenOdd,
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("fill-rule=\"evenodd\""));
    }

    #[test]
    fn stroke_parameters_carry_over_in_millimetres() {
        let style = StrokeStyle {
            width: 0.5,
            dash: vec![3.0, 1.0],
            dash_offset: 0.25,
            cap: LineCap::Round,
            join: LineJoin::Bevel,
            miter_limit: 2.0,
        };
        let p = page(vec![DrawOp::Path {
            path: rect(0.0, 0.0, 1.0, 1.0),
            fill: None,
            stroke: Some(Paint::Solid(Color::BLACK)),
            stroke_style: style,
            rule: PaintRule::NonZero,
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("stroke-width=\"0.5\""));
        assert!(svg.contains("stroke-dasharray=\"3 1\""));
        assert!(svg.contains("stroke-dashoffset=\"0.25\""));
        assert!(svg.contains("stroke-linecap=\"round\""));
        assert!(svg.contains("stroke-linejoin=\"bevel\""));
        assert!(svg.contains("stroke-miterlimit=\"2\""));
    }

    #[test]
    fn transforms_nest_as_groups_that_restore_closes() {
        let p = page(vec![
            DrawOp::Save,
            DrawOp::Transform(Affine::translate(5.0, 6.0)),
            DrawOp::Restore,
        ]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<g transform=\"matrix(1 0 0 1 5 6)\">"));
        // The group count balances: every opened group is closed again.
        assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count());
    }

    #[test]
    fn a_clip_emits_a_definition_and_a_referencing_group() {
        let p = page(vec![
            DrawOp::Save,
            DrawOp::ClipPath {
                path: rect(0.0, 0.0, 10.0, 10.0),
                rule: PaintRule::EvenOdd,
            },
            DrawOp::Restore,
        ]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<clipPath id=\"clip1\" clipPathUnits=\"userSpaceOnUse\">"));
        assert!(svg.contains("clip-rule=\"evenodd\""));
        assert!(svg.contains("<g clip-path=\"url(#clip1)\">"));
    }

    #[test]
    fn a_linear_gradient_becomes_a_def_and_a_clipped_fill() {
        let stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::WHITE,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLACK,
            },
        ];
        let p = page(vec![DrawOp::LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(100.0, 0.0),
            stops,
            path: rect(0.0, 0.0, 100.0, 50.0),
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<linearGradient id=\"grad1\" gradientUnits=\"userSpaceOnUse\" x1=\"0\" y1=\"0\" x2=\"100\" y2=\"0\">"));
        assert!(svg.contains("<stop offset=\"0\" stop-color=\"#ffffff\" stop-opacity=\"1\"/>"));
        assert!(svg.contains("<stop offset=\"1\" stop-color=\"#000000\" stop-opacity=\"1\"/>"));
        assert!(svg.contains("fill=\"url(#grad1)\""));
    }

    #[test]
    fn a_radial_gradient_becomes_a_def_and_a_clipped_fill() {
        let stops = vec![GradientStop {
            offset: 0.5,
            color: Color::WHITE,
        }];
        let p = page(vec![DrawOp::RadialGradient {
            center: Point::new(10.0, 10.0),
            radius: 25.0,
            stops,
            path: rect(0.0, 0.0, 20.0, 20.0),
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<radialGradient id=\"grad1\" gradientUnits=\"userSpaceOnUse\" cx=\"10\" cy=\"10\" r=\"25\">"));
        assert!(svg.contains("fill=\"url(#grad1)\""));
    }

    #[test]
    fn an_image_embeds_as_a_data_uri_rectangle() {
        let p = page(vec![DrawOp::Image {
            image: ImageRef::new(
                "i",
                std::sync::Arc::new(vec![1, 2, 3, 4]),
                (2, 2),
                ImageCodec::Png,
            ),
            dst: Rect::new(5.0, 6.0, 7.0, 8.0),
            transform: Affine::IDENTITY,
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(svg.contains("<image href=\"data:image/png;base64,AQIDBA==\" x=\"5\" y=\"6\" width=\"7\" height=\"8\"/>"));
    }

    #[test]
    fn a_rotated_image_falls_back_to_the_matrix_form() {
        let p = page(vec![DrawOp::Image {
            image: ImageRef::new("i", std::sync::Arc::new(vec![1]), (1, 1), ImageCodec::Png),
            dst: Rect::new(0.0, 0.0, 10.0, 10.0),
            transform: Affine::rotate_deg(90.0),
        }]);
        let svg = page_to_svg(&p, &SvgOptions::default()).expect("convert");
        assert!(
            svg.contains("<image href=\"data:image/png;base64,AQ==\" width=\"1\" height=\"1\"")
        );
        assert!(svg.contains("transform=\"matrix("));
    }

    #[test]
    fn live_text_uses_the_run_transform_and_size() {
        let run = GlyphRun {
            font: FontId(1),
            size: Millimeter(3.5),
            glyphs: vec![1, 2],
            advances: vec![3.5, 3.5],
            text: Some("a<b".into()),
            cluster_map: None,
            transform: Affine::translate(12.0, 30.0),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::new(0.0, 0.0, 7.0, 3.5),
        };
        let svg =
            page_to_svg(&page(vec![DrawOp::Text(run)]), &SvgOptions::default()).expect("convert");
        assert!(
            svg.contains(
                "<text x=\"12\" y=\"30\" font-size=\"3.5\" fill=\"#000000\">a&lt;b</text>"
            )
        );
    }

    #[test]
    fn a_rotated_run_emits_a_matrix_instead_of_x_and_y() {
        let run = GlyphRun {
            font: FontId(1),
            size: Millimeter(3.0),
            glyphs: vec![1],
            advances: vec![3.0],
            text: Some("A".into()),
            cluster_map: None,
            transform: Affine::translate(1.0, 2.0).then(Affine::rotate_deg(90.0)),
            fill: None,
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::new(0.0, 0.0, 3.0, 3.0),
        };
        let svg =
            page_to_svg(&page(vec![DrawOp::Text(run)]), &SvgOptions::default()).expect("convert");
        // translate(1, 2) acts after the rotation, so the glyph origin lands at
        // (-2, 1) rather than (1, 2).
        assert!(svg.contains("transform=\"matrix(0 1 -1 0 -2 1)\""));
    }

    #[test]
    fn a_run_without_text_and_without_fonts_is_counted_as_dropped() {
        let run = GlyphRun {
            font: FontId(9),
            size: Millimeter(3.0),
            glyphs: vec![1],
            advances: vec![3.0],
            text: None,
            cluster_map: None,
            transform: Affine::IDENTITY,
            fill: None,
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::EMPTY,
        };
        let mut sink = SvgSink::new(Rect::A4_PORTRAIT, &SvgOptions::default());
        replay(&page(vec![DrawOp::Text(run)]), &mut sink).expect("replay");
        let (svg, stats) = sink.finish();
        assert!(!svg.contains("<text"));
        assert_eq!(stats.dropped_runs, 1);
    }

    #[test]
    fn an_empty_run_emits_nothing_at_all() {
        let run = GlyphRun {
            font: FontId(1),
            size: Millimeter(3.0),
            glyphs: Vec::new(),
            advances: Vec::new(),
            text: Some(String::new()),
            cluster_map: None,
            transform: Affine::IDENTITY,
            fill: None,
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::EMPTY,
        };
        let svg =
            page_to_svg(&page(vec![DrawOp::Text(run)]), &SvgOptions::default()).expect("convert");
        assert!(!svg.contains("<text"));
    }

    #[test]
    fn an_unbalanced_restore_is_an_error() {
        let err = page_to_svg(&page(vec![DrawOp::Restore]), &SvgOptions::default())
            .expect_err("an unbalanced restore must fail");
        assert!(matches!(
            err,
            steleofd_error::OfdError::Format(steleofd_error::FormatError::CircularReference { .. })
        ));
    }

    #[test]
    fn a_trailing_save_still_produces_balanced_markup() {
        let svg = page_to_svg(&page(vec![DrawOp::Save]), &SvgOptions::default()).expect("convert");
        assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count());
    }

    #[test]
    fn arcs_carry_their_flags_into_the_path_data() {
        let mut path = Path::new();
        path.segments.push(PathSegment::MoveTo(0.0, 0.0));
        path.segments.push(PathSegment::Arc {
            rx: 5.0,
            ry: 5.0,
            x_axis_rotation: 30.0,
            large_arc: true,
            sweep: false,
            to: (10.0, 10.0),
        });
        let svg = page_to_svg(
            &page(vec![DrawOp::Path {
                path,
                fill: None,
                stroke: None,
                stroke_style: StrokeStyle::default(),
                rule: PaintRule::NonZero,
            }]),
            &SvgOptions::default(),
        )
        .expect("convert");
        assert!(svg.contains("A5 5 30 1 0 10 10"));
    }

    #[test]
    fn numbers_strip_trailing_zeros_and_negative_zero() {
        assert_eq!(num(2.0), "2");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(1.0 / 3.0), "0.3333");
        assert_eq!(num(f64::NAN), "0");
    }

    #[test]
    fn text_content_is_escaped() {
        assert_eq!(escape_text("a&b<c>d"), "a&amp;b&lt;c&gt;d");
    }

    #[test]
    fn every_page_of_a_list_becomes_a_document() {
        let list = steleofd_display::DisplayList {
            page_count: 2,
            pages: vec![page(Vec::new()), page(Vec::new())],
        };
        let docs = pages_to_svg(&list, &SvgOptions::default()).expect("convert");
        assert_eq!(docs.len(), 2);
        assert!(docs.iter().all(|d| d.contains("</svg>")));
    }
}
