# SteleOFD Rust Architecture

> Version v0.1 | 26 crates | `cargo check --workspace` clean
> Brand: SteleOFD (external); code prefix: `steleofd-*`

---

## 1. Layered architecture

```
+--------------------------------------------------------------------+
| L5 adapters   steleofd-cli / -wasm / -ffi / -server / -reader      |
|               The only layer aware of a concrete delivery form     |
+--------------------------------------------------------------------+
| L4 facade     steleofd-core                                        |
|               The single public entry point, trims capabilities    |
|               through features, free to refactor internals         |
+--------------------------------------------------------------------+
| L1.5 i18n     steleofd-i18n                                        |
|               Depended on by presentation only, never by engine    |
+--------------------------------------------------------------------+
| L3 capability sign / crypto / convert / edit / search / archive /  |
|               print. Decoupled from each other, optional per feature|
+--------------------------------------------------------------------+
| L2 engine     parser / render / display / font / geometry          |
|               Parsing, rendering, fonts, geometry; produces the    |
|               display list IR                                      |
+--------------------------------------------------------------------+
| L1 format     model / xml / package                                |
|               OFD data model, XML codec, ZIP container             |
+--------------------------------------------------------------------+
| L0 foundation common / error / io                                  |
|               Value types, unified errors, I/O abstraction;        |
|               no domain semantics                                  |
+--------------------------------------------------------------------+
```

**Dependencies always point from upper to lower layers, never the reverse.** The
compiler enforces this: a backward dependency immediately becomes a cycle and
fails the build. The `font`-to-`display` cycle was caught and fixed exactly this
way.

---

## 2. Core design decisions

### 2.1 The display list is the keystone

```
  Content.xml (nested template tree)
         |  parsed exactly once, here
         v
  DisplayList (flattened, absolute CTM, resources resolved)
         |
         +--> render    raster                    screen and images
         +--> print                               printing
         +--> convert   PDF / SVG / PNG           Office and OCR are planned
         +--> search    text extraction
```

**Problem:** if every backend parsed the XML and handled transforms itself, the
logic would be duplicated N times, the backends would disagree (the classic
"export does not match the preview"), and no optimization could be shared.

**Solution:** `steleofd-display` defines a single intermediate representation. The
parser flattens the nested template tree into a one-dimensional command sequence
carrying absolute transforms, and every backend consumes only that IR.

**Benefits:**
- Cross-backend rendering can be verified pixel by pixel, so the web build is not
  an approximation;
- A new backend only translates the `DrawOp` enum without understanding OFD;
- Viewport culling, batching and glyph caching can be implemented once.

**Key constraint:** `steleofd-display` depends on **no backend library** (no
tiny-skia, no lopdf), only on geometry and fonts, and the whole IR is
serializable. That is what lets it cross process and language boundaries.

### 2.2 The render backend

```rust
[features]
default = ["backend-raster"]
backend-raster = []   # tiny-skia bitmap, PNG for server and CLI
```

There is one backend today. A **command stream** backend for browsers is
planned: it would ship a vector command list about a tenth the size of a
bitmap, re-zooming without refetching and staying sharp at 200% or 300%. It
adds its own feature when it exists; the feature is not declared in advance.

### 2.3 No user-facing text in the engine

Error types are structured enums with parameters, never prose:

```rust
// Correct: any language can render this
FormatError::PageOutOfRange { index: 42, total: 100 }

// Forbidden: hardcoded prose makes the engine unreusable
"page index out of range: 42 (document has 100 pages)"
```

The presentation layer renders the enum into a concrete language. English in the
`#[error("...")]` attribute is developer-facing log text only. This rule is the
precondition for localization to work at all: the engine stays reusable from C
ABI, JS and Python, and adding a language never touches engine code.

### 2.4 Unified error taxonomy

`steleofd-error` defines one error enum per subsystem, each concerned only with its
own failure semantics: `PackageError`, `XmlError`, `FormatError`, `RenderError`,
`FontError`, `SignatureError`, `CryptoError`, `ConvertError` and `EditError`, all
converting losslessly into the top level `OfdError`.

