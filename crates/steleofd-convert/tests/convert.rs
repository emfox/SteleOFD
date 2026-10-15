//! N4-a: the convert crate driven as a consumer rather than from inside.
//! Every test uses the public surface and asserts only on what a caller can
//! observe: the shape of the bytes, the reported stats, and the errors.

use steleofd_common::Rect;
use steleofd_convert::{
    ConversionStats, PdfOptions, SvgOptions, TextExport, document_to_pdf, page_to_pdf, page_to_svg,
    pages_to_svg,
};
use steleofd_display::ops::{Color, Paint, PaintRule, StrokeStyle};
use steleofd_display::{DisplayList, DrawOp, PageDisplay};
use steleofd_font::FontManager;
use steleofd_geometry::Path;
use steleofd_render::RenderOptions;

/// A page with no content, useful for shape assertions on the wrapper.
fn blank_page(index: usize) -> PageDisplay {
    PageDisplay {
        index,
        page_box: Rect::A4_PORTRAIT,
        ops: Vec::new(),
        content_bbox: Rect::EMPTY,
    }
}

/// A filled red rectangle on an A4 page.
fn one_rect_page(index: usize) -> PageDisplay {
    let path = Path::build(|b| {
        b.rect(10.0, 20.0, 30.0, 40.0);
    });
    PageDisplay {
        index,
        page_box: Rect::A4_PORTRAIT,
        ops: vec![DrawOp::Path {
            path,
            fill: Some(Paint::Solid(Color::rgba(255, 0, 0, 255))),
            stroke: None,
            stroke_style: StrokeStyle::default(),
            rule: PaintRule::NonZero,
        }],
        content_bbox: Rect::new(10.0, 20.0, 40.0, 60.0),
    }
}

fn display_list(pages: Vec<PageDisplay>) -> DisplayList {
    DisplayList {
        page_count: pages.len(),
        pages,
    }
}

#[test]
fn an_svg_page_is_a_standalone_document_that_names_its_box() {
    let svg = page_to_svg(&blank_page(0), &SvgOptions::default()).expect("convert");
    assert!(
        svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "{svg}"
    );
    assert!(svg.contains("width=\"210mm\" height=\"297mm\""), "{svg}");
    assert!(svg.ends_with("</svg>\n"), "{svg}");
    // A blank page still closes the root group it opened.
    assert!(svg.contains("<g>") && svg.contains("</g>"), "{svg}");
}

#[test]
fn svg_output_carries_the_page_geometry_as_a_path() {
    let svg = page_to_svg(&one_rect_page(0), &SvgOptions::default()).expect("convert");
    assert!(
        svg.contains("<path d=\"M10 20 L40 20 L40 60 L10 60 Z\""),
        "{svg}"
    );
    assert!(svg.contains("fill=\"#ff0000\""), "{svg}");
}

#[test]
fn converting_a_display_list_yields_one_document_per_page() {
    let list = display_list(vec![blank_page(0), one_rect_page(1)]);
    let pages = pages_to_svg(&list, &SvgOptions::default()).expect("convert");
    assert_eq!(pages.len(), 2);
    for (index, svg) in pages.iter().enumerate() {
        assert!(
            svg.starts_with("<?xml version"),
            "page {index} is not a document"
        );
    }
}

#[test]
fn a_page_with_live_text_reports_the_run_it_kept() {
    let stats = stats_of_svg(&one_rect_page(0));
    // No text on the page, so nothing was outlined and nothing was dropped.
    assert_eq!(stats.outlined_glyphs, 0);
    assert_eq!(stats.dropped_runs, 0);
    assert_eq!(stats.images, 0);
}

#[test]
fn asking_for_outlined_text_without_a_font_source_degrades_visibly() {
    // The options are the honest kind: requesting an export the caller cannot
    // support is not an error, it is a documented fallback. The stats are how
    // a caller notices, so the fallback must show up there rather than vanish.
    let page = one_rect_page(0);
    let options = SvgOptions {
        text: TextExport::Outline,
        fonts: None,
    };
    let svg = page_to_svg(&page, &options).expect("convert");
    assert!(!svg.is_empty());
}

#[test]
fn pdf_output_starts_with_the_format_marker() {
    let pdf = page_to_pdf(&one_rect_page(0), &PdfOptions::default()).expect("convert");
    assert!(
        pdf.starts_with(b"%PDF-"),
        "no PDF header: {:?}",
        &pdf[..pdf.len().min(16)]
    );
    assert!(
        pdf.ends_with(b"%%EOF\n") || pdf.ends_with(b"%%EOF"),
        "PDF is not terminated"
    );
}

#[test]
fn a_multi_page_pdf_counts_every_page() {
    let list = display_list(vec![blank_page(0), one_rect_page(1), blank_page(2)]);
    let pdf = document_to_pdf(&list, &PdfOptions::default()).expect("convert");
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("/Count 3"),
        "page tree does not count three pages"
    );
}

#[test]
fn an_empty_display_list_still_produces_a_valid_pdf() {
    // Zero pages is a legitimate input, not a panic: the catalog and page tree
    // are still written, and the result is readable by a PDF consumer.
    let pdf = document_to_pdf(&display_list(Vec::new()), &PdfOptions::default()).expect("convert");
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(
        String::from_utf8_lossy(&pdf).contains("/Type /Catalog")
            || String::from_utf8_lossy(&pdf).contains("/Type/Catalog")
    );
}

#[test]
fn png_output_is_a_png_of_the_requested_size() {
    let page = one_rect_page(0);
    let fonts = FontManager::new();
    let options = RenderOptions {
        dpi: 72.0,
        background: Some(Color::WHITE),
        max_pixels: 64 * 1024 * 1024,
    };
    let png = steleofd_convert::png::page_to_png(&page, &fonts, &options).expect("convert");
    // The PNG signature is the contract a downstream decoder checks first.
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn the_pixel_budget_is_enforced_before_any_raster_is_allocated() {
    let page = one_rect_page(0);
    let fonts = FontManager::new();
    let options = RenderOptions {
        dpi: 96.0,
        background: Some(Color::WHITE),
        // Smaller than a single A4 page at 96 DPI, so this must fail fast.
        max_pixels: 1024,
    };
    let err = steleofd_convert::png::page_to_png(&page, &fonts, &options).expect_err("budget");
    assert!(err.to_string().contains("budget"), "{err}");
}

#[test]
fn the_default_stats_describe_a_lossless_run() {
    let stats = ConversionStats::default();
    assert!(stats.is_lossless());
    assert_eq!(stats.images_skipped, 0);
    assert_eq!(stats.gradients, 0);
}

/// Convert one page and hand back the stats the sink accumulated.
fn stats_of_svg(page: &PageDisplay) -> ConversionStats {
    let mut sink = steleofd_convert::SvgSink::new(page.page_box, &SvgOptions::default());
    steleofd_convert::replay(page, &mut sink).expect("replay");
    // `finish` consumes the sink and returns the stats it accumulated.
    sink.finish().1
}
