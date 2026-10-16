//! Gradients plus the content-stream helpers the sink writes with.

use super::*;

pub(crate) fn channel_array(c: &Color) -> Object {
    Object::Array(
        channel_triplet(c)
            .into_iter()
            .map(|channel| Object::Real(channel as f32))
            .collect(),
    )
}

/// Build an image XObject from encoded bytes, dropping alpha. JBIG2 is not one
/// `image` reads, so a bi-level stream is decoded to grey first and expanded to
/// RGB, exactly as the raster backend does.
pub(crate) fn image_xobject(image: &PendingImage) -> Result<Stream> {
    let rgb = if image.codec == steleofd_display::ImageCodec::Jbig2 {
        let gray = steleofd_display::jbig2::decode_to_gray(&image.data)
            .map_err(|reason| convert_error("image", reason))?;
        gray.iter().flat_map(|&v| [v, v, v]).collect()
    } else {
        image::load_from_memory(&image.data)
            .map_err(|e| convert_error("image", e.to_string()))?
            .to_rgb8()
            .into_raw()
    };
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", image.pixels.0 as i64);
    dict.set("Height", image.pixels.1 as i64);
    dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    dict.set("BitsPerComponent", Object::Integer(8));
    let mut stream = Stream::new(dict, rgb);
    stream
        .compress()
        .map_err(|e| convert_error("image stream", e.to_string()))?;
    Ok(stream)
}

/// `r g b` operands for a colour, each channel `0..=1`.
pub(crate) fn rgb(c: &Color) -> String {
    format!(
        "{} {} {}",
        num(c.r as f64 / 255.0),
        num(c.g as f64 / 255.0),
        num(c.b as f64 / 255.0)
    )
}

/// `r g b` for the mean colour of a mesh, used where no shading can be written.
pub(crate) fn mesh_rgb(spec: &steleofd_display::MeshSpec) -> String {
    if spec.points.is_empty() {
        return "0 0 0".to_string();
    }
    let n = spec.points.len() as u32;
    let sum = spec.points.iter().fold((0u32, 0u32, 0u32), |acc, v| {
        (
            acc.0 + v.color.r as u32,
            acc.1 + v.color.g as u32,
            acc.2 + v.color.b as u32,
        )
    });
    let mean = Color::rgba((sum.0 / n) as u8, (sum.1 / n) as u8, (sum.2 / n) as u8, 255);
    rgb(&mean)
}

/// Channel triplet of a colour, for gradient function dictionaries.
pub(crate) fn channel_triplet(c: &Color) -> [f64; 3] {
    [c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0]
}

/// Stroke parameters, scaled by the transform in effect.
pub(crate) fn stroke_prefix(style: &StrokeStyle, ctm: &Affine) -> String {
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
pub(crate) fn point_op(point: (f64, f64), operator: &str) -> String {
    format!("{} {} {operator}", num(point.0), num(point.1))
}

/// A cubic Bezier operator with all three points mapped.
pub(crate) fn curve_op(total: Affine, c1: (f64, f64), c2: (f64, f64), to: (f64, f64)) -> String {
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
pub(crate) fn pdf_string(text: &str) -> String {
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
pub(crate) fn convert_error(object: &str, reason: String) -> OfdError {
    OfdError::Convert(ConvertError::MappingFailed {
        page: 0,
        object: object.to_string(),
        reason,
    })
}

/// Compact number: at most four decimals, no trailing zeros, no negative zero.
pub(crate) fn num(v: f64) -> String {
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
