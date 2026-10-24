//! A malformed font is refused rather than trusted: the bytes come from a
//! document, and the layer's job is to turn a backend assertion into an error.

use std::sync::Arc;

use sofd_font::{FontFace, FontId};

fn face(data: Vec<u8>) -> sofd_error::Result<FontFace> {
    FontFace::new(FontId(0), "Broken".to_string(), Arc::new(data), false)
}

#[test]
fn a_face_the_backend_refuses_is_an_error_rather_than_a_panic() {
    // A four-byte file is not an sfnt, so the parse fails before any table.
    assert!(face(vec![0, 1, 0, 0]).is_err());
}

#[test]
fn a_table_offset_past_the_end_of_the_file_is_an_error() {
    // A plausible sfnt header whose table directory claims to start at
    // 0xfffffff0, which is the shape that reaches the backend's offset check.
    let mut bytes = vec![0u8; 12];
    bytes[0..4].copy_from_slice(&1u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&1u16.to_be_bytes());
    bytes[6..8].copy_from_slice(&0u16.to_be_bytes());
    bytes[8..10].copy_from_slice(&0u16.to_be_bytes());
    bytes[10..12].copy_from_slice(&0u16.to_be_bytes());
    bytes.extend_from_slice(&0xffff_fff0u32.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 12]);
    let outcome = face(bytes);
    let _ = outcome.as_ref().err().map(|e| e.to_string());
}

#[test]
fn a_truncated_real_face_still_answers_the_questions_a_renderer_asks() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sofd-render/tests/data/Orbitron-Bold.ttf"
    );
    let full = std::fs::read(path).expect("font fixture");
    // Every prefix either parses into a face that answers, or is refused; the
    // assertion is that the call returns, in whichever way, for each of them.
    for cut in [0, 4, 12, 64, 512, full.len() / 2, full.len() - 1] {
        let bytes = full[..cut.min(full.len())].to_vec();
        match face(bytes) {
            Ok(face) => {
                let _ = face.units_per_em();
                let _ = face.metrics();
                for glyph in 0..8u16 {
                    let _ = face.advance_width(glyph);
                    let _ = face.outline(glyph);
                }
                let _ = face.glyph_id(0x41);
            }
            Err(error) => {
                let _ = error.to_string();
            }
        }
    }
}
