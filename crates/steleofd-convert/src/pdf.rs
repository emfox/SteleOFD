//! PDF backend.
//! Every transform is baked into the content stream, so the output is a flat
//! sequence of absolute painting operators that any viewer can replay.

use std::fmt::Write as _;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use steleofd_common::{Point, Rect};
use steleofd_display::ops::{
    Color, GradientStop, LineCap, LineJoin, Paint, PaintRule, StrokeStyle,
};
use steleofd_display::{GlyphRun, ImageRef};
use steleofd_error::{ConvertError, OfdError, Result};
use steleofd_font::FontManager;
use steleofd_geometry::{Affine, Path, PathSegment};
use steleofd_render::arc_cubics;

use crate::replay::{GradientShape, PageSink, PathShape, replay};
use crate::{ConversionStats, TextExport};

/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// Options for PDF export.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions<'a> {
    /// How glyph runs become PDF content.
    pub text: TextExport,
    /// Font source for outlined text; without one, runs fall back to live text.
    pub fonts: Option<&'a FontManager>,
}

/// An image waiting to become an XObject.
struct PendingImage {
    /// Encoded bytes.
    data: std::sync::Arc<Vec<u8>>,
    /// Intrinsic pixel dimensions.
    pixels: (u32, u32),
}

/// A gradient waiting to become a shading pattern.
struct PendingShading {
    /// Whether the gradient runs out of a circle rather than along an axis.
    radial: bool,
    /// PDF `Coords`: two points, or two circle centres with radii.
    coords: Vec<f64>,
    /// Colour stops.
    stops: Vec<GradientStop>,
}

/// Builds one PDF page by replaying a display list into it.
pub struct PdfSink<'a> {
    ops: String,
    page_box: Rect,
    text: TextExport,
    fonts: Option<&'a FontManager>,
    ctm: Affine,
    stack: Vec<Affine>,
    /// Open clip groups per save level, closed by the matching restore.
    clips: Vec<usize>,
    images: Vec<PendingImage>,
    /// All shadings in creation order; the name is the position.
    shadings: Vec<PendingShading>,
    /// Per-kind index into `shadings`, for gradient paint references.
    linear_refs: Vec<usize>,
    /// Per-kind index into `shadings`, for gradient paint references.
    radial_refs: Vec<usize>,
    /// Distinct alpha values, each becoming one ExtGState.
    alphas: Vec<f64>,
    /// Whether live text was emitted, which needs the Helvetica resource.
    live_text: bool,
    stats: ConversionStats,
}

impl<'a> PdfSink<'a> {
    /// Start a page of the given size.
    pub fn new(page_box: Rect, options: &PdfOptions<'a>) -> Self {
        Self {
            ops: String::new(),
            page_box,
            text: options.text,
            fonts: options.fonts,
            ctm: Affine::IDENTITY,
            stack: Vec::new(),
            clips: vec![0],
            images: Vec::new(),
            shadings: Vec::new(),
            linear_refs: Vec::new(),
            radial_refs: Vec::new(),
            alphas: Vec::new(),
            live_text: false,
            stats: ConversionStats::default(),
        }
    }

    /// What the page contained, for callers that report conversion quality.
    pub fn stats(&self) -> &ConversionStats {
        &self.stats
    }