Errors carry context such as path, page index and object id, and expose semantic
predicates like `is_corruption()` so callers can decide whether to degrade.

### 2.5 Where a line of text goes

Table 46 of GB/T 33190 gives a `TextCode` an origin (`X`, `Y`) and two lists of
offsets between glyphs. Read plainly, `DeltaX[i]` is a step: the distance glyph
`i + 1` sits from glyph `i`, with any glyph past the end of the list taking the
text size instead. Producers need a second reading to lay out a row of digits,
because a digit is about half an em wide and a step has to be wider than the
glyph it moves over.

A list shorter than the steps between the glyphs is read as positions: a glyph
with no entry of its own sits where its own offset puts it, and a negative
offset is a carriage return. How many steps a list supplies matters here, and
`g count value` states `count` of them, so `DeltaX="g 11 1.5875 -17.4625"` is
twelve offsets rather than two. `DeltaY` is read the same way, for the step
across the line, so a run that turns onto a second row lands where the list
says. The rule is one comparison, in `flatten::GlyphAt`:

| Offsets | Glyphs | Read as |
|---|---|---|
| one per step, or more | any | steps |
| fewer than the steps | two or more | positions |

`CharDirection` and `ReadDirection` (table 47) each reach `flatten` through one
transform, `flatten::run_placement`. Figure 62 puts the pen in the line's own
frame, where `+x` is the read direction, so the read angle carries that frame
onto the page and `HScale` widens it there. The glyph's own angle stays out of
that transform, in `GlyphRun::glyph_angle`, because a glyph turns about its
origin and must not turn the line with it; `GlyphRun::glyph_matrix` applies it,
so the SVG, PDF and raster backends cannot disagree about the order.

Table 47 measures both angles from the page, so the glyph keeps
`CharDirection - ReadDirection` inside the line: reading downwards with
`CharDirection` 0 cancels the line's quarter and stands the glyph upright.
Figure 63's downward line therefore needs no `DeltaX` at all, and `DeltaY`
becomes the step along it. A producer can also write offsets directly into the
text, with a space and a `g` standing for a glyph that takes no room of its own;
that is a property of the string, and it is honoured the same way in either
reading.

Export takes the same route. A PDF is one document holding a list of pages,
and each page carries its own `PhysicalBox` as a `MediaBox`, because a receipt
is shorter than the A4 sheets that follow it. The conversion is handed the
document's font manager so a run is drawn as outlines; without one it falls
back to live text, which only reads where the same face is installed.

A step back moves no glyph forward, so it ends a row: `flatten::run_shapes`
cuts the code at each one, and the row below takes its own height from `DeltaY`.
A single run has one origin and one transform, so a code set in rows cannot be
one run without the return becoming an advance that drags the row sideways.

### 2.6 Safety built in

| Threat | Defense |
|---|---|
| Zip Slip path traversal | `PackageError::IllegalPath` |
| Zip bomb | Entry count and decompressed size ceilings (512 MB) |
| Stack exhaustion via deep nesting | `DisplayListBuilder::max_depth`, default 64 |
| Hostile document hanging the renderer | `RenderError::BudgetExceeded`, degrading to partial output |
| Missing fonts producing a silently blank page | Every candidate substitute must cover the text; misses land in `missing_fonts` / `missing_glyphs`, are printed by the CLI, and set a non-zero exit code |
| Edits breaking existing signatures | `EditError::WouldInvalidateSignature`, requiring confirmation |

### 2.7 Zero unsafe

The whole workspace uses `#![forbid(unsafe_code)]`; `steleofd-ffi` is the only
exemption, because a C ABI requires it. Two designs were reworked to reach this:

- **Font cache:** previously extended a borrow with `unsafe`; now returns
  `Arc<FontFace>`, so there is no `unsafe`, the lifetime is governed by reference
  counting, and `clear_cache` cannot dangle.
- **Font face:** previously held data through `Box::leak`, which was a genuine
  memory leak; now holds an `Arc<Vec<u8>>` and constructs a temporary `Face` per
  outline query, parsing only the table directory at microsecond cost.

