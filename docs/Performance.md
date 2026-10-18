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
| `memory` | `steleofd-parser` | What a rasterized page holds, counted rather than timed |
| `convert` | `steleofd-convert` | PNG, SVG and PDF output for one page |
| `search` | `steleofd-search` | Building an index, and querying it |

The timed targets run at one page, eight and twenty, and report throughput in
objects, so both the per-page and the per-object cost are visible. A single
target runs alone with `cargo bench -p steleofd-parser --bench parse`.

## The document being measured

The benchmarks do not use the fixtures. Samples are fetched rather than
committed, and a benchmark needs the same bytes on every machine and every
run. `steleofd_testkit::bench_archive(pages)` builds one instead: a real
archive with a real embedded face, 120 objects per page, mixed on purpose —
filled paths, strokes, axial and radial shadings, and text runs — because a
baseline of one path and one word measures nothing a reader would wait for.

## The baseline

Measured on the development machine, 96 DPI, `cargo bench` defaults. Treat
these as the shape of the cost, not as a promise about other hardware.

| Benchmark | One page | Eight pages | Twenty pages |
| --- | --- | --- | --- |
| open a document | 24 µs | 32 µs | 45 µs |
| flatten a page | 477 µs | 481 µs | 465 µs |
| flatten a document | 477 µs | 3.72 ms | 9.27 ms |
| render to PNG | 5.46 ms | 5.17 ms | 5.19 ms |
| convert to SVG | 530 µs | 526 µs | 530 µs |
| convert to PDF | 2.21 ms | 2.13 ms | 2.13 ms |
| build a search index | 1.85 µs | 27.5 µs | — |
| run a query | 3.19 µs | 25.8 µs | — |

The per-page figures are flat across all three counts, which is what linearity
looks like: rasterizing a page costs the same whether it is the first or the
twenty-first. Rasterization dominates everything else, and it is the one number
a performance budget should be stated against.

`open a document`, `flatten a document` and `build a search index` grow with the
page count because all three walk every page, so their twenty-page figure is the
interesting one; the one-page figure is the fixed cost of reading the entry
point.

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
| Open a document | 150 µs | 24 µs | 6.3× |
| Flatten a page | 2 ms | 477 µs | 4.2× |
| Render a page to PNG | 20 ms | 5.46 ms | 3.7× |
| Convert a page to SVG | 2.5 ms | 530 µs | 4.7× |
| Convert a page to PDF | 10 ms | 2.21 ms | 4.5× |

Two budgets are stated per page rather than per document, because the fixed
cost of opening is already a separate line: a 500-page document is allowed
500 × its per-page budget plus one open, and anything that scales worse than
that is the regression.

> The budget is a ceiling for a *deliberate* regression check, not a target.
> A change that makes a stage slower and does not cross the ceiling is still
> worth a note in the commit message; the ceiling only prevents the slow
> change nobody noticed.

## The memory ceiling

A timing says how long a page takes and nothing about how much of the machine
it is holding. The documents this is for are scanned invoices of tens of pages,
and a single A4 at 300 dpi is about 26 MB once decoded — so a reader that keeps
a dozen of them is holding three hundred megabytes and will be killed on a
machine that looked fine a minute earlier. Time and memory therefore have
separate budgets, and the memory one is the reason the cache has a size at all.

| Ceiling | Budget | Measured | Headroom |
| --- | --- | --- | --- |
| One page, rasterized and held | 40 MB | 18.8 MB | 2.1× |
| A twenty-page packet, all pages held | 512 MB | 18.8 MB | 27× |

The single-page figure is what one raster holds once it is decoded: A4 at 96
DPI is 794 × 1123 pixels of RGBA, which is 3.6 MB, and the rest is the
flattened page, the glyph cache and the copy the encoder makes. The page is
measured at the reader's default zoom, not at its worst, because 40 MB is
already above what the worst case can reach within the texture limit: a page
that fits `MAX_TEXTURE_SIDE` is at most 2048 px on its long edge, so its raster
cannot exceed 2048 × 2048 × 4 bytes, or 16 MB, plus the same again for the copy
the texture upload makes.

The twenty-page figure is the one that decides the cache. Every page held at
once must stay under 512 MB, which at 18.8 MB a page is twenty-seven pages; the
reader holds twelve, so the ceiling has room for the whole window a continuous
layout shows and then some. A change that made a page cost 40 MB would bring
twenty pages to 800 MB and be caught here rather than on a reader's machine.

Both readings come from the `memory` benchmark, which wraps the allocator and
prints what a page holds once it has been drawn:

```sh
cargo bench -p steleofd-parser --bench memory | tee memory.log
python3 scripts/check_performance.py --logs memory.log
```

It is a printout rather than a criterion estimate because criterion measures
time: a byte count has no duration, and criterion would report the work as
nanoseconds and mean nothing by it. The reading is taken once, outside the
measurement loop, so the loop's own allocations cannot disturb it.

## How the budget is enforced

`scripts/check_performance.py` reads the criterion output a `cargo bench` run
leaves in `target/criterion/`, plus the memory printout when one is given, and
checks each budgeted line against its ceiling. It runs locally after a
`cargo bench`, and in CI as the `performance` job:

```sh
cargo bench -p steleofd-parser --bench parse
cargo bench -p steleofd-parser --bench memory | tee memory.log
cargo bench -p steleofd-convert --bench convert
python3 scripts/check_performance.py --logs memory.log
```

Only the ceiling is enforced, never the change against a stored baseline. A
gate on the change would fail on ordinary noise and be disabled within a week;
a gate on the ceiling fails on the change worth stopping. A missing reading is
a failure rather than a pass, because a budget nobody measures is a budget
nobody has. Moving a ceiling is a deliberate edit to this document, which is
the point: the argument happens in the diff rather than in a comment nobody
reads.

## What is not measured yet

The budget covers opening, flattening, the three one-page conversions and the
memory a page holds. Search is benchmarked but not budgeted, because a reader
never waits on it directly.

Two costs are still unmeasured: the memory of a *flattened* document without any
raster, which matters for export and for the CLI; and the reader's own peak,
which includes the decoded image, its GPU upload and the `egui` texture, not
only the raster counted here. Both are candidates once a reader on a small
machine says which one hurts.