    /// Add this page's content and resources to a document.
    pub fn emit_page(&mut self, doc: &mut Document) -> Result<ObjectId> {
        let mut resources = Dictionary::new();
        if !self.images.is_empty() {
            let mut xobjects = Dictionary::new();
            for (index, pending) in self.images.iter().enumerate() {
                let stream = image_xobject(pending)?;
                xobjects.set(
                    format!("Im{index}"),
                    Object::Reference(doc.add_object(stream)),
                );
            }
            resources.set("XObject", Object::Dictionary(xobjects));
        }
        if !self.shadings.is_empty() {
            let mut patterns = Dictionary::new();
            for (index, shading) in self.shadings.iter().enumerate() {
                let stream = shading_pattern(shading, doc)?;
                patterns.set(
                    format!("Sh{index}"),
                    Object::Reference(doc.add_object(stream)),
                );
            }
            resources.set("Pattern", Object::Dictionary(patterns));
        }
        if !self.alphas.is_empty() {
            let mut states = Dictionary::new();
            for (index, alpha) in self.alphas.iter().enumerate() {
                let mut state = Dictionary::new();
                state.set("Type", Object::Name(b"ExtGState".to_vec()));
                state.set("ca", Object::Real(*alpha as f32));
                state.set("CA", Object::Real(*alpha as f32));
                states.set(format!("GS{index}"), Object::Dictionary(state));
            }
            resources.set("ExtGState", Object::Dictionary(states));
        }
        if self.live_text {
            let mut fonts = Dictionary::new();
            let mut font = Dictionary::new();
            font.set("Type", Object::Name(b"Font".to_vec()));
            font.set("Subtype", Object::Name(b"Type1".to_vec()));
            font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
            font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
            fonts.set("F1", Object::Dictionary(font));
            resources.set("Font", Object::Dictionary(fonts));
        }

        let mut content = Stream::new(
            Dictionary::new(),
            std::mem::take(&mut self.ops).into_bytes(),
        );
        content
            .compress()
            .map_err(|e| convert_error("content stream", e.to_string()))?;
        let content_id = doc.add_object(content);

        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real((self.page_box.w * PT_PER_MM) as f32),
                Object::Real((self.page_box.h * PT_PER_MM) as f32),
            ]),
        );
        page.set("Contents", Object::Reference(content_id));
        page.set("Resources", Object::Dictionary(resources));
        Ok(doc.add_object(page))
    }

    /// Page millimetres to PDF points, page box origin and y flip included.
    /// `local` acts first, then the current transform, matching how the raster
    /// backend composes the same matrices.
    fn to_pt(&self, local: Affine) -> Affine {
        // Scale first, then translate: composing the other way round would
        // scale the page offset by `PT_PER_MM` a second time.
        let base = Affine::scale(PT_PER_MM, -PT_PER_MM).then(Affine::translate(
            -self.page_box.x * PT_PER_MM,
            (self.page_box.y + self.page_box.h) * PT_PER_MM,
        ));
        local.then(self.ctm).then(base)
    }

    fn op(&mut self, text: String) {
        self.ops.push_str(&text);
        self.ops.push('\n');
    }

    /// Register an ExtGState for a translucent alpha, reusing what exists.
    fn alpha_state(&mut self, alpha: f64) -> String {
        let found = self.alphas.iter().position(|a| (*a - alpha).abs() < 1e-6);
        let index = match found {
            Some(i) => i,
            None => {
                self.alphas.push(alpha);
                self.alphas.len() - 1
            }
        };
        format!(" /GS{index} gs")
    }

    /// Emit a path, flattening arcs and elevating curves into cubics.
    fn write_path(&mut self, path: &Path, total: Affine) {
        let mut cur = (0.0, 0.0);
        for seg in &path.segments {
            match *seg {
                PathSegment::MoveTo(x, y) => {
                    cur = (x, y);
                    self.op(point_op(total.apply(x, y), "m"));
                }
                PathSegment::LineTo(x, y) => {
                    cur = (x, y);
                    self.op(point_op(total.apply(x, y), "l"));
                }
                PathSegment::CurveTo { c1, c2, to } => {
                    cur = to;
                    self.op(curve_op(total, c1, c2, to));
                }
                PathSegment::QuadTo { c, to } => {
                    // Elevating a quadratic to a cubic is exact.
                    let c1 = (
                        cur.0 + 2.0 / 3.0 * (c.0 - cur.0),
                        cur.1 + 2.0 / 3.0 * (c.1 - cur.1),
                    );
                    let c2 = (
                        to.0 + 2.0 / 3.0 * (c.0 - to.0),
                        to.1 + 2.0 / 3.0 * (c.1 - to.1),
                    );
                    cur = to;
                    self.op(curve_op(total, c1, c2, to));
                }
                PathSegment::Arc {
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    to,
                } => {
                    for (c1, c2, end) in
                        arc_cubics(rx, ry, x_axis_rotation, large_arc, sweep, cur, to)
                    {
                        self.op(curve_op(total, c1, c2, end));
                    }
                    cur = to;
                }
                PathSegment::Close => self.op("h".to_string()),
            }
        }
    }

    /// Fill a region with a shading, clipped to the path by construction.
    fn fill_shading(&mut self, path: &Path, name: &str) {
        let total = self.to_pt(Affine::IDENTITY);
        let bbox = path.tight_bounding_box();
        let (x0, y0) = total.apply(bbox.x, bbox.y);
        let (x1, y1) = total.apply(bbox.x + bbox.w, bbox.y + bbox.h);
        self.write_path(path, total);
        self.op(format!(
            "W n /Pattern cs /{name} scn {} {} {} {} re f",
            num(x0),
            num(y0),
            num(x1),
            num(y1)
        ));
    }

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
            let Some(outline) = face.outline_for_run_code(gid, run.codes_are_glyphs) else {
                continue;
            };
            // Outlines are normalized to 1/1000 em with y pointing up: scale
            // to millimetres first, then position, or the origin gets scaled.
            let glyph = Affine::new(k, 0.0, 0.0, -k, 0.0, 0.0)
                .then(Affine::translate(offset, 0.0))
                .then(run.transform);
            let total = self.to_pt(glyph);
            self.write_path(&outline, total);
            let mut setup = match fill {
                Paint::Solid(c) => {
                    let mut s = format!("{} rg", rgb(c));
                    if c.a != 255 {
                        s.push_str(&self.alpha_state(c.a as f64 / 255.0));
                    }
                    s
                }
                _ => "0 0 0 rg".to_string(),
            };
            setup.push_str(" f");
            self.op(setup);
            drawn += 1;
        }
        if drawn > 0 {
            self.stats.outlined_glyphs += drawn;
            true
        } else {
            false
        }
    }

    /// Emit the run's Unicode text against a standard font.
    fn live_text(&mut self, run: &GlyphRun) {
        let Some(text) = run.text.as_deref().filter(|t| !t.is_empty()) else {
            self.stats.dropped_runs += 1;
            return;
        };
        self.live_text = true;
        self.stats.live_text_runs += 1;
        let mut out = format!("BT /F1 {} Tf", num(run.size.value() * PT_PER_MM));
        let mut offset = 0.0;
        for (i, ch) in text.chars().enumerate() {
            if i > 0 {
                offset += run.advances.get(i - 1).copied().unwrap_or(0.0);
            }
            offset += run.letter_spacing;
            let local = run.transform.then(Affine::translate(offset, 0.0));
            let (e, f) = self.to_pt(local).apply(0.0, 0.0);
            let composed = self.ctm.then(local);
            let _ = write!(
                out,
                " {} {} {} {} {} {} Tm ({}) Tj",
                num(composed.a),
                num(-composed.b),
                num(-composed.c),
                num(composed.d),
                num(e),
                num(f),
                pdf_string(ch.encode_utf8(&mut [0u8; 4]))
            );
        }
        out.push_str(" ET");
        self.op(out);
    }
}

