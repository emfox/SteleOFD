//! What it costs to turn a flattened page into PNG, SVG and PDF.
//! A reader spends most of its time here on an opened document, so the three
//! backends are measured apart: they share the display list and nothing else.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sofd::Document;
use sofd_convert::png::page_to_png;
use sofd_convert::{PdfOptions, SvgOptions, TextExport, page_to_pdf, page_to_svg};
use sofd_render::RenderOptions;
use sofd_testkit::{OBJECTS_PER_PAGE, bench_archive};

/// Page counts to measure at. One page is a preview, eight is a document
/// somebody would export, and twenty is the packet the budgets are stated for.
const PAGES: &[usize] = &[1, 8, 20];

/// A document, opened and flattened with its fonts registered, which is the
/// state every conversion starts from.
fn fixture(pages: usize) -> Document {
    Document::open_bytes(&bench_archive(pages)).expect("benchmark document")
}

fn converting(c: &mut Criterion) {
    let mut group = c.benchmark_group("convert");
    for &pages in PAGES {
        let doc = fixture(pages);
        let page = doc.page(0).expect("first page");
        let fonts = doc.fonts();
        // The throughput unit is objects, so the three backends are
        // comparable on the same document.
        group.throughput(Throughput::Elements((pages * OBJECTS_PER_PAGE) as u64));

        group.bench_with_input(BenchmarkId::new("png", pages), &(), |b, ()| {
            let options = RenderOptions::default();
            b.iter(|| page_to_png(page, fonts, &options).expect("png"));
        });

        group.bench_with_input(BenchmarkId::new("svg", pages), &(), |b, ()| {
            let options = SvgOptions {
                text: TextExport::LiveText,
                fonts: None,
            };
            b.iter(|| page_to_svg(page, &options).expect("svg"));
        });

        group.bench_with_input(BenchmarkId::new("pdf", pages), &(), |b, ()| {
            let options = PdfOptions {
                text: TextExport::LiveText,
                fonts: None,
            };
            b.iter(|| page_to_pdf(page, &options).expect("pdf"));
        });
    }
    group.finish();
}

criterion_group!(benches, converting);
criterion_main!(benches);
