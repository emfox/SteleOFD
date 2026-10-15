# SteleOFD Rust Architecture

> Version v0.1 | 24 crates | `cargo check --workspace` clean
> Brand: SteleOFD (external); code prefix: `steleofd-*`

---

## 1. Layered architecture

```
+--------------------------------------------------------------------+
| L5 adapters   steleofd-cli / -wasm / -ffi / -server                |
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
         +--> render    raster / canvas / svg     screen and images
         +--> print                               printing
         +--> convert   PDF / Office
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

### 2.2 Pluggable render backends

```rust
[features]
default = ["backend-raster"]
backend-raster = []   # tiny-skia bitmap, PNG for server and CLI
backend-canvas = []   # Canvas 2D command stream, replayed by browser JS
backend-svg    = []
backend-pdf    = []
```

One Rust engine compiles to different shapes: the server emits bitmaps while the
browser receives a **command stream** rather than an image. The stream is about a
tenth the size of a bitmap and re-zooms vectorially without refetching, staying
sharp at 200% or 300%.

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

### 2.5 Safety built in

| Threat | Defense |
|---|---|
| Zip Slip path traversal | `PackageError::IllegalPath` |
| Zip bomb | Entry count and decompressed size ceilings (512 MB) |
| Stack exhaustion via deep nesting | `DisplayListBuilder::max_depth`, default 64 |
| Hostile document hanging the renderer | `RenderError::BudgetExceeded`, degrading to partial output |
| Missing fonts producing garbled text | Pre-check via `FontRegistry::missing_families()` |
| Edits breaking existing signatures | `EditError::WouldInvalidateSignature`, requiring confirmation |

### 2.6 Zero unsafe

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

| Layer | Crate | Responsibility |
|---|---|---|
| L0 | `steleofd-common` | Value types (`Millimeter`, `Point`, `Rect`), versions, standard constants |
| L0 | `steleofd-error` | Unified error taxonomy |
| L0 | `steleofd-io` | `ByteSource` and `ByteSink` abstraction over file, memory, slice and directory |
| L1 | `steleofd-model` | OFD data model (`CT_*` and `ST_*` type system) |
| L1 | `steleofd-xml` | XML codec wrapping quick-xml |
| L1 | `steleofd-package` | ZIP container I/O, safety validation, atomic replacement |
| L1.5 | `steleofd-i18n` | Fluent localization layer |
| L2 | `steleofd-geometry` | Backend-independent `Path` and `Affine` |
| L2 | `steleofd-font` | Font registry, metrics, shaping, system fallback |
| L2 | `steleofd-display` | **Display list IR**, the architectural keystone |
| L2 | `steleofd-parser` | `Content.xml` to `DisplayList` |
| L2 | `steleofd-render` | Pluggable backends: raster, canvas, svg, pdf |
| L3 | `steleofd-crypto` | SM2, SM3, SM4, X.509, PKCS#12 |
| L3 | `steleofd-sign` | Electronic seals and verification per GB/T 38540 |
| L3 | `steleofd-convert` | Format interconversion: PDF, image, SVG, Office, OCR |
| L3 | `steleofd-edit` | Command-pattern editing with an undo stack |
| L3 | `steleofd-search` | Full-text search and reverse glyph-to-Unicode lookup |
| L3 | `steleofd-archive` | Archival compliance validation (OFD-A) |
| L3 | `steleofd-print` | Printing, reusing the display list for fidelity |
| L4 | `steleofd-core` | Facade, trimming capabilities through features |
| L5 | `steleofd-cli` | Command line tool `steleofd` |
| L5 | `steleofd-wasm` | Browser WebAssembly bindings |
| L5 | `steleofd-ffi` | C ABI shared library for OA and ERP integration |
| L5 | `steleofd-server` | HTTP service for batch conversion and verification |

---

## 4. Capability features

Features trim binary size per delivery form:

```toml
[features]
default  = ["render", "sign", "convert", "pdf"]
render   = ["dep:steleofd-render", "steleofd-render/backend-raster"]
canvas   = ["dep:steleofd-render", "steleofd-render/backend-canvas"]
print    = ["dep:steleofd-print"]
sign     = ["dep:steleofd-sign", "dep:steleofd-crypto"]
convert  = ["dep:steleofd-convert", "dep:steleofd-render"]
pdf      = ["convert", "steleofd-convert/pdf"]
office   = ["convert", "steleofd-convert/office"]
ocr      = ["convert", "steleofd-convert/ocr"]
edit     = ["dep:steleofd-edit"]
search   = ["dep:steleofd-search"]
archive  = ["dep:steleofd-archive"]
async    = ["dep:rayon"]
```

Typical combinations:

| Delivery form | Enabled features |
|---|---|
| Server-side conversion service | `convert, pdf, archive, async` |
| Browser preview SDK | `canvas, sign` |
| Desktop reader | `render, sign, convert, print, edit, search` |
| Embedded slim build | `render`, English only |

---

## 5. Localization architecture

### 5.1 Layering

```
engine       ->  structured errors and data only
   |
i18n         ->  renders MessageId into a concrete language
   |
presentation ->  CLI / WASM / FFI / Server / GUI
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

```
locales/
├── en-US/          <- fallback locale, must always be complete
│   ├── core.ftl        application level
│   ├── reader.ftl      reader module
│   ├── convert.ftl     conversion module
│   ├── sign.ftl        signature module
│   ├── edit.ftl        edit module
│   ├── search.ftl      search module
│   └── error.ftl       error messages
└── zh-CN/          <- same structure
```

**Current state:** 56 message keys across 2 locales, with identical key sets.

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
| **P0 skeleton** | 24-crate layering, error taxonomy, display list IR, i18n layer | Done |
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

**Still to build:** the original sequence is complete; what comes next is tracked
in [`ROADMAP.md`](ROADMAP.md). Clause-level conformance is tracked in
`compliance/gb-t-33190.toml`; 20 of 47 clauses are covered so far, and the
register is the single source of truth that `scripts/check_compliance.py` gates
on.

---

## 8. Decided

1. **Clipping uses the backend's native support.** The raster backend relies on
   `tiny-skia` clipping and blending. Blend modes it cannot express degrade to a
  `RenderError` rather than a silently wrong image. A self-built clip stack is
   deferred; see the `TODO` in `crates/steleofd-render/src/lib.rs`.
2. **Fonts are embedded in full.** PDF and SVG export embed the whole font rather
   than a subset. Subsetting, which can cut output size by an order of magnitude
   for CJK, is tracked as a separate later item.
3. **No render cache yet.** Bitmap caching, page-window pruning and eviction stay
   out of scope until raster rendering works and a real large document can be
   measured.

## 9. Open decisions

1. **Render backend priority:** raster first (server and CLI images) or canvas first
   (browser)?
2. **Chinese national cryptography:** pure Rust using the `sm2`, `sm3` and `sm4`
   crates, which suit WASM and cross-compilation, or binding GmSSL, which is more
   complete but brings a C toolchain? The pure Rust path means handling X.509 chain
   validation details ourselves.
3. **Domestic OS support:** is a native build and test matrix on Kylin and UnionTech
   UOS required? This affects dependency choices such as `font-kit`.
4. **Chinese product name:** deferred until the brand strategy is settled.
