//! JBIG2 decoding, for the bi-level images a scanned page carries: black text
//! on white, which the format compresses far better than any raster format.
//! Pure Rust with `simd` off, so nothing in its tree uses unsafe code.

use crate::resource::ImageCodec;

use hayro_jbig2::{Decoder, Image};

/// Decode JBIG2 into 8-bit greyscale, one byte per pixel, row-major, 1 black.
/// A packed bitmap would suit, but every backend wants RGBA downstream.
pub fn decode_to_gray(data: &[u8]) -> Result<Vec<u8>, String> {
    // A file carries a header naming the page count; an embedded stream is
    // bare segments. Both forms occur, so the cheap check decides which.
    let image = match Image::new(data) {
        Ok(image) => image,
        Err(_) => Image::new_embedded(data, None).map_err(|e| e.to_string())?,
    };
    let width = image.width();
    let height = image.height();
    let mut sink = Grayscale::new(width, height);
    image.decode(&mut sink).map_err(|e| e.to_string())?;
    Ok(sink.finish())
}

/// The pixel dimensions of a JBIG2 stream, without decoding it.
pub fn dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let image = Image::new(data)
        .or_else(|_| Image::new_embedded(data, None))
        .ok()?;
    Some((image.width(), image.height()))
}

/// Dimensions for any codec, so a caller need not match on it first.
pub fn dimensions_for(data: &[u8], codec: ImageCodec) -> Option<(u32, u32)> {
    match codec {
        ImageCodec::Jbig2 => dimensions(data),
        _ => None,
    }
}

/// Collects decoded pixels as one byte per pixel, in row order.
struct Grayscale {
    width: u32,
    height: u32,
    /// The line under construction; pushed pixels are appended here.
    row: Vec<u8>,
    /// Every finished line, in order.
    rows: Vec<u8>,
}

impl Grayscale {
    fn new(width: u32, height: u32) -> Self {
        // A hostile stream can declare an enormous page, so the buffer grows
        // on demand rather than being sized from the declared width.
        Self {
            width,
            height,
            row: Vec::new(),
            rows: Vec::new(),
        }
    }

    fn push(&mut self, black: bool) {
        self.row.push(if black { 0 } else { u8::MAX });
    }

    fn end_line(&mut self) {
        // A row the encoder left short is padded white rather than dropped,
        // which keeps the buffer rectangular.
        let width = self.width as usize;
        if self.row.len() < width {
            self.row.resize(width, u8::MAX);
        }
        self.rows.append(&mut self.row);
    }

    fn finish(mut self) -> Vec<u8> {
        if !self.row.is_empty() {
            self.end_line();
        }
        // A stream may stop short of the declared height, so pad the tail.
        self.rows
            .resize(self.width as usize * self.height as usize, u8::MAX);
        self.rows
    }
}

impl Decoder for Grayscale {
    fn push_pixel(&mut self, black: bool) {
        self.push(black);
    }

    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        // Whole chunks are uniform, so one resize beats a per-pixel loop.
        self.row.resize(
            self.row.len() + chunk_count as usize * 8,
            if black { 0 } else { u8::MAX },
        );
    }

    fn next_line(&mut self) {
        self.end_line();
    }
}
