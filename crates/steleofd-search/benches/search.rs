//! What it costs to index a document and to query the index.
//! Building is paid once per open and a query on every keystroke, so the two
//! are measured apart and the query at two hit rates.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use steleofd_package::Package;
use steleofd_parser::Parser;
use steleofd_search::{SearchIndex, SearchOptions};
use steleofd_testkit::bench_archive;

/// Page counts to measure at. Eight pages is enough that the per-page cost of
/// indexing shows, and small enough that the query stays fast to sample.
const PAGES: &[usize] = &[1, 8];

/// A term every page carries, and one only the first page's text has. The
/// first is the worst case for a query; the second is the common one.
const BROAD: &str = "invoice";
const NARROW: &str = "of page";

/// A flattened document plus the index over it. The parser borrows its
/// package, so the package lives here on the stack and drops with the result;
/// nothing is leaked, because this runs once per group.
fn fixture(pages: usize) -> (SearchIndex, steleofd_display::DisplayList) {
    let bytes = bench_archive(pages);
    let mut package = Package::from_bytes(&bytes).expect("benchmark package");
    let mut parser = Parser::new(&mut package);
    let doc = parser.open_document().expect("open");
    let list = parser.display_list(&doc, 0, pages).expect("flatten");
    let index = SearchIndex::build(&list);
    (index, list)
}

fn indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_index");
    for &pages in PAGES {
        let (_, list) = fixture(pages);
        group.throughput(Throughput::Elements(pages as u64));
        group.bench_with_input(BenchmarkId::new("build", pages), &list, |b, list| {
            b.iter(|| SearchIndex::build(list));
        });
    }
    group.finish();
}

fn querying(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_query");
    for &pages in PAGES {
        let (index, _) = fixture(pages);
        group.throughput(Throughput::Elements(pages as u64));
        for (label, needle) in [("broad", BROAD), ("narrow", NARROW)] {
            group.bench_with_input(
                BenchmarkId::new(label, pages),
                &(index.clone(), needle),
                |b, (index, needle)| {
                    b.iter(|| index.search(needle, SearchOptions::default()));
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, indexing, querying);
criterion_main!(benches);
