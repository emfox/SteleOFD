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

