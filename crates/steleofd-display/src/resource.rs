//! Resources resolved into a form backends can consume directly.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use steleofd_common::Rect;
use steleofd_font::FontId;

/// Font data ready for glyph rasterization by a backend.
#[derive(Clone)]
pub struct ResolvedFont {
    /// Globally unique font id.
    pub id: FontId,
    /// Font family name.
    pub family: String,
    /// Whether the face covers CJK.
    pub is_cjk: bool,
    /// Font binary, either embedded in the document or supplied by a system face.
    pub data: Arc<Vec<u8>>,
    /// Whether this is a substituted system font rather than embedded data.
    pub is_fallback: bool,
}

impl std::fmt::Debug for ResolvedFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedFont")
            .field("id", &self.id)
            .field("family", &self.family)
            .field("is_cjk", &self.is_cjk)
            .field("bytes", &self.data.len())
            .field("is_fallback", &self.is_fallback)
            .finish()
    }
}

/// Reference to an image resource, decoded lazily by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    /// Resource id as it appears in the OFD `ID` attribute.
    pub id: String,
    /// Encoded bytes in the original format.
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
    /// Intrinsic pixel dimensions.
    pub pixel_size: (u32, u32),
    /// Nominal placement size on the page, in millimetres.
    pub nominal_size: Rect,
    /// Encoding of `data`.
    pub codec: ImageCodec,
}

impl ImageRef {
    /// Build a reference from an id and encoded bytes.
    pub fn new(
        id: impl Into<String>,
        data: Arc<Vec<u8>>,
        pixel_size: (u32, u32),
        codec: ImageCodec,
    ) -> Self {
        Self {
            id: id.into(),
            data,
            pixel_size,
            nominal_size: Rect::default(),
            codec,
        }
    }
}

/// Supported image encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageCodec {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// TIFF.
    Tiff,
    /// BMP.
    Bmp,
    /// JBIG2, common for bi-level scanned pages where it saves a lot of space.
    Jbig2,
    /// Uncompressed bitmap.
    Raw,
}

impl ImageCodec {
    /// Guess the encoding from a file name, the way the package does when it
    /// classifies an entry. Unknown names read as `Raw`, which no decoder
    /// claims, so a wrong guess fails loudly instead of painting noise.
    pub fn from_file_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        let ext = lower.rsplit('.').next().unwrap_or_default();
        match ext {
            "jpg" | "jpeg" => Self::Jpeg,
            "png" => Self::Png,
            "tif" | "tiff" => Self::Tiff,
            "bmp" => Self::Bmp,
            "jbig2" | "jb2" => Self::Jbig2,
            _ => Self::Raw,
        }
    }

    /// Whether a decoder exists for this encoding.
    pub fn is_decodable(self) -> bool {
        !matches!(self, Self::Raw)
    }
}

/// Everything one page needs, already resolved.
#[derive(Debug, Default)]
pub struct ResolvedResources {
    /// Font table indexed by [`steleofd_font::FontId`].
    pub fonts: Vec<ResolvedFont>,
    /// Image table keyed by resource id.
    pub images: Vec<ImageRef>,
}
