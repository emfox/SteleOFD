# Performance

What the engine costs, how to measure it, and what the current numbers are.

## How to run the benchmarks

```sh
cargo bench
```

Four targets, one per stage a reader pays for:

| Target | Crate | Measures |
| --- | --- | --- |
| `parse` | `steleofd-parser` | Opening a document, and flattening one page or all of them |
| `convert` | `steleofd-convert` | PNG, SVG and PDF output for one page |
| `search` | `steleofd-search` | Building an index, and querying it |

Each runs at one page and eight, and reports throughput in objects, so both
the per-page and the per-object cost are visible. A single target runs alone
with `cargo bench -p steleofd-parser --bench parse`.

## The document being measured

The benchmarks do not use the corpus. A corpus is fetched rather than
committed, and a benchmark needs the same bytes on every machine and every
run. `steleofd_testkit::bench_archive(pages)` builds one instead: a real
archive with a real embedded face, 120 objects per page, mixed on purpose —
filled paths, strokes, axial and radial shadings, and text runs — because a
baseline of one path and one word measures nothing a reader would wait for.

## The baseline

Measured on the development machine, 96 DPI, `cargo bench` defaults. Treat
these as the shape of the cost, not as a promise about other hardware.

| Benchmark | One page | Eight pages |
| --- | --- | --- |
| open a document | 18 µs | 28 µs |
| flatten a page | 428 µs | 434 µs |
| flatten a document | 426 µs | 3.5 ms |
| render to PNG | 4.6 ms | 4.3 ms |
| convert to SVG | 732 µs | 738 µs |
| convert to PDF | 2.2 ms | 2.2 ms |
| build a search index | 1.9 µs | 26 µs |
| run a query | 3.1 µs | 26 µs |

The per-page figures are flat across one page and eight, which is what
linearity looks like: rasterizing a page costs the same whether it is the
first or the eighth. Rasterization dominates everything else, and it is the
one number a performance budget should be stated against.

`open a document` and `build a search index` both grow with the page count
because both walk every page, so their eight-page figure is the interesting
one; the one-page figure is the fixed cost of reading the entry point.

## What is not measured yet

There are no budgets and no CI gate. M1-b and M1-c in [`ROADMAP.md`](ROADMAP.md)
are where those land; until then a regression is visible only by running the
benchmarks by hand and reading the diff. The numbers above are the ones a
budget should be set from.
