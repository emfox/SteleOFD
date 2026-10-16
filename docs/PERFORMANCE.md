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
| open a document | 26 µs | 34 µs |
| flatten a page | 427 µs | 422 µs |
| flatten a document | 431 µs | 3.40 ms |
| render to PNG | 4.29 ms | 4.33 ms |
| convert to SVG | 486 µs | 490 µs |
| convert to PDF | 1.91 ms | 1.93 ms |
| build a search index | 1.85 µs | 27.5 µs |
| run a query | 3.19 µs | 25.8 µs |

The per-page figures are flat across one page and eight, which is what
linearity looks like: rasterizing a page costs the same whether it is the
first or the eighth. Rasterization dominates everything else, and it is the
one number a performance budget should be stated against.

`open a document` and `build a search index` both grow with the page count
because both walk every page, so their eight-page figure is the interesting
one; the one-page figure is the fixed cost of reading the entry point.

## The budget

A benchmark on its own is a number nobody acts on. A budget turns it into a
line: this is what a reader is allowed to wait for, and a change that crosses
it has to be argued for rather than merged by accident.

The figures below are the *one-page* case, because that is what a reader feels
when they open a document and scroll. They are set at roughly **four times the
measured baseline**, which is deliberately loose: ordinary refactoring,
dependency bumps and CI hardware all move a benchmark by tens of percent, and a
budget that tight would be red on a bad afternoon and ignored within a week.
What it catches is the change that makes a page cost ten times what it did —
a backend that stops caching, a font lookup turned back into a re-parse per
glyph, a shading resolved per pixel.

| Stage | Budget (one page) | Baseline | Headroom |
| --- | --- | --- | --- |
| Open a document | 150 µs | 26 µs | 5.8× |
| Flatten a page | 2 ms | 427 µs | 4.7× |
| Render a page to PNG | 20 ms | 4.29 ms | 4.7× |
| Convert a page to SVG | 2.5 ms | 486 µs | 5.1× |
| Convert a page to PDF | 10 ms | 1.91 ms | 5.2× |

Two budgets are stated per page rather than per document, because the fixed
cost of opening is already a separate line: a 500-page document is allowed
500 × its per-page budget plus one open, and anything that scales worse than
that is the regression.

> The budget is a ceiling for a *deliberate* regression check, not a target.
> A change that makes a stage slower and does not cross the ceiling is still
> worth a note in the commit message; the ceiling only prevents the slow
> change nobody noticed.

## What is not measured yet

The budgets above are enforced by [M1-c](ROADMAP.md), the CI job that runs the
benchmarks and fails on a large regression. Until that job lands, a regression
is visible only by running `cargo bench` by hand and reading the diff — so the
numbers are the budget, and the gate is still to come.