---

## 3. Crate inventory

The **Registry** column is what `publish` in each manifest states, and
`scripts/check_publish.py` holds it to the whole workspace. A published crate is
one a consumer adds as a dependency; the rest ship as an application, and
`steleofd-i18n` and `steleofd-testkit` stay in the workspace alone.

| Layer | Crate | Responsibility | Registry |
|---|---|---|---|
| L0 | `steleofd-common` | Value types (`Millimeter`, `Point`, `Rect`), versions, standard constants | published |
| L0 | `steleofd-error` | Unified error taxonomy | published |
| L0 | `steleofd-io` | `ByteSource` and `ByteSink` abstraction over file, memory, slice and directory | published |
| L1 | `steleofd-model` | OFD data model (`CT_*` and `ST_*` type system) | published |
| L1 | `steleofd-xml` | XML codec wrapping quick-xml | published |
| L1 | `steleofd-package` | ZIP container I/O, safety validation, atomic replacement | published |
| L1.5 | `steleofd-i18n` | Fluent localization layer | withheld |
| L2 | `steleofd-geometry` | Backend-independent `Path` and `Affine` | published |
| L2 | `steleofd-font` | Font registry, metrics, shaping, system fallback | published |
| L2 | `steleofd-display` | **Display list IR**, the architectural keystone | published |
| L2 | `steleofd-parser` | `Content.xml` to `DisplayList` | published |
| L2 | `steleofd-render` | Raster backend: display list to pixels | published |
| L3 | `steleofd-crypto` | SM2, SM3, SM4, X.509, PKCS#12 | published |
| L3 | `steleofd-sign` | Electronic seals and verification per GB/T 38540 | published |
| L3 | `steleofd-convert` | Format interconversion: PDF, SVG, PNG (Office, OCR planned) | published |
| L3 | `steleofd-edit` | Command-pattern editing with an undo stack | published |
| L3 | `steleofd-search` | Full-text search and reverse glyph-to-Unicode lookup | published |
| L3 | `steleofd-archive` | Archival compliance validation (OFD-A) | published |
| L3 | `steleofd-print` | Printing, reusing the display list for fidelity | published |
| L4 | `steleofd-core` | Facade, trimming capabilities through features | published |
| L5 | `steleofd-cli` | Command line tool `steleofd` | withheld |
| L5 | `steleofd-wasm` | Browser WebAssembly bindings | withheld |
| L5 | `steleofd-ffi` | C ABI shared library for OA and ERP integration | withheld |
| L5 | `steleofd-reader` | Desktop reader (egui/eframe) | withheld |
| L5 | `steleofd-server` | HTTP service for batch conversion and verification | withheld |
| — | `steleofd-testkit` | Fixtures shared by the test suites, a dev-dependency only | withheld |

---

## 4. Capability features

Features trim binary size per delivery form:

```toml
[features]
default  = ["render", "sign", "convert", "pdf"]
render   = ["dep:steleofd-render", "steleofd-render/backend-raster"]
print    = ["dep:steleofd-print"]
sign     = ["dep:steleofd-sign", "dep:steleofd-crypto"]
convert  = ["dep:steleofd-convert", "dep:steleofd-render"]
pdf      = ["convert", "steleofd-convert/pdf"]
office   = ["convert", "steleofd-convert/office"]   # not implemented yet
ocr      = ["convert", "steleofd-convert/ocr"]      # not implemented yet
edit     = ["dep:steleofd-edit"]
search   = ["dep:steleofd-search"]
archive  = ["dep:steleofd-archive"]
async    = ["dep:rayon"]
```

`office` and `ocr` compile and change nothing; they exist so the capability has
a name before it has an implementation. Everything else is wired.

Typical combinations:

| Delivery form | Enabled features |
|---|---|
| Server-side conversion service | `convert, pdf, archive, async` |
| Browser preview SDK | `sign` (a canvas backend is planned) |
| Web reader, full featured | `render, sign, convert, print, edit, search` |
| Embedded slim build | `render`, English only |

