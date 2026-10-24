#![no_main]

//! Font bytes arrive from the document, so the reader must answer every input
//! with a face or an error. A panic here would take the reader down on a file
//! it was asked to display, and the parser is reachable from every page.

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(face) = sofd_font::FontFace::new(
        sofd_font::FontId(0),
        "Fuzz".to_string(),
        Arc::new(data.to_vec()),
        false,
    ) else {
        return;
    };
    // A face that parsed is then asked the questions the renderer asks it,
    // which is where a table the header promised but the file omits shows up.
    let _ = face.units_per_em();
    let _ = face.metrics();
    for glyph in 0..64u16 {
        let _ = face.advance_width(glyph);
        let _ = face.outline(glyph);
        let _ = face.glyph_outline(glyph);
        let _ = face.reverse_cmap(glyph);
    }
    for codepoint in [0x41, 0x4e2d, 0x1f600, 0x10ffff] {
        if let Some(glyph) = face.glyph_id(codepoint) {
            let _ = face.outline(glyph);
        }
    }
    let _ = face.covers(&[0x41, 0x4e2d]);
});
