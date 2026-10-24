//! Decoding an image resource, so that every reader of one agrees.
//! OFD packs an image in whichever format the producer chose. The parser
//! measures the bytes, the rasteriser paints them and the exporters embed them,
//! so the reading is here: a format only some of them knew would still render
//! and then refuse to export.

use steleofd_display::ImageCodec;

/// Decode an image resource into the pixels a consumer asked for.
///
/// `Pixels` says which layout is wanted rather than leaving the caller to
/// reshape the bytes: a raster backend wants RGBA, an exporter embedding a PDF
/// image XObject wants RGB, and both used to spell out the expansion
/// themselves.
pub fn decode(data: &[u8], codec: ImageCodec, pixels: Pixels) -> Result<Decoded, String> {
    let read = read(data, codec)?;
    let (width, height) = (read.width(), read.height());
    let bytes = match pixels {
        // An RGB image is written out as it is: premultiplying is the raster
        // backend's need, and an exporter embedding the pixels has nowhere to
        // put the alpha it would be multiplying by.
        Pixels::Rgb => read.to_rgb8().into_raw(),
        Pixels::Rgba => read
            .to_rgba8()
            .pixels()
            .flat_map(|p| premultiply(p.0))
            .collect(),
    };
    Ok(Decoded {
        bytes,
        width,
        height,
    })
}

/// The pixel size of an image, read from its own header rather than from OFD's
/// `<Size>`, which a placement may scale.
pub fn pixel_size(data: &[u8], codec: ImageCodec) -> Option<(u32, u32)> {
    read(data, codec)
        .ok()
        .map(|image| (image.width(), image.height()))
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

/// Read a resource with the `image` crate, one call per encoding.
///
/// A decode may only name the formats `image` spells out, so a decoder
/// registered under another name is out of reach through
/// `load_from_memory_with_format`; a named file reaches the same table by the
/// one route `image` leaves open. The rest name their format directly.
fn read(data: &[u8], codec: ImageCodec) -> Result<image::DynamicImage, String> {
    let fail = |e: image::ImageError| e.to_string();
    match codec {
        ImageCodec::Jbig2 => {
            hayro_jbig2::integration::register_decoding_hook();
            let path = named(data, "jbig2");
            // Unlinked once read, so a decode leaves nothing behind even if a
            // later step fails or the caller panics.
            let loaded = image::ImageReader::open(&path).and_then(|reader| {
                reader
                    .with_guessed_format()
                    .map_err(std::io::Error::other)
                    .and_then(|reader| reader.decode().map_err(std::io::Error::other))
            });
            let _ = std::fs::remove_file(&path);
            loaded.map_err(|e| e.to_string())
        }
        // Bare pixels carry no header, so `image` has nothing to read them by,
        // and naming one would paint noise. No sample uses a raw resource.
        ImageCodec::Raw => Err("bare pixels carry no header to read them by".to_string()),
        _ => image::load_from_memory_with_format(data, format_of(codec)).map_err(fail),
    }
}

/// The `image` format an OFD codec corresponds to.
///
/// A JPEG carries a format marker of its own and `image` reads it under any
/// name, which is why OFD's producers spell the same codec `jpg`, `jpeg` and
/// `JPG` without any of them failing.
fn format_of(codec: ImageCodec) -> image::ImageFormat {
    match codec {
        ImageCodec::Jpeg => image::ImageFormat::Jpeg,
        ImageCodec::Png => image::ImageFormat::Png,
        ImageCodec::Tiff => image::ImageFormat::Tiff,
        ImageCodec::Bmp => image::ImageFormat::Bmp,
        ImageCodec::Gif => image::ImageFormat::Gif,
        // Answered by `read` before this is reached, for the reason it gives.
        ImageCodec::Jbig2 | ImageCodec::Raw => image::ImageFormat::Png,
    }
}

/// Write the bytes to a temporary file whose extension names their format.
///
/// `image` reads an extension only from a path, so a decode that names a format
/// has to arrive as one.
fn named(data: &[u8], extension: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "steleofd-{}-{}.{extension}",
        std::process::id(),
        NAMED[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&path, data).expect("a temporary file for one decode");
    path
}

/// Counts the files this process has written, so two decodes racing in one
/// process do not share a name.
static NAMED: [std::sync::atomic::AtomicU64; 1] = [std::sync::atomic::AtomicU64::new(0)];

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