---

## 5. Localization architecture

### 5.1 Layering

```
engine       ->  structured errors and data only
   |
i18n         ->  renders MessageId into a concrete language
   |
presentation ->  CLI / WASM / FFI / Server / Desktop reader
```

### 5.2 Why Fluent

Key-value schemes in the gettext style cannot express plural categories (six in
Russian, six in Arabic), grammatical gender and case, or localized numbers and
dates.

```ftl
# One key; Chinese has no plural distinction, English does
sign-count = { $count ->
    [0] No signatures
    [one] { $count } signature
   *[other] { $count } signatures
}
```

### 5.3 Directory layout

There are two catalogues, not one, and they live beside the layer that shows
their strings:

```
crates/steleofd-i18n/locales/        # engine messages, keyed MessageId
├── en-US/          <- fallback locale, must always be complete
│   ├── core.ftl        application level
│   ├── convert.ftl     conversion module
│   ├── sign.ftl        signature module
│   ├── edit.ftl        edit module
│   ├── search.ftl      search module
│   └── error.ftl       error messages
└── zh-CN/          <- same structure

crates/steleofd-reader/locales/      # reader interface, keyed ReaderMessage
├── en-US/reader.ftl
└── zh-CN/reader.ftl
```

The reader owns its own `ReaderMessage` list and its own `reader.ftl`, and merges
it into the engine bundle with `I18nBuilder::add_resource`, reaching its strings
through the key-based `I18n::text_key`. The engine never names a widget it does
not own, which is why `steleofd-i18n` holds engine messages only. The reader's
layout and per-frame flow are fixed in [`Ui-spec.md`](Ui-spec.md).

**Current state:** 73 engine keys and 32 reader keys, each across 2 locales with
identical key sets within its catalogue.

### 5.4 Conventions and safeguards

| Convention | Rationale |
|---|---|
| Kebab-case English keys | Never a sentence as a key, since rewording would then touch every catalogue |
| `en-US` is the fallback | Any missing translation degrades to English, preserving minimum usability |
| Embedded at compile time | `include_str!` means zero runtime I/O, and a missing file fails the build |
| CI consistency check | `scripts/check_i18n.py` verifies key sets, variables and code-to-catalogue agreement |
| No hand-assembled formats | `LocalizedFormat` centralizes numbers, dates and file sizes |

**A real defect the checker caught:** the first run found `edit-*` (4 keys) and
`search-*` (2 keys) missing from the FTL catalogues. Leaked to runtime, users
would have seen raw key names instead of text.

### 5.5 Reader window rules

The desktop reader keeps its layout zones and its per-frame flow in
[`Ui-spec.md`](Ui-spec.md), which also records why the window draws nothing it
cannot test. It is a separate document because the rules are about behaviour
over time, not about the shape of a layer.

---

## 6. Build and environment

### 6.1 Mirror configuration

Measured in the sandbox: `index.crates.io` responds (2.3 s) but **`static.crates.io`,
the actual download domain, returns zero bytes after a 30 second timeout**, which
makes cargo wait indefinitely. A mirror is required:

```toml
# ~/.cargo/config.toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[build]
jobs = 4
```

Measured throughput: `static.crates.io` at **0 B/s** (stalled) versus `rsproxy.cn`
at **3.9 MB/s**.

### 6.2 Common commands

```bash
cargo check --offline --workspace          # whole workspace, once deps are cached
cargo check -p steleofd-i18n               # a single crate
cargo test --offline --workspace           # full test run
cargo clippy --offline --workspace -- -D warnings
python3 scripts/check_i18n.py              # message catalogue consistency
python3 scripts/check_language.py          # English-only policy and comment density
```

### 6.3 Profile strategy

```toml
[profile.release]
lto = "thin"            # cross-crate inlining, offsetting layered abstraction
codegen-units = 1       # maximum optimization
panic = "abort"         # smaller binaries
strip = "symbols"

[profile.wasm-release]
inherits = "release"
opt-level = "s"         # size first for WASM
```