impl PageSink for PdfSink<'_> {
    fn save(&mut self) {
        self.stack.push(self.ctm);
        self.clips.push(0);
    }

    fn restore(&mut self) {
        self.ctm = self.stack.pop().unwrap_or(Affine::IDENTITY);
        let open = if self.clips.len() > 1 {
            self.clips.pop().unwrap_or(0)
        } else {
            0
        };
        for _ in 0..open {
            self.op("Q".to_string());
        }
    }

    fn transform(&mut self, matrix: Affine) {
        self.ctm = self.ctm.then(matrix);
    }

    fn clip(&mut self, path: &Path, rule: PaintRule) {
        let total = self.to_pt(Affine::IDENTITY);
        self.write_path(path, total);
        // `W n` ends the path but leaves the clip in force until `Q`.
        let operator = match rule {
            PaintRule::NonZero => "W n",
            PaintRule::EvenOdd => "W* n",
        };
        self.op(format!("q {operator}"));
        if let Some(top) = self.clips.last_mut() {
            *top += 1;
        }
    }

    fn path(&mut self, shape: PathShape<'_>) {
        let total = self.to_pt(Affine::IDENTITY);
        let mut setup = String::new();
        let mut operator = String::new();
        let gradient = match shape.fill {
            Some(Paint::LinearGradientRef(i)) => self.linear_refs.get(*i).copied(),
            Some(Paint::RadialGradientRef(i)) => self.radial_refs.get(*i).copied(),
            _ => None,
        };
        let mut shaded = false;
        if let Some(index) = gradient {
            // A shading paints its own region, so a stroke has to follow it
            // rather than share one painting operator.
            self.fill_shading(shape.path, &format!("Sh{index}"));
            shaded = true;
            if shape.stroke.is_none() {
                return;
            }
        } else {
            match shape.fill {
                None => {}
                Some(Paint::Solid(c)) => {
                    let _ = write!(setup, "{} rg", rgb(c));
                    if c.a != 255 {
                        setup.push_str(&self.alpha_state(c.a as f64 / 255.0));
                    }
                    operator.push_str(match shape.rule {
                        PaintRule::NonZero => " f",
                        PaintRule::EvenOdd => " f*",
                    });
                }
                // Tiling patterns have no equivalent here; black keeps the
                // region visible rather than silently empty.
                Some(_) => {
                    setup.push_str("0 0 0 rg");
                    operator.push_str(" f");
                }
            }
        }
        if shape.stroke.is_some() {
            setup.push_str(&stroke_prefix(shape.stroke_style, &self.ctm));
            if let Some(paint) = shape.stroke {
                match paint {
                    Paint::Solid(c) => {
                        let _ = write!(setup, " {} RG", rgb(c));
                        if c.a != 255 {
                            setup.push_str(&self.alpha_state(c.a as f64 / 255.0));
                        }
                    }
                    _ => setup.push_str(" 0 0 0 RG"),
                }
            }
            operator = if shaded {
                " S".to_string()
            } else if shape.fill.is_some() {
                match shape.rule {
                    PaintRule::NonZero => " B".to_string(),
                    PaintRule::EvenOdd => " B*".to_string(),
                }
            } else {
                " S".to_string()
            };
        }
        if operator.trim().is_empty() {
            return;
        }
        self.write_path(shape.path, total);
        self.op(format!("{setup}{operator}"));
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
        if image.data.is_empty() {
            return;
        }
        let (iw, ih) = (
            image.pixel_size.0.max(1) as f64,
            image.pixel_size.1.max(1) as f64,
        );
        let pixel_to_page = Affine::new(dst.w / iw, 0.0, 0.0, dst.h / ih, dst.x, dst.y);
        let total = self.to_pt(transform.then(pixel_to_page));
        let index = self.images.len();
        self.images.push(PendingImage {
            data: image.data.clone(),
            pixels: image.pixel_size,
        });
        self.stats.images += 1;
        self.op(format!(
            "q {} {} {} {} {} {} cm /Im{index} Do Q",
            num(total.a),
            num(total.b),
            num(total.c),
            num(total.d),
            num(total.e),
            num(total.f)
        ));
    }

    fn linear_gradient(&mut self, start: Point, end: Point, shape: GradientShape<'_>) {
        if shape.stops.is_empty() {
            return;
        }
        let total = self.to_pt(Affine::IDENTITY);
        let (x0, y0) = total.apply(start.x, start.y);
        let (x1, y1) = total.apply(end.x, end.y);
        let index = self.shadings.len();
        self.linear_refs.push(index);
        self.shadings.push(PendingShading {
            radial: false,
            coords: vec![x0, y0, x1, y1],
            stops: shape.stops.to_vec(),
        });
        self.stats.gradients += 1;
        self.fill_shading(shape.path, &format!("Sh{index}"));
    }

    fn radial_gradient(
        &mut self,
        center: Point,
        radius: f64,
        end: Point,
        end_radius: f64,
        shape: GradientShape<'_>,
    ) {
        if shape.stops.is_empty() {
            return;
        }
        let total = self.to_pt(Affine::IDENTITY);
        let (cx, cy) = total.apply(center.x, center.y);
        let (ex, ey) = total.apply(end.x, end.y);
        // The page transform is axis preserving up to the y flip, so a radius
        // scales with the mean factor.
        let scale = total.scale_factor();
        let index = self.shadings.len();
        self.radial_refs.push(index);
        // PDF's radial shading is the very cone the clause describes: two
        // circles, with the parameter running from the first to the second,
        // so the geometry travels as written.
        self.shadings.push(PendingShading {
            radial: true,
            coords: vec![cx, cy, radius * scale, ex, ey, end_radius * scale],
            stops: shape.stops.to_vec(),
        });
        self.stats.gradients += 1;
        self.fill_shading(shape.path, &format!("Sh{index}"));
    }
}

