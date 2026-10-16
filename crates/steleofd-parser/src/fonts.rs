//! Wiring the font resources a document declares to the bytes in the archive.
//! A text run's `Font` names an id, and only the declaration says which file
//! that id means; pairing them by position silently paints an unrelated face.

use std::collections::HashMap;
use std::sync::Arc;

use steleofd_package::Package;

use crate::content;

/// One font the document declares, ready to hand to a registry.
pub struct FontResource {
    /// Container path of the font file.
    pub path: String,
    /// Name the declaration carries, for family matching.
    pub family: String,
    /// Encoded bytes as stored in the package.
    pub data: Arc<Vec<u8>>,
}

/// The fonts a document declares, keyed by resource id.
pub type FontTable = HashMap<u32, FontResource>;

/// Read the `Fonts` tables of a document and load the files they name.
/// An absent table is skipped, and so is a font with no file to load.
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
            if font.font_file.is_empty() {
                continue;
            }
            let Ok(id) = font.id.parse::<u32>() else {
                continue;
            };
            let path = steleofd_package::resolve(&base, &font.font_file);
            let Some(data) = package.read_optional(&path)? else {
                continue;
            };
            if data.is_empty() {
                continue;
            }
            table.insert(
                id,
                FontResource {
                    path,
                    family: font.font_name,
                    data: Arc::new(data),
                },
            );
        }
    }
    Ok(table)
}