---

## 7. Roadmap

| Phase | Content | Status |
|---|---|---|
| **P0 skeleton** | 26-crate layering, error taxonomy, display list IR, i18n layer | Done |
| **P1 parsing** | `model`, `xml`, `package`, `parser` implementing OFD structure parsing | Done |
| **P2 rendering** | Raster backend in `render`, completing open-to-display | Done |
| **P3 conversion** | `convert`: OFD to PDF, PNG, SVG | Done |
| **P3b printing** | `print`: task planning and rasterization | Done |
| **P4b facade** | `core`: the feature-trimmed public surface | Done |
| **P4 verification** | Signature verification in `crypto` and `sign` | Done |
| **P5a delivery** | `cli`: the `steleofd` command line tool | Done |
| **P5b delivery** | `wasm`: browser bindings over the `core` facade | Done |
| **P5c delivery** | `ffi`: C ABI shared library for OA and ERP integration | Done |
| **P5d delivery** | `server`: HTTP service paths for OA and ERP back ends | Done |
| **P6 editing** | Command pattern and undo stack in `edit` | Done |
| **P7 search** | Full-text search over the display list in `search` | Done |
| **P8 archival** | Archival readiness auditing in `archive` | Done |
| **P9 conformance** | Annex A schemas rebuilt from the standard and enforced in CI | Done |

**Why this order:** finish the minimal visible path of P1 plus P2 first, opening and
displaying a single OFD page, then broaden capabilities. That yields end-to-end
feedback early instead of spinning inside abstraction layers.

**Still to build:** the original sequence is complete and shipped as `0.1.0`;
what is left, and which release has claimed it, is in
[`Roadmap.md`](Roadmap.md). Clause-level conformance is tracked in
`compliance/gb-t-33190.toml`; 74 clauses are registered, 73 of them `verified`
and one — `8.1.6` `PrintQuality` — a `deviation` with a stated reason. The
register is the single source of truth that `scripts/check_compliance.py` gates
on.

---

## 8. Decisions, and how they aged

Each of these was open when the document was written. They are kept with their
outcome rather than deleted, because the reasoning is what a new contributor
actually needs.

1. **Clipping uses the backend's native support.** The raster backend relies on
   `tiny-skia` clipping. This stands; no self-built clip stack was needed. One
   correction from the interoperability audit: `Clips` carries no `TransFlag`
   meaningful to a 2016 reader — the attribute is an OFD 1.2 extension, so the
   parser ignores it and the clip stays in object space. See
   [`INTEROP.md`](INTEROP.md).
2. **Fonts are embedded in full — superseded.** PDF and SVG export now subset:
   `steleofd-font::subset` rebuilds `glyf`/`loca`/`cmap`/`maxp`/`hmtx`/`hhea`/
   `head` down to the glyphs a document uses, and a face it cannot rewrite (CFF,
   or no `cmap`) is carried whole. A 20 MB CJK collection no longer costs 20 MB
   per conversion.
3. **No render cache yet.** Still true and still deliberate: raster rendering
   works and is measured against the budget in [`PERFORMANCE.md`](PERFORMANCE.md),
   but bitmap caching and eviction are not in scope until a real large document
   shows they are needed.
4. **Render backend priority: raster first — settled.** Raster is the backend the
   CLI, server and reader all share, and it is what the golden-image suite gates.
   There is no separate canvas backend; a browser shell would re-zoom the
   vectors, and that backend is a far-term candidate rather than a decision.
5. **Chinese national cryptography: pure Rust — settled.** The project uses the
   pure-Rust `sm2`/`sm3`/`sm4` path, which keeps WASM and cross-compilation
   working and needs no C toolchain. X.509 validation is handled in
   `steleofd-crypto`. `GmSSL` stays listed as a reference implementation, not a
   dependency.
6. **Domestic OS support:** open. No native build or test matrix on Kylin or
   UnionTech UOS is in place; the release workflow builds Linux, macOS and
   Windows. This stays open because it needs a decision rather than code.
7. **Chinese product name:** deferred, as before.