/// Convert one page into a single-page PDF document.
pub fn page_to_pdf(
    page: &steleofd_display::PageDisplay,
    options: &PdfOptions<'_>,
) -> Result<Vec<u8>> {
    let mut sink = PdfSink::new(page.page_box, options);
    replay(page, &mut sink)?;
    let mut doc = Document::new();
    let page_id = sink.emit_page(&mut doc)?;
    finish_document(&mut doc, vec![page_id])
}

/// Convert a whole display list into one multi-page PDF document.
pub fn document_to_pdf(
    list: &steleofd_display::DisplayList,
    options: &PdfOptions<'_>,
) -> Result<Vec<u8>> {
    let every: Vec<usize> = list.pages.iter().map(|page| page.index).collect();
    document_to_pdf_pages(list, &every, options)
}

/// Convert the named pages into one PDF, in the order given. A reader exporting
/// "the pages I am looking at" holds a subset, not a contiguous range: facing
/// mode shows a pair and a search lands anywhere, so this takes a list.
pub fn document_to_pdf_pages(
    list: &steleofd_display::DisplayList,
    pages: &[usize],
    options: &PdfOptions<'_>,
) -> Result<Vec<u8>> {
    let mut doc = Document::new();
    let mut page_ids = Vec::with_capacity(pages.len());
    for wanted in pages {
        let Some(page) = list.page(*wanted) else {
            return Err(OfdError::Format(
                steleofd_error::FormatError::PageOutOfRange {
                    index: *wanted,
                    total: list.page_count,
                },
            ));
        };
        let mut sink = PdfSink::new(page.page_box, options);
        replay(page, &mut sink)?;
        page_ids.push(sink.emit_page(&mut doc)?);
    }
    finish_document(&mut doc, page_ids)
}

/// Wire the catalog and page tree, then serialize.
fn finish_document(doc: &mut Document, page_ids: Vec<ObjectId>) -> Result<Vec<u8>> {
    let kids: Vec<Object> = page_ids.iter().copied().map(Object::Reference).collect();
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(kids));
    pages.set("Count", page_ids.len() as i64);
    let pages_id = doc.add_object(pages);
    for id in &page_ids {
        if let Ok(dict) = doc.get_dictionary_mut(*id) {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| convert_error("document", e.to_string()))?;
    Ok(out)
}

/// Build the shading pattern that paints one gradient.
fn shading_pattern(shading: &PendingShading, doc: &mut Document) -> Result<Stream> {
    let mut dict = Dictionary::new();
    dict.set("PatternType", Object::Integer(2));
    let mut inner = Dictionary::new();
    inner.set(
        "ShadingType",
        Object::Integer(if shading.radial { 3 } else { 2 }),
    );
    inner.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    inner.set(
        "Coords",
        Object::Array(
            shading
                .coords
                .iter()
                .map(|c| Object::Real(*c as f32))
                .collect(),
        ),
    );
    inner.set(
        "Extend",
        Object::Array(vec![Object::Boolean(true), Object::Boolean(true)]),
    );
    inner.set("Function", Object::Reference(stops_function(shading, doc)?));
    dict.set("Shading", Object::Dictionary(inner));
    Ok(Stream::new(dict, Vec::new()))
}

