# SteleOFD

A Rust implementation of OFD (Open Fixed-layout Document), covering reading,
conversion and signature verification.

[English](README.md) | [简体中文](README.zh-CN.md)

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/edition-2024-blue)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)
[![Standard](https://img.shields.io/badge/standard-GB%2FT%2033190--2016-red)]()

SteleOFD implements [GB/T 33190-2016, the Chinese national standard for
fixed-layout electronic documents](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D).
Parsing and rendering are written from scratch; there is no C or C++ dependency.

Status: early. The workspace and interfaces are in place, the engine is not.

## Features

| Capability | |
|---|---|
| Reading and rendering | GB/T 33190 rendering, lossless zoom, sharded loading for large documents |
| Conversion | OFD to and from PDF, SVG and PNG; batch and merge. Office and OCR are planned, not built |
| Signatures | SM2/SM3/SM4 and international algorithms, per GB/T 38540-2020 |
| Delivery | One engine built as a CLI, WebAssembly, a C ABI library and an HTTP service |
| Localization | Fluent message layer, en-US and zh-CN |

## Background

OFD and PDF are both fixed-layout formats, but OFD is an open ZIP + XML
container with national cryptography and electronic seals built into the
standard. Its layout is required to stay identical across software and hardware,
which is why it serves as the legal carrier for official documents, licences,
invoices and archives.

Rust fits here for two reasons. Document processing is CPU bound and has to run
in a browser, on a server and on the desktop, so zero-cost abstraction lets the
layering stay clean without a performance penalty. And since OFD files arrive
from arbitrary sources, `#![forbid(unsafe_code)]` matters more than usual: the
parser is the part handling untrusted input.

## Project layout

```
steleofd/
├── crates/
│   ├── steleofd-common        Value types, versions, standard constants
│   ├── steleofd-error         Error taxonomy
│   ├── steleofd-io            I/O abstraction (file, memory, slice)
│   ├── steleofd-model         OFD data model
│   ├── steleofd-xml           XML encoding and decoding
│   ├── steleofd-package       ZIP container I/O and validation
│   ├── steleofd-i18n          Localization messages (Fluent)
│   ├── steleofd-geometry      Paths and affine transforms
│   ├── steleofd-font          Font registry, metrics and shaping
│   ├── steleofd-display       Display list IR
│   ├── steleofd-parser        OFD to display list
│   ├── steleofd-render        Render backends
│   ├── steleofd-crypto        National cryptography and X.509
│   ├── steleofd-sign          Seals and verification
│   ├── steleofd-convert       Format interconversion
│   ├── steleofd-edit          Document editing
│   ├── steleofd-search        Full-text search
│   ├── steleofd-archive       Archival compliance validation
│   ├── steleofd-print         Printing
│   ├── steleofd-core          Facade
│   ├── steleofd-cli           Command line tool
│   ├── steleofd-wasm          WebAssembly bindings
│   ├── steleofd-ffi           C ABI shared library
│   └── steleofd-server        HTTP service
├── locales/                   Translation catalogues (.ftl)
├── scripts/                   Build and validation scripts
└── docs/                      Architecture documentation
```

## Design notes

### One parse, many consumers

`Content.xml` is a nested tree of templates. Having every output backend walk it
separately would duplicate the logic and, worse, let the backends drift apart.
That drift is the usual cause of an export not matching the preview.

So the tree is flattened once into a sequence of commands carrying absolute
transforms, and every downstream consumer reads that:

```
Content.xml (nested template tree)
       |
       v  parsed once
DisplayList (flattened, absolute CTM, resources resolved)
       |
       +--> render    raster
       +--> print
       +--> convert   PDF / SVG / PNG
       +--> search    text extraction
```

Office and OCR conversion are on the roadmap, not in the code.

### No user-facing text in the engine

Errors are structured enums with parameters, never prose:

```rust
// Any locale can render this
FormatError::PageOutOfRange { index: 42, total: 100 }

// Hardcoding prose here would make the engine unusable from C ABI, JS or Python
"page index out of range: 42 (document has 100 pages)"
```

### Untrusted input

| Threat | Defense |
|---|---|
| Zip Slip path traversal | `PackageError::IllegalPath` |
| Zip bomb | Entry count and decompressed size ceilings (512 MB) |
| Stack exhaustion via deep nesting | `DisplayListBuilder::max_depth`, default 64 |
| Hostile document hanging the renderer | `RenderError::BudgetExceeded`, degrading to partial output |
| Edits silently breaking signatures | `EditError::WouldInvalidateSignature`, requiring confirmation |

## Getting started

Rust 1.85+ with Edition 2024. No C toolchain needed.

```bash
git clone https://github.com/emfox/SteleOFD.git
cd SteleOFD

cargo check --workspace          # check the whole workspace
cargo build --release            # release build
cargo test --workspace           # run tests
```

### If crates.io downloads stall

When `static.crates.io` is unreachable, cargo can sit with no output for a long
time. A mirror fixes it:

```toml
# ~/.cargo/config.toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

`cargo check --offline` then works with no network access at all.

### Checks

```bash
python3 scripts/check_i18n.py            # message key consistency across locales
python3 scripts/check_language.py        # English-only policy and comment density
python3 scripts/check_i18n.py --strict   # treat warnings as failures
```

The i18n check compares the catalogues: same file set, same key set, consistent
placeholders, and the `all_keys()` list in Rust matching the keys in the FTL
files.

## Standards

| Standard | Scope |
|---|---|
| [GB/T 33190-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D) | Fixed-layout document format for electronic file storage and exchange |
| [GB/T 33476.1-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=419233A04D029066CF032CFACBC8713B) | Electronic official document format for Party and government bodies, part 1: document structure |
| [GB/T 33476.2-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=5B4E730245FF2FAF909435EBE6A659D2) | Same, part 2: presentation |
| [GB/T 38540-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=EBF1360C272E40E7A8B9B32ED0724AB1) | Cryptographic specification for secure electronic seals |
| [GM/T 0099-2020](https://std.samr.gov.cn/hb/search/stdHBDetailed?id=C60C676BC220358AE05397BE0A0AFABB) | Cryptography application for open fixed-layout documents: signature, encryption, integrity |
| [GB/T 39362-2020](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=C6B230010286DC79C84CF8EB72DE3F97) | Archiving specification for electronic official documents |

GB/T 33476 is published in three parts; parts 1 and 2 are the ones this project
follows. GM/T 0099-2020 is a cryptography industry standard issued by the State
Cryptography Administration, so it lives on the industry standards platform
rather than the national standards one.

## Roadmap

- [x] **P0** Skeleton: layered architecture, error taxonomy, display list IR, i18n layer
- [x] **P1** OFD structure parsing (`model`, `xml`, `package`, `parser`)
- [x] **P2** Raster render backend, completing the open-to-display path
- [x] **P3** Conversion (OFD to PDF, PNG, SVG)
- [x] **P3b** Printing: task planning and rasterization (`print`)
- [x] **P4b** The `core` facade: one handle, features per delivery form
- [x] **P4** Signature verification with national algorithms
- [x] **P5a** The `steleofd` command line tool
- [x] **P5b** WebAssembly bindings for the browser
- [x] **P5c** C ABI shared library for OA and ERP integration (`ffi`)
- [x] **P5d** HTTP service delivery for OA and ERP back ends (`server`)
- [x] **P6** Document editing with an undo stack
- [x] **P7** Full-text search over the display list (`search`)
- [x] **P8** Archival readiness auditing (`archive`)
- [x] **P9** GB/T 33190-2016 conformance harness: annex A schemas rebuilt from the standard, enforced in CI

All 26 crates are in place, and the five delivery forms (`cli`, `wasm`, `ffi`,
`server`, `reader`) sit on the same `core` facade. What comes next is tracked in
[`docs/ROADMAP.md`](docs/ROADMAP.md), which starts with the reader interface
the engine is built to serve. Clause-level coverage lives in
[`compliance/gb-t-33190.toml`](compliance/gb-t-33190.toml) and the method
behind it in [`compliance/CLAUSES.md`](compliance/CLAUSES.md): 74 clauses are
registered, 66 of them covered, and the eight that are not carry a `deviation`
note saying why.
What the engine can and cannot draw with is set out face by face in
[`docs/FONTS.md`](docs/FONTS.md).

Performance baselines, and how to run them, are in
[`docs/PERFORMANCE.md`](docs/PERFORMANCE.md).

## Contributing

Issues and pull requests are welcome. Before submitting, make sure these pass:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
python3 scripts/check_i18n.py --strict
python3 scripts/check_language.py
```

New code needs tests. Do not introduce `unsafe`; `steleofd-ffi` is the only
documented exemption, because a C ABI requires it. Code, comments, commit
messages and issues are English only: [docs/CONVENTIONS.md](docs/CONVENTIONS.md).

## License

[Apache-2.0](LICENSE)

## Related

- [National standards full-text portal](https://openstd.samr.gov.cn/bzgk/gb/index), where the GB/T texts above can be previewed
- [Industry standards platform](https://std.samr.gov.cn/hb/), for GM/T standards
- [GmSSL](https://github.com/guanzhi/GmSSL), an open source national cryptography implementation
