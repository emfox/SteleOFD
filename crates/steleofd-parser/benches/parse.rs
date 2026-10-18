//! What it costs to open a document and flatten its pages.
//! The two scale differently: opening reads the entry point once, flattening
//! walks every page's content.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use steleofd_package::Package;
use steleofd_parser::{ParsedDocument, Parser};
use steleofd_testkit::{OBJECTS_PER_PAGE, bench_archive};

/// Page counts the baseline is measured at: one page is the floor a reader
/// notices, eight is a chapter, and twenty is the packet the memory ceiling is
/// stated for — the first count where a cost per page that is not linear shows.
const PAGES: &[usize] = &[1, 8, 20];

/// Run `body` with a parser over `bytes` and the document it opens.
/// The parser borrows its package, so the package lives on the stack here;
/// leaking one per iteration would grow without bound inside a benchmark.
fn with_document<R>(bytes: &[u8], body: impl FnOnce(&mut Parser<'_>, &ParsedDocument) -> R) -> R {
    let mut package = Package::from_bytes(bytes).expect("benchmark package");
    let mut parser = Parser::new(&mut package);
    let doc = parser.open_document().expect("open");
    body(&mut parser, &doc)
}

fn parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    for &pages in PAGES {
        let bytes = bench_archive(pages);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("open_document", pages),
            &bytes,
            |b, bytes| {
                b.iter(|| with_document(bytes, |_, doc| doc.document.pages.len()));
            },
        );
    }
    group.finish();
}

fn flattening(c: &mut Criterion) {
    let mut group = c.benchmark_group("flatten");
    for &pages in PAGES {
        let bytes = bench_archive(pages);
        group.throughput(Throughput::Elements((pages * OBJECTS_PER_PAGE) as u64));
        group.bench_with_input(
            BenchmarkId::new("page_display", pages),
            &bytes,
            |b, bytes| {
                with_document(bytes, |parser, doc| {
                    b.iter(|| parser.page_display(doc, 0).expect("flatten"));
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("whole_document", pages),
            &bytes,
            |b, bytes| {
                with_document(bytes, |parser, doc| {
                    b.iter(|| parser.display_list(doc, 0, pages).expect("flatten"));
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, parsing, flattening);
criterion_main!(benches);