/// Build the colour function of a gradient: one exponential interval per pair
/// of stops, stitched together so piecewise linear interpolation is exact.
fn stops_function(shading: &PendingShading, doc: &mut Document) -> Result<ObjectId> {
    let stops = &shading.stops;
    let intervals: Vec<ObjectId> = stops
        .windows(2)
        .map(|pair| {
            let mut interval = Dictionary::new();
            interval.set("FunctionType", Object::Integer(2));
            interval.set(
                "Domain",
                Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
            );
            interval.set("C0", channel_array(&pair[0].color));
            interval.set("C1", channel_array(&pair[1].color));
            interval.set("N", Object::Integer(1));
            doc.add_object(interval)
        })
        .collect();
    if intervals.len() == 1 {
        return Ok(intervals[0]);
    }
    // A lone stop degenerates to a constant colour; two or more stops stitch
    // their intervals at the stop offsets between them.
    let (first, last) = match (stops.first(), stops.last()) {
        (Some(first), Some(last)) => (first.color, last.color),
        _ => {
            return Err(convert_error(
                "gradient",
                "a gradient needs at least one stop".into(),
            ));
        }
    };
    if intervals.is_empty() {
        let mut constant = Dictionary::new();
        constant.set("FunctionType", Object::Integer(2));
        constant.set(
            "Domain",
            Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
        );
        constant.set("C0", channel_array(&first));
        constant.set("C1", channel_array(&last));
        constant.set("N", Object::Integer(1));
        return Ok(doc.add_object(constant));
    }
    let mut stitched = Dictionary::new();
    stitched.set("FunctionType", Object::Integer(3));
    stitched.set(
        "Domain",
        Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
    );
    stitched.set(
        "Functions",
        Object::Array(intervals.iter().copied().map(Object::Reference).collect()),
    );
    let bounds: Vec<Object> = stops[1..stops.len() - 1]
        .iter()
        .map(|s| Object::Real(s.offset.clamp(0.0, 1.0) as f32))
        .collect();
    stitched.set("Bounds", Object::Array(bounds));
    let encode: Vec<Object> = (0..intervals.len())
        .flat_map(|_| [Object::Real(0.0), Object::Real(1.0)])
        .collect();
    stitched.set("Encode", Object::Array(encode));
    Ok(doc.add_object(stitched))
}

/// Channel triplet of a colour, for gradient function dictionaries.
fn channel_array(c: &Color) -> Object {
    Object::Array(
        channel_triplet(c)
            .into_iter()
            .map(|channel| Object::Real(channel as f32))
            .collect(),
    )
}

/// Build an image XObject from encoded bytes, dropping alpha.
fn image_xobject(image: &PendingImage) -> Result<Stream> {
    let decoded =
        image::load_from_memory(&image.data).map_err(|e| convert_error("image", e.to_string()))?;
    let rgb = decoded.to_rgb8();
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", image.pixels.0 as i64);
    dict.set("Height", image.pixels.1 as i64);
    dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    dict.set("BitsPerComponent", Object::Integer(8));
    let mut stream = Stream::new(dict, rgb.into_raw());
    stream
        .compress()
        .map_err(|e| convert_error("image stream", e.to_string()))?;
    Ok(stream)
}

/// `r g b` operands for a colour, each channel `0..=1`.
fn rgb(c: &Color) -> String {
    format!(
        "{} {} {}",
        num(c.r as f64 / 255.0),
        num(c.g as f64 / 255.0),
        num(c.b as f64 / 255.0)
    )
}

/// Channel triplet of a colour, for gradient function dictionaries.
fn channel_triplet(c: &Color) -> [f64; 3] {
    [c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0]
}

/// Stroke parameters, scaled by the transform in effect.
fn stroke_prefix(style: &StrokeStyle, ctm: &Affine) -> String {
    let s = PT_PER_MM * ctm.scale_factor();
    let mut out = format!("{} w", num(style.width.max(0.0) * s));
    if !style.dash.is_empty() {
        let pattern: Vec<String> = style.dash.iter().map(|d| num(*d * s)).collect();
        let _ = write!(
            out,
            " [{}] {} d",
            pattern.join(" "),
            num(style.dash_offset * s)
        );
    }
    let cap = match style.cap {
        LineCap::Butt => 0,
        LineCap::Round => 1,
        LineCap::Square => 2,
    };
    let join = match style.join {
        LineJoin::Miter => 0,
        LineJoin::Round => 1,
        LineJoin::Bevel => 2,
    };
    let _ = write!(out, " {cap} J {join} j {} M", num(style.miter_limit));
    out
}

/// One point operand followed by an operator.
fn point_op(point: (f64, f64), operator: &str) -> String {
    format!("{} {} {operator}", num(point.0), num(point.1))
}

/// A cubic Bezier operator with all three points mapped.
fn curve_op(total: Affine, c1: (f64, f64), c2: (f64, f64), to: (f64, f64)) -> String {
    let (c1x, c1y) = total.apply(c1.0, c1.1);
    let (c2x, c2y) = total.apply(c2.0, c2.1);
    let (x, y) = total.apply(to.0, to.1);
    format!(
        "{} {} {} {} {} {} c",
        num(c1x),
        num(c1y),
        num(c2x),
        num(c2y),
        num(x),
        num(y)
    )
}

