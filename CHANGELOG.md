# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

A release is described by what it offers, grouped by capability, rather than by
the conventional-commit types that produced it: for a version that is still the
first, `Added` would list everything and `Changed` would list nothing a reader
can act on. Later versions keep that shape and add a `Changed` or `Removed`
section only when a release actually has one.

## [Unreleased]

## [0.1.1] - 2026-10-18

A repair release for `0.1.0`: no behaviour changed, and the two defects were
both in what the release said about itself rather than in what it does.

- **The release page carried no notes.** The workflow asked GitHub to generate
  them from the commit range, and the first release has no previous release to
  compare against, so `v0.1.0` published with an empty body. The notes now come
  from this file, through `scripts/release_notes.py`, so the release page and
  the changelog are one text from one commit. The script also makes the
  changelog's relative links absolute, which a release page needs and a
  repository file does not.
- **`repository` was a placeholder.** Every crate inherited
  `https://example.com/ofd/steleofd-rs`, which is not a repository. It now names
  this one, which is what a published `Cargo.toml` and docs.rs read.

## [0.1.0] - 2026-10-17

The first release. It is cut from `0.1.0-rc.1`: **the two tags differ in the
documentation and in no code at all**. This file and `docs/Roadmap.md` are
rewritten, and the documents under `docs/` are renamed `Conventions.md` rather
than `CONVENTIONS.md`. The
candidate was a candidate because no crate has been published yet and the
interfaces may still move, not because anything known was missing.

### What this is

SteleOFD implements [GB/T 33190-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D),
the Chinese national standard for fixed-layout electronic documents. Parsing,
rendering, cryptography and writing are all written from scratch in safe Rust:
the tree carries `#![forbid(unsafe_code)]`, and the only C library under it is
fontconfig, reached through `font-kit` by the reader on Linux.

26 crates sit in layers, and a dependency only ever points downward, which the
compiler enforces. L0 holds value types, errors and I/O; L1 is the format
(model, XML, ZIP); L2 parses and renders into a display-list IR; L3 holds the
optional capabilities (sign, crypto, convert, edit, search, archive, print),
each behind its own feature; L4 is `steleofd-core`, the one entry point that
trims the engine by feature; L5 is the delivery forms, because a CLI, a browser
page, a C header, an HTTP service and a window can share an engine but not a
shape. [`docs/Architecture.md`](docs/Architecture.md) draws it out.

### Reading

- Open a document, flatten its pages to a display list, and render at any zoom
  without loss.
- All three chapter 8.3 fill models — `Pattern`, `GouraudShd`, `LaGouraudShd` —
  plus every colour space the standard defines, index palettes, and `RadialShd`
  carried intact to each backend.
- Large documents load in shards, so a long file is not resident all at once.
- Font fallback resolves through a `FontConfig` list, so a document naming a
  face it does not embed still shows its text, and a glyph no available face
  covers is reported rather than quietly stretched.
- Font subsetting rebuilds an embedded face down to the glyphs the document uses
  (28 648 → 2 804 bytes on the sample).

### Writing and conversion

- `steleofd-convert` replays a display list into SVG, PDF and PNG, so export and
  re-parse stay one code path.
- `steleofd-edit` gives command-pattern editing with undo and permission checks.
- `steleofd-search` indexes display-list text for full-text search.
- `steleofd-print` plans print jobs and rasterizes them at print resolution.

### Signatures

- SM3, SM4, SM2 and X.509 primitives in `steleofd-crypto`, with the
  international algorithms available through the same interface.
- `steleofd-sign` reads signature lists and verifies SM3+SM2 check values; the
  check method and signed-value path are read from the schema, not assumed.
- Seal and incremental update, so a signed document can carry a countersignature
  (`18.2.4`), and the seal appearance renders on the page.
- GB/T 20520 timestamp tokens are read as a CMS `SignedData` around a
  `TSTInfo` and given four verdicts — `Absent`, `Unbound`, `Unverified`,
  `Bound` — through `Document::verify_timestamps`.

### Archiving

- `steleofd-archive` audits a container for archival readiness, and `archive()`
  produces the GB/T 42133-2022 profile (`6.2.2`): it strips the optional
  structures that clause names, drops the attachments that leaves unreachable,
  and declares `DocType="OFD-A"`.

### Delivery

| Form | Crate | What it gives you |
|---|---|---|
| CLI | `steleofd-cli` | The engine as the `steleofd` command |
| WebAssembly | `steleofd-wasm` | The engine in a browser page, over the same frame code |
| C ABI | `steleofd-ffi` | The engine behind a committed header, `steleofd.h` |
| HTTP | `steleofd-server` | Documents over HTTP: a store that survives restart, API-key auth scoped per tenant, rate limiting, a request timeout budget, graceful drain, and OpenAPI docs held to the router's route list |
| Desktop and mobile | `steleofd-reader` | A window over the engine: open by drop, print from the current view, an annotation list, a two-revision comparison view, full keyboard navigation |

The reader is built for every platform `egui` runs on — desktop, a browser page
over the same wasm build, and an arm64 Android archive — each from the crate's
own entry point, so window, renderer and engine are one code path everywhere.
Every artefact ships beside a `SHA256SUMS` manifest; the builds are unsigned, so
the checksum is the trust anchor.

### Safety

- OFD files arrive from arbitrary sources, so the container and parser are
  hardened first: Zip Slip and zip-bomb defences, a nesting ceiling in the XML
  reader, a pixel budget guarding hostile page boxes.
- `proptest` suites over `package`, `xml` and `model`, a crash-free invariant
  test over every parser entry point, and `cargo-fuzz` targets for the ZIP and
  XML readers, run in CI.
- A golden-image suite compares rendering against committed PNGs, the annex A
  schemas are rebuilt from the standard and validated, and the GB/T 33190
  clause register is gated.
- `cargo-semver-checks` runs against the last tag, a coverage floor is enforced,
  and the minimum Rust version is built in CI as well as checkable locally.

### Interoperability

A nine-page golden book is compared against another vendor's reader and a second
implementation's source, which showed four gaps in the oracle and three in our
own reading; the evidence is in [`docs/Interop.md`](docs/Interop.md). The
third-party corpus of 28 documents opens cleanly, and the standard's own clause
samples are parsed in CI on every push.

### Known limits

- No crate is published to a registry yet, so this release is a source tag
  and a set of artefacts rather than a `cargo add steleofd-core`.
- Blending is deliberately unsupported: the standard defines no blend mode, and
  `Alpha` is the whole of its transparency model.
- Office formats and OCR are planned, not built.
- iOS is not in the release matrix — it needs signing on macOS, and the project
  holds no keys.
- The release artefacts are unsigned, so trust rests on the checksums.

[Unreleased]: https://github.com/emfox/SteleOFD/compare/v0.1.1...main
[0.1.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.1
[0.1.0]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0
[0.1.0-rc.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0-rc.1
