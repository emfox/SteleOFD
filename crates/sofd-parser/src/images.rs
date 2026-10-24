//! Wiring the image resources a document declares to the bytes inside the
//! archive. The display list only records which resource a placement names;
//! the encoded bytes and their encoding are recovered here.

use std::collections::HashMap;
use std::sync::Arc;

use sofd_display::ImageCodec;
use sofd_package::Package;

use crate::content;

/// One image the document declares, ready to hand to a backend.
pub struct ImageResource {
    /// Encoded bytes as stored in the package.
    pub data: Arc<Vec<u8>>,
    /// Encoding, guessed from the media file name.
    pub codec: ImageCodec,
}

/// The image resources of one document, keyed by resource id.
pub type ImageTable = HashMap<String, ImageResource>;

/// Read the `MultiMedia` tables of a document and load the files they name.
///
/// The two `Res` tables sit beside `Document.xml`; an absent one is skipped.
pub fn load_images(package: &mut Package, doc_dir: &str) -> sofd_error::Result<ImageTable> {
    let mut table = ImageTable::new();
    for name in [sofd_common::DOCUMENT_RES_XML, sofd_common::PUBLIC_RES_XML] {
        let path = sofd_package::join(doc_dir, name);
        let Some(xml) = package.read_optional(&path)? else {
            continue;
        };
        let Ok(text) = String::from_utf8(xml) else {
            continue;
        };
        let list = content::read_multimedias(&text)?;
        let base = sofd_package::join(doc_dir, &res_base_loc(&text));
        for media in list.items {
            // Only images carry bytes a backend can paint; audio and video
            // are action targets, not placements.
            if media.kind != sofd_model::MultimediaKind::Image || media.media_file.is_empty() {
                continue;
            }
            let path = sofd_package::resolve(&base, &media.media_file);
            let Some(data) = package.read_optional(&path)? else {
                continue;
            };
            if data.is_empty() {
                continue;
            }
            table.insert(
                media.id,
                ImageResource {
                    codec: ImageCodec::from_file_name(&media.media_file),
                    data: Arc::new(data),
                },
            );
        }
    }
    Ok(table)
}

/// The `BaseLoc` a `Res` table declares, defaulting to `Res` itself.
///
/// A scanner, not a full parse, so a malformed table costs nothing.
fn res_base_loc(xml: &str) -> String {
    content::res_base_loc(xml)
}

/// Fill in the bytes and encoding of every image placement on a page.
///
/// Flattening runs without the archive, so a placement holds an id and no data.
pub fn attach_images(page: &mut sofd_display::PageDisplay, table: &ImageTable) {
    for op in &mut page.ops {
        let sofd_display::DrawOp::Image { image, .. } = op else {
            continue;
        };
        let Some(res) = table.get(&image.id) else {
            continue;
        };
        image.data = res.data.clone();
        image.codec = res.codec;
        // Flattening has no bytes to measure, so it leaves the size at 1x1;
        // the placement transform is derived from it and would collapse.
        image.pixel_size = pixel_size(&res.data, res.codec).unwrap_or((1, 1));
    }
}

/// The pixel size of an image, read from its own header.
///
/// The reading is `sofd_io`'s, so a resource placed here is one the
/// rasteriser and the exporters can also read.
pub fn pixel_size(data: &[u8], codec: ImageCodec) -> Option<(u32, u32)> {
    sofd_io::pixel_size(data, codec)
}
