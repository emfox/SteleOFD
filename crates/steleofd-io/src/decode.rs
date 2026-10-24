//! Decoding an image resource, so that every reader of one agrees.
//! OFD packs an image in whichever format the producer chose, and the formats
//! split in two: those the `image` crate reads, and JBIG2, which it does not.
//! The parser measures the bytes, the rasteriser paints them and the exporters
//! embed them, so the reading is here. A format only some of them knew would
//! still render and then refuse to export.

use steleofd_display::ImageCodec;

/// The pixel size of an image, read from its own header rather than from OFD's
/// `<Size>`, which a placement may scale.
///
/// JBIG2 keeps its dimensions in a segment rather than a magic number, so it is
/// asked first instead of being left to `image` to fail on.
pub fn pixel_size(data: &[u8], codec: ImageCodec) -> Option<(u32, u32)> {
    let by_codec = match codec {
        ImageCodec::Jbig2 => jbig2::dimensions(data),
        _ => None,
    };
    match by_codec {
        Some(size) => Some(size),
        None => image::load_from_memory(data)
            .ok()
            .map(|i| (i.width(), i.height())),
    }
}

/// Decode an image resource into the pixels a consumer asked for.
///
/// `Pixels` says which layout is wanted rather than leaving the caller to
/// reshape the bytes: a raster backend wants RGBA, an exporter embedding a PDF
/// image XObject wants RGB, and both used to spell out the expansion
/// themselves.
pub fn decode(data: &[u8], codec: ImageCodec, pixels: Pixels) -> Result<Decoded, String> {
    if codec == ImageCodec::Jbig2 {
        let gray = jbig2::decode_to_gray(data)?;
        let (width, height) = jbig2::dimensions(data).ok_or("JBIG2 has no dimensions")?;
        if gray.len() != width as usize * height as usize {
            return Err(format!(
                "JBIG2 declares {width}x{height} but decoded {} bytes",
                gray.len()
            ));
        }
        let bytes = match pixels {
            Pixels::Rgb => expand(&gray, 3),
            Pixels::Rgba => expand(&gray, 4),
        };
        return Ok(Decoded {
            bytes,
            width,
            height,
        });
    }
    let read = image::load_from_memory(data).map_err(|e| e.to_string())?;
    // An RGB image is written out as it is: premultiplying is the raster
    // backend's need, and an exporter embedding the pixels has nowhere to put
    // the alpha it would be multiplying by.
    if pixels == Pixels::Rgb {
        let rgb = read.to_rgb8();
        return Ok(Decoded {
            width: rgb.width(),
            height: rgb.height(),
            bytes: rgb.into_raw(),
        });
    }
    let rgba = read.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    let bytes = rgba.pixels().flat_map(|p| premultiply(p.0)).collect();
    Ok(Decoded {
        bytes,
        width,
        height,
    })
}

/// Which layout a decode produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixels {
    /// Three channels, no alpha: what an image XObject embedded in a PDF holds.
    Rgb,
    /// Four channels, premultiplied: what a raster backend blits.
    Rgba,
}

/// One decoded image, row-major, with the size it actually came out as.
///
/// The size travels with the bytes because a stream may declare a different one
/// from what it holds, and a caller that assumed the declared size would read
/// past the buffer.
#[derive(Clone, Debug)]
pub struct Decoded {
    /// The pixels, in the layout the call asked for.
    pub bytes: Vec<u8>,
    /// Columns.
    pub width: u32,
    /// Rows.
    pub height: u32,
}

/// Repeat each grey sample into an RGB or RGBA pixel.
///
/// A bi-level page has only two values, so the three colour channels are the
/// same byte; the fourth is opaque.
fn expand(gray: &[u8], channels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(gray.len() * channels);
    for &v in gray {
        out.extend_from_slice(&[v, v, v]);
        if channels == 4 {
            out.push(u8::MAX);
        }
    }
    out
}

/// Scale a colour channel by its alpha, the form a raster canvas expects.
///
/// An opaque pixel is returned untouched, which is every pixel of a JPEG and
/// most of a scan.
pub fn premultiply(px: [u8; 4]) -> [u8; 4] {
    let a = px[3] as u32;
    if a == 255 {
        return px;
    }
    let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
    [m(px[0]), m(px[1]), m(px[2]), px[3]]
}

pub mod jbig2;
