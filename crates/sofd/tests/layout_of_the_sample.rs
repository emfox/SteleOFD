//! What the sample that renders but lays out wrong actually places. It is a
//! VAT invoice laid out by offsets rather than by a face, so the display list,
//! not a raster, is the witness. The sample is fetched; these skip when absent.

use std::path::PathBuf;

use sofd_display::DrawOp;
use sofd_display::text::GlyphRun;

/// The sample, or nothing when it has not been fetched. CI fetches it, so a
/// skip there is a fetched file that went missing rather than a quiet pass.
fn sample() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ofdrw/999.ofd");
    // The existence test comes first: canonicalising a missing file answers
    // `None`, which would return above the guard and pass without reading it.
    if !path.is_file() {
        if std::env::var_os("CI").is_some() {
            panic!("{} is missing on a CI run", path.display());
        }
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

/// The open display list of a page, or nothing when the sample is absent.
fn page(index: usize) -> Option<sofd_display::PageDisplay> {
    let data = std::fs::read(sample()?).ok()?;
    let doc = sofd::Document::open_bytes(&data).expect("the sample opens");
    // A fetched sample that stops flattening is the failure these tests exist
    // to catch, so an unreadable page may not come back as a skip.
    let displayed = doc.page(index).cloned();
    assert!(
        displayed.is_some() || std::env::var_os("CI").is_none_or(|v| v.is_empty()),
        "page {index} of the sample did not flatten"
    );
    displayed
}

/// The rows of the tax control code, top to bottom. The code is drawn as four
/// runs now that a row return breaks the line, and it is the only text on the
/// page that comes in runs of this size.
fn rows(page: &sofd_display::PageDisplay) -> Vec<GlyphRun> {
    let mut found: Vec<GlyphRun> = page
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) if run.glyphs.len() == 28 => Some(run.clone()),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| a.transform.f.total_cmp(&b.transform.f));
    found
}

fn starts(run: &GlyphRun) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut at = 0.0;
    for i in 0..run.glyphs.len() {
        if i > 0 {
            at += run.advances.get(i - 1).copied().unwrap_or(0.0);
        }
        at += run.letter_spacing;
        out.push(run.transform.apply(at, 0.0));
    }
    out
}

fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Whether two placements agree to a hundredth of a millimetre, which is as
/// close as a page's own numbers and a float walk over them ever get.
fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 0.005
}

#[test]
fn the_invoice_code_is_one_row_a_column_per_glyph() {
    // `DeltaX="g 11 1.5875 -17.4625"` is eleven steps of 1.5875 and a fall back
    // to the left edge. A step reading that mishandled either half would put
    // the digits on top of each other or scatter them across the page.
    let page = page(0).expect("the sample is fetched");
    // Twelve glyphs in one run: the code is the only one on the page.
    let run = page
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) if run.glyphs.len() == 12 => Some(run.clone()),
            _ => None,
        })
        .next()
        .expect("the invoice code is placed");
    let placed = starts(&run);
    assert_eq!(placed.len(), 12, "eleven digits and no more");
    // The code sits at the object's own origin and never leaves its row.
    let row = placed[0].1;
    for (i, (x, y)) in placed.iter().enumerate() {
        assert!(near(*x, i as f64 * 1.5875), "digit {i} column: {x}");
        assert_eq!(*y, row, "digit {i} stays on the row");
    }
}

#[test]
fn a_date_drawn_character_by_character_keeps_its_own_characters() {
    // The date is written with a column of its own between each pair of digits
    // and month character, which is what a producer writes when a character
    // does not take the width of the digits beside it.
    let page = page(0).expect("the sample is fetched");
    // Several runs hold eleven glyphs; the date is the narrowest of them, since
    // its last column takes the face width rather than a step of its own.
    let run = page
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) if run.glyphs.len() == 11 => Some(run.clone()),
            _ => None,
        })
        .min_by(|a, b| {
            let width = |r: &GlyphRun| r.advances.iter().sum::<f64>() - r.advances[0];
            width(a).total_cmp(&width(b))
        })
        .expect("the date is placed");
    let placed = starts(&run);
    let columns: Vec<f64> = placed.iter().map(|(x, _)| round(x / 1.5875)).collect();
    // The offsets run one column per glyph, so the date reads straight across
    // and every step is one column but the two that a character takes.
    assert!(
        columns.is_sorted(),
        "the date reads left to right: {columns:?}"
    );
    let room: Vec<f64> = columns.windows(2).map(|w| round(w[1] - w[0])).collect();
    assert!(
        room.iter().any(|step| *step > 1.0),
        "a character is given room of its own: {room:?}"
    );
}

#[test]
fn a_tax_control_code_of_a_hundred_and_twelve_characters_runs_off_its_box() {
    // 112 characters stepped 2.5 mm. DeltaX gives 27 steps and a return of the
    // row's own 27 * 2.5, so each row starts where the first did, and DeltaY
    // drops it 4.5. A return is a step no single run carries, so rows.
    let page = page(0).expect("the sample is fetched");
    let rows = rows(&page);
    assert_eq!(rows.len(), 4, "the code is set in four rows");
    let total: usize = rows.iter().map(|run| run.glyphs.len()).sum();
    assert_eq!(total, 112, "every character of the code reaches the page");
    let mut fell = 0.0;
    for (index, run) in rows.iter().enumerate() {
        let placed = starts(run);
        assert_eq!(placed.len(), 28, "row {index} holds the row's characters");
        assert!(
            near(placed[0].0, 3.0),
            "row {index} starts at the left of the code: {:?}",
            placed[0]
        );
        assert!(
            near(placed[0].1, 4.732 + fell),
            "row {index} sits below the one above it by 4.5: {:?}",
            placed[0]
        );
        let span = placed.last().expect("glyphs").0 - placed[0].0;
        assert!(near(span, 27.0 * 2.5), "a column per character, got {span}");
        fell += 4.5;
    }
}

#[test]
fn the_labels_of_the_template_keep_the_kerning_the_producer_wrote() {
    // The document sets one glyph 15.57 mm from the next, an artefact of how
    // the form was drawn. It is placed as written: a layout that tidies a label
    // is no longer a rendering of the file.
    let page = page(0).expect("the sample is fetched");
    // Two labels are set this way, one in each party's block.
    let wide: Vec<GlyphRun> = page
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::Text(run) if run.glyphs.len() == 3 => Some(run.clone()),
            _ => None,
        })
        .filter(|run| {
            let placed = starts(run);
            near(placed[1].0 - placed[0].0, 15.57) && near(placed[2].0 - placed[1].0, 3.175)
        })
        .collect();
    assert_eq!(wide.len(), 2, "the buyer and the seller row");
}
