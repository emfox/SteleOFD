//! The golden-image suite: a document's exact pixels, committed as a PNG.
//! Every other suite asserts the ops a page emits, so a backend that paints
//! them wrong still passes. Set `BLESS=1` to rewrite the references.

use std::path::PathBuf;

use image::RgbaImage;
use steleofd_render::RenderOptions;
use steleofd_testkit::{DOCS, GoldenDoc, golden_archive};

/// One pixel channel this far off is still the same picture: anti-aliasing and
/// the odd rounding difference must not make a golden test flaky.
const CHANNEL_TOLERANCE: i32 = 4;
/// How much of the page may differ before it is a regression. A one-pixel
/// border shifting should not fail; a lost clip or a flat gradient should.
const MAX_DIFFERING_RATIO: f64 = 0.005;

/// Where the committed references live, beside the fixtures the other suites use.
fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden")
}

/// How a golden document is rendered: the default DPI, no background fill so
/// the comparison sees only what the page painted, and the default budget.
const RENDER_OPTIONS: RenderOptions = RenderOptions {
    dpi: 96.0,
    background: None,
    max_pixels: 64 * 1024 * 1024,
};

/// Render one golden document to PNG bytes, through the same path a caller uses.
fn render(doc: &GoldenDoc) -> Vec<u8> {
    let bytes = golden_archive(doc);
    let document = steleofd_core::Document::open_bytes(&bytes).expect("open the golden archive");
    document
        .to_png(0, &RENDER_OPTIONS)
        .unwrap_or_else(|e| panic!("render {}: {e}", doc.name))
}

/// Decode a PNG to its raw RGBA pixels, failing loudly on a bad reference.
fn decode(png: &[u8], what: &str) -> RgbaImage {
    image::load_from_memory(png)
        .unwrap_or_else(|e| panic!("decode {what}: {e}"))
        .to_rgba8()
}

/// The share of pixels that differ by more than the channel tolerance.
fn difference_ratio(expected: &RgbaImage, actual: &RgbaImage) -> f64 {
    if expected.dimensions() != actual.dimensions() {
        return 1.0;
    }
    let total = (expected.width() * expected.height()) as f64;
    let mut differing = 0u64;
    for (e, a) in expected.pixels().zip(actual.pixels()) {
        let off =
            e.0.iter()
                .zip(a.0.iter())
                .map(|(x, y)| (*x as i32 - *y as i32).abs())
                .max()
                .unwrap_or(0);
        if off > CHANNEL_TOLERANCE {
            differing += 1;
        }
    }
    differing as f64 / total
}

/// Compare one document against its reference, writing it when `BLESS` is set.
fn check_one(doc: &GoldenDoc) {
    let actual_png = render(doc);
    let reference = reference_dir().join(format!("{}.png", doc.name));

    if std::env::var_os("BLESS").is_some() {
        std::fs::create_dir_all(reference_dir()).expect("create the reference directory");
        std::fs::write(&reference, &actual_png).expect("write the reference");
        return;
    }

    let expected_png = std::fs::read(&reference).unwrap_or_else(|e| {
        panic!(
            "read the reference {}: {e}\nRun with BLESS=1 to write it.",
            reference.display()
        )
    });
    let expected = decode(&expected_png, "the reference");
    let actual = decode(&actual_png, "the rendering");

    let ratio = difference_ratio(&expected, &actual);
    assert!(
        ratio <= MAX_DIFFERING_RATIO,
        "{} differs from its reference in {:.2}% of pixels (allowed {:.2}%): \
         the backend drifted, or the reference needs a deliberate `BLESS=1`",
        doc.name,
        ratio * 100.0,
        MAX_DIFFERING_RATIO * 100.0,
    );
}

#[test]
fn every_golden_document_matches_its_committed_pixels() {
    for doc in DOCS {
        check_one(doc);
    }
}

#[test]
fn the_golden_documents_render_to_distinct_pictures() {
    // A comparator that passed everything would pass a suite where all three
    // documents rendered the same. This asserts they do not, so the reference
    // set is known to be discriminating rather than merely green.
    let rendered: Vec<RgbaImage> = DOCS
        .iter()
        .map(|doc| decode(&render(doc), doc.name))
        .collect();
    for (i, left) in rendered.iter().enumerate() {
        for (j, right) in rendered.iter().enumerate().skip(i + 1) {
            let ratio = difference_ratio(left, right);
            assert!(
                ratio > MAX_DIFFERING_RATIO,
                "{} and {} render alike ({:.2}%), so the set cannot catch a regression",
                DOCS[i].name,
                DOCS[j].name,
                ratio * 100.0,
            );
        }
    }
}

#[test]
fn a_reference_png_is_readable_at_the_expected_size() {
    let doc = &DOCS[0];
    let actual = decode(&render(doc), doc.name);
    // A 100x60mm page at 96 DPI is 378x227 pixels, rounded up by `size_px`.
    assert_eq!(actual.width(), 378, "width");
    assert_eq!(actual.height(), 227, "height");
}
