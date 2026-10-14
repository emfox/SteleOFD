//! Entry classification.
//! OFD archives follow a fixed directory convention, so an entry's role can be
//! derived from its path instead of being recorded in a manifest.

use std::fmt;

/// What an archive entry holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// Top-level `OFD.xml`, the entry point.
    EntryPoint,
    /// A per-document `Document.xml`.
    Document,
    /// A page description under `Pages/`.
    Page,
    /// A template under `Tpls/`.
    Template,
    /// A resource container under `Res/`.
    Resource,
    /// A font program.
    Font,
    /// An image.
    Image,
    /// A signature description or value.
    Signature,
    /// Anything else, kept so unknown extensions survive a round trip.
    Other,
}

impl EntryKind {
    /// Classify by path.
    pub fn from_path(path: &str) -> Option<Self> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let lower = name.to_ascii_lowercase();

        if path == crate::ENTRY_POINT {
            return Some(Self::EntryPoint);
        }
        if name == "Document.xml" {
            return Some(Self::Document);
        }
        if name == "Signatures.xml" || name == "Signature.xml" {
            return Some(Self::Signature);
        }

        // Extension wins over directory, since a document may store an image
        // anywhere and the bytes are what matter for decoding.
        if matches!(
            extension(&lower).as_deref(),
            Some("ttf" | "otf" | "ttc" | "woff" | "woff2")
        ) {
            return Some(Self::Font);
        }
        if matches!(
            extension(&lower).as_deref(),
            Some("png" | "jpg" | "jpeg" | "jbig2" | "jb2" | "bmp" | "tif" | "tiff" | "gif")
        ) {
            return Some(Self::Image);
        }

        if path.contains("/Pages/") || path.starts_with("Pages/") {
            return Some(Self::Page);
        }
        if path.contains("/Tpls/") || path.starts_with("Tpls/") {
            return Some(Self::Template);
        }
        if path.contains("/Res/") || path.starts_with("Res/") {
            return Some(Self::Resource);
        }

        Some(Self::Other)
    }

    /// Whether the entry is binary rather than XML text.
    pub fn is_binary(self) -> bool {
        matches!(self, Self::Font | Self::Image)
    }

    /// Whether the entry holds XML.
    pub fn is_xml(self) -> bool {
        !self.is_binary() && !matches!(self, Self::Other)
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::EntryPoint => "entry-point",
            Self::Document => "document",
            Self::Page => "page",
            Self::Template => "template",
            Self::Resource => "resource",
            Self::Font => "font",
            Self::Image => "image",
            Self::Signature => "signature",
            Self::Other => "other",
        };
        f.write_str(name)
    }
}

/// Description of one entry, useful for listings and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    /// Archive path.
    pub path: String,
    /// Derived role.
    pub kind: EntryKind,
    /// Decompressed size in bytes.
    pub size: u64,
}

impl EntryInfo {
    /// Build a description.
    pub fn new(path: impl Into<String>, size: u64) -> Self {
        let path = path.into();
        let kind = EntryKind::from_path(&path).unwrap_or(EntryKind::Other);
        Self { path, kind, size }
    }
}

impl fmt::Display for EntryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] {} bytes", self.path, self.kind, self.size)
    }
}

/// Lower-cased extension without the dot.
fn extension(name: &str) -> Option<String> {
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}