/// Escape a string for a PDF literal, keeping raw bytes intact.
fn pdf_string(text: &str) -> String {
    let mut out = Vec::with_capacity(text.len() + 2);
    for &byte in text.as_bytes() {
        match byte {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(byte);
            }
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\n' => out.extend_from_slice(b"\\n"),
            _ => out.push(byte),
        }
    }
    // Escaping only inserts ASCII, so the result stays valid UTF-8.
    String::from_utf8(out).unwrap_or_default()
}

/// A conversion failure tied to one part of the document.
fn convert_error(object: &str, reason: String) -> OfdError {
    OfdError::Convert(ConvertError::MappingFailed {
        page: 0,
        object: object.to_string(),
        reason,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use steleofd_common::Millimeter;
    use steleofd_display::ops::Paint;
    use steleofd_font::FontId;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
        Path::build(|b| {
            b.rect(x, y, w, h);
        })
    }

    fn page(ops: Vec<steleofd_display::DrawOp>) -> steleofd_display::PageDisplay {
        steleofd_display::PageDisplay {
            index: 0,
            page_box: Rect::A4_PORTRAIT,
            ops,
            content_bbox: Rect::EMPTY,
        }
    }

    fn content_of(bytes: &[u8]) -> String {
        let doc = Document::load_from(bytes).expect("the pdf we just wrote must parse");
        let pages = doc.get_pages();
        let page_id = *pages.values().next().expect("one page");
        let content = doc.get_page_content(page_id).expect("page content");
        String::from_utf8(content).expect("utf8 content")
    }

    #[test]
    fn a_rect_becomes_a_fill_operator_in_points() {
        // 10mm at 72/25.4 points per millimetre is 28.3465pt.
        let p = page(vec![steleofd_display::DrawOp::Path {
            path: rect(10.0, 10.0, 10.0, 10.0),
            fill: Some(Paint::Solid(Color::rgba(255, 0, 0, 255))),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }]);
        let pdf = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        assert!(pdf.starts_with(b"%PDF-"));
        let content = content_of(&pdf);
        assert!(content.contains("1 0 0 rg"));
        // x runs 10..30mm; y is measured from the bottom, so 10..20mm from the
        // top of a 297mm page lands at 756.8504..813.5433 points.
        assert!(content.contains("28.3465 813.5433 m"));
        assert!(content.contains("56.6929 813.5433 l"));
        assert!(content.trim_end().ends_with("f"));
    }

    #[test]
    fn the_y_axis_flips_into_pdf_space() {
        // A rect at the top of a 297mm page lands near the top in points.
        let p = page(vec![steleofd_display::DrawOp::Path {
            path: rect(0.0, 0.0, 10.0, 10.0),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(
            content.contains("0 841.8898 m"),
            "the origin maps to the top"
        );
    }

    #[test]
    fn the_even_odd_rule_selects_the_starred_fill() {
        let p = page(vec![steleofd_display::DrawOp::Path {
            path: rect(0.0, 0.0, 10.0, 10.0),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::EvenOdd,
        }]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(content.contains("f*"));
    }

    #[test]
    fn a_stroke_carries_width_dash_and_joins() {
        let style = StrokeStyle {
            width: 0.5,
            dash: vec![3.0, 1.0],
            dash_offset: 0.25,
            cap: steleofd_display::ops::LineCap::Round,
            join: steleofd_display::ops::LineJoin::Bevel,
            miter_limit: 2.0,
        };
        let p = page(vec![steleofd_display::DrawOp::Path {
            path: rect(0.0, 0.0, 10.0, 10.0),
            fill: None,
            stroke: Some(Paint::Solid(Color::BLACK)),
            stroke_style: style,
            rule: PaintRule::NonZero,
        }]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(content.contains("1.4173 w"));
        assert!(content.contains("[8.5039 2.8346] 0.7087 d"));
        assert!(content.contains("1 J 2 j 2 M"));
        assert!(content.trim_end().ends_with("S"));
    }

    #[test]
    fn a_transform_scales_geometry_and_stroke_together() {
        let p = page(vec![
            steleofd_display::DrawOp::Save,
            steleofd_display::DrawOp::Transform(Affine::scale(2.0, 2.0)),
            steleofd_display::DrawOp::Path {
                path: rect(0.0, 0.0, 10.0, 10.0),
                fill: None,
                stroke: Some(Paint::Solid(Color::BLACK)),
                stroke_style: StrokeStyle::default(),
                rule: PaintRule::NonZero,
            },
            steleofd_display::DrawOp::Restore,
        ]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        // The rect doubles to 56.69pt along the top edge and the 1mm stroke to
        // 5.6693pt.
        assert!(content.contains("56.6929 841.8898 l"));
        assert!(content.contains("5.6693 w"));
    }

    #[test]
    fn a_clip_opens_a_q_group_the_restore_closes() {
        let p = page(vec![
            steleofd_display::DrawOp::Save,
            steleofd_display::DrawOp::ClipPath {
                path: rect(0.0, 0.0, 10.0, 10.0),
                rule: PaintRule::NonZero,
            },
            steleofd_display::DrawOp::Restore,
        ]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(content.contains("q "), "the clip opens a group");
        assert!(content.contains("W n"), "the path becomes a clip");
        assert!(content.contains("Q"), "the restore closes the group");
    }

    #[test]
    fn an_even_odd_clip_uses_the_starred_operator() {
        let p = page(vec![
            steleofd_display::DrawOp::Save,
            steleofd_display::DrawOp::ClipPath {
                path: rect(0.0, 0.0, 10.0, 10.0),
                rule: PaintRule::EvenOdd,
            },
            steleofd_display::DrawOp::Restore,
        ]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(content.contains("W* n"));
    }

    #[test]
    fn a_linear_gradient_becomes_a_shading_pattern() {
        let p = page(vec![steleofd_display::DrawOp::LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::WHITE,
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::BLACK,
                },
            ],
            path: rect(0.0, 0.0, 10.0, 10.0),
        }]);
        let bytes = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        let page_id = *doc.get_pages().values().next().expect("page");
        let resources = doc.get_page_resources(page_id).expect("resources");
        let patterns = resources
            .0
            .expect("pattern dict")
            .get(b"Pattern")
            .expect("patterns");
        assert!(
            patterns.as_dict().is_ok(),
            "a shading pattern is registered"
        );
        let content = content_of(&bytes);
        assert!(content.contains("/Pattern cs"));
        assert!(content.contains("scn"));
        assert!(content.contains("re f"));
    }

    #[test]
    fn a_radial_gradient_uses_shading_type_three() {
        let p = page(vec![steleofd_display::DrawOp::RadialGradient {
            center: Point::new(5.0, 5.0),
            radius: 1.0,
            end: Point::new(8.0, 5.0),
            end_radius: 5.0,
            stops: vec![GradientStop {
                offset: 0.0,
                color: Color::WHITE,
            }],
            path: rect(0.0, 0.0, 10.0, 10.0),
        }]);
        let bytes = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        // The shading dictionary sits inside the pattern stream, so look one
        // level down rather than at the top of every object.
        let found = doc.objects.values().any(|o| {
            let dict = match o {
                Object::Stream(stream) => Some(&stream.dict),
                Object::Dictionary(dict) => Some(dict),
                _ => None,
            };
            dict.and_then(|d| d.get(b"Shading").ok())
                .and_then(|s| s.as_dict().ok())
                .and_then(|d| d.get(b"ShadingType").ok())
                .and_then(|t| t.as_i64().ok())
                == Some(3)
        });
        assert!(found, "a radial shading is type 3");
        // Both circles travel: PDF's type 3 is the cone the clause describes,
        // so the two centres and both radii reach the shading as written.
        let coords = doc.objects.values().find_map(|o| match o {
            Object::Stream(stream) => stream
                .dict
                .get(b"Shading")
                .ok()
                .and_then(|s| s.as_dict().ok())
                .and_then(|d| d.get(b"Coords").ok())
                .and_then(|c| c.as_array().ok())
                .map(|a| a.iter().filter_map(|v| v.as_f32().ok()).collect::<Vec<_>>()),
            _ => None,
        });
        let coords = coords.expect("the shading carries its two circles");
        assert_eq!(coords.len(), 6, "two circles, each a centre and a radius");
        let pt = (72.0 / 25.4) as f32;
        assert!((coords[2] - pt).abs() < 1e-3, "start radius: {:?}", coords);
        assert!(
            (coords[5] - 5.0 * pt).abs() < 1e-3,
            "end radius: {:?}",
            coords
        );
        assert!(
            coords[0] < coords[3],
            "the centres stay apart: {:?}",
            coords
        );
    }

    #[test]
    fn a_multi_stop_gradient_stitches_its_intervals() {
        let p = page(vec![steleofd_display::DrawOp::LinearGradient {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::WHITE,
                },
                GradientStop {
                    offset: 0.5,
                    color: Color::rgba(255, 0, 0, 255),
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::BLACK,
                },
            ],
            path: rect(0.0, 0.0, 10.0, 10.0),
        }]);
        let bytes = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        let stitched = doc.objects.values().any(|o| {
            o.as_dict()
                .ok()
                .and_then(|d| d.get(b"FunctionType").ok())
                .and_then(|t| t.as_i64().ok())
                == Some(3)
        });
        assert!(stitched, "three stops stitch two intervals");
    }

    #[test]
    fn an_image_becomes_an_xobject_drawn_with_a_matrix() {
        let mut png = Vec::new();
        let solid = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        solid
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode the fixture");
        let p = page(vec![steleofd_display::DrawOp::Image {
            image: ImageRef::new(
                "i",
                std::sync::Arc::new(png),
                (2, 2),
                steleofd_display::ImageCodec::Png,
            ),
            dst: Rect::new(5.0, 5.0, 10.0, 10.0),
            transform: Affine::IDENTITY,
        }]);
        let bytes = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        let page_id = *doc.get_pages().values().next().expect("page");
        let images = doc.get_page_images(page_id).expect("images");
        assert_eq!(images.len(), 1, "one image xobject");
        let content = content_of(&bytes);
        assert!(content.contains("/Im0 Do"));
        assert!(content.contains("14.1732 0 0 -14.1732 14.1732 827.7165 cm"));
    }

    #[test]
    fn live_text_positions_each_character_with_its_own_matrix() {
        let run = GlyphRun {
            font: FontId(1),
            size: Millimeter(10.0),
            glyphs: vec![1, 2],
            advances: vec![4.0, 4.0],
            text: Some("ab".into()),
            cluster_map: None,
            codes_are_glyphs: false,
            transform: Affine::translate(20.0, 30.0),
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::new(0.0, 0.0, 8.0, 10.0),
        };
        let content = content_of(
            &page_to_pdf(
                &page(vec![steleofd_display::DrawOp::Text(run)]),
                &PdfOptions::default(),
            )
            .expect("convert"),
        );
        assert!(content.contains("BT /F1 28.3465 Tf"));
        assert!(content.contains("1 0 0 1 56.6929 756.8504 Tm (a) Tj"));
        assert!(content.contains("1 0 0 1 68.0315 756.8504 Tm (b) Tj"));
        assert!(content.contains("ET"));
    }

    #[test]
    fn parentheses_in_text_are_escaped() {
        assert_eq!(pdf_string("a(b)c\\d"), "a\\(b\\)c\\\\d");
        assert_eq!(pdf_string("line\nbreak"), "line\\nbreak");
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
            codes_are_glyphs: false,
            transform: Affine::IDENTITY,
            fill: None,
            stroke: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            bbox: Rect::EMPTY,
        };
        let mut sink = PdfSink::new(Rect::A4_PORTRAIT, &PdfOptions::default());
        replay(&page(vec![steleofd_display::DrawOp::Text(run)]), &mut sink).expect("replay");
        assert_eq!(sink.stats().dropped_runs, 1);
        assert!(!sink.ops.contains("BT"), "nothing was emitted");
    }

    #[test]
    fn a_translucent_fill_registers_one_extgstate() {
        let p = page(vec![
            steleofd_display::DrawOp::Path {
                path: rect(0.0, 0.0, 10.0, 10.0),
                fill: Some(Paint::Solid(Color::rgba(0, 0, 0, 128))),
                stroke: None,
                stroke_style: StrokeStyle::default(),
                rule: PaintRule::NonZero,
            },
            steleofd_display::DrawOp::Path {
                path: rect(20.0, 20.0, 10.0, 10.0),
                fill: Some(Paint::Solid(Color::rgba(0, 0, 0, 128))),
                stroke: None,
                stroke_style: StrokeStyle::default(),
                rule: PaintRule::NonZero,
            },
        ]);
        let bytes = page_to_pdf(&p, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        let page_id = *doc.get_pages().values().next().expect("page");
        // The graphics states live inline in the page resources, so count them
        // there rather than among the top level objects.
        let resources = doc.get_page_resources(page_id).expect("resources");
        let states = resources
            .0
            .expect("resource dict")
            .get(b"ExtGState")
            .expect("extgstate dict")
            .as_dict()
            .expect("state dict");
        assert_eq!(states.len(), 1, "one alpha, one state");
        let content = content_of(&bytes);
        assert!(content.contains("/GS0 gs"));
    }

    #[test]
    fn every_page_of_a_list_lands_in_the_document() {
        let list = steleofd_display::DisplayList {
            page_count: 3,
            pages: vec![page(Vec::new()), page(Vec::new()), page(Vec::new())],
        };
        let bytes = document_to_pdf(&list, &PdfOptions::default()).expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        assert_eq!(doc.get_pages().len(), 3);
    }

    #[test]
    fn an_unbalanced_restore_is_an_error() {
        let err = page_to_pdf(
            &page(vec![steleofd_display::DrawOp::Restore]),
            &PdfOptions::default(),
        )
        .expect_err("an unbalanced restore must fail");
        assert!(matches!(
            err,
            OfdError::Format(steleofd_error::FormatError::CircularReference { .. })
        ));
    }

    #[test]
    fn a_trailing_save_still_writes_a_balanced_document() {
        let bytes = page_to_pdf(
            &page(vec![steleofd_display::DrawOp::Save]),
            &PdfOptions::default(),
        )
        .expect("convert");
        let doc = Document::load_from(&bytes[..]).expect("parse");
        assert_eq!(doc.get_pages().len(), 1);
    }

    #[test]
    fn an_arc_is_flattened_into_cubics() {
        let mut path = Path::new();
        path.segments.push(PathSegment::MoveTo(0.0, 0.0));
        path.segments.push(PathSegment::Arc {
            rx: 5.0,
            ry: 5.0,
            x_axis_rotation: 0.0,
            large_arc: false,
            sweep: true,
            to: (10.0, 0.0),
        });
        let p = page(vec![steleofd_display::DrawOp::Path {
            path,
            fill: Some(Paint::Solid(Color::BLACK)),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }]);
        let content = content_of(&page_to_pdf(&p, &PdfOptions::default()).expect("convert"));
        assert!(content.contains(" c"), "arcs flatten to cubic operators");
        assert!(!content.contains('A'), "no arc operator survives");
    }

    #[test]
    fn numbers_strip_trailing_zeros_and_negative_zero() {
        assert_eq!(num(2.0), "2");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(f64::NAN), "0");
    }
}
