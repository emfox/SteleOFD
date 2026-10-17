//! Wiring the font resources a document declares to the bytes in the archive.
//! A text run's `Font` names an id, and only the declaration says which file
//! that id means; pairing them by position silently paints an unrelated face.

use std::collections::HashMap;
use std::sync::Arc;

use steleofd_font::FontTraits;
use steleofd_package::Package;

use crate::content;

/// One font the document declares, ready to hand to a registry.
pub struct FontResource {
    /// Container path of the font file, empty when none was shipped.
    pub path: String,
    /// Name the declaration carries, for family matching.
    pub family: String,
    /// Encoded bytes as stored in the package; empty when none was shipped.
    pub data: Arc<Vec<u8>>,
    /// Whether the package actually carried the file. A declaration without a
    /// `FontFile`, or whose file failed to read, still names a face the reader
    /// is expected to find itself; clauses 11.1 and 11.4 say so.
    pub embedded: bool,
    /// What the declaration said about the face, for matching a substitute.
    pub traits: FontTraits,
}

/// The fonts a document declares, keyed by resource id.
pub type FontTable = HashMap<u32, FontResource>;

/// Read the `Fonts` tables of a document and load the files they name. An
/// absent table is skipped; a font with no readable file is kept unembedded.
pub fn load_fonts(package: &mut Package, doc_dir: &str) -> steleofd_error::Result<FontTable> {
    let mut table = FontTable::new();
    for name in [
        steleofd_common::DOCUMENT_RES_XML,
        steleofd_common::PUBLIC_RES_XML,
    ] {
        let path = steleofd_package::join(doc_dir, name);
        let Some(xml) = package.read_optional(&path)? else {
            continue;
        };
        let Ok(text) = String::from_utf8(xml) else {
            continue;
        };
        let list = content::read_fonts(&text)?;
        let base = steleofd_package::join(doc_dir, content::res_base_loc(&text));
        for font in list.items {
            let Ok(id) = font.id.parse::<u32>() else {
                continue;
            };
            let traits = FontTraits {
                family: font.family_name.clone(),
                charset: font.charset.clone(),
                serif: font.serif,
                bold: font.bold,
                italic: font.italic,
                fixed_width: font.fixed_width,
            };
            let family = match font.family_name.clone().filter(|f| !f.is_empty()) {
                Some(family) => family,
                None => font.font_name.clone(),
            };
            let loaded = if font.font_file.is_empty() {
                None
            } else {
                let path = steleofd_package::resolve(&base, &font.font_file);
                package
                    .read_optional(&path)?
                    .filter(|data| !data.is_empty())
                    .map(|data| (path, data))
            };
            table.insert(
                id,
                match loaded {
                    Some((path, data)) => FontResource {
                        path,
                        family,
                        data: Arc::new(data),
                        embedded: true,
                        traits,
                    },
                    None => FontResource {
                        path: String::new(),
                        family,
                        data: Arc::new(Vec::new()),
                        embedded: false,
                        traits,
                    },
                },
            );
        }
    }
    Ok(table)
}
