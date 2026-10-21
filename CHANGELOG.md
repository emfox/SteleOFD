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

### Reader interface

- **The window follows the GNOME Human Interface Guidelines**, with the two clauses it does not take recorded in `Ui-spec.md`.
- **A theme (follow-the-system, light, dark), a text size and a high contrast switch**, with every colour and size a token and each text-on-panel pair held to 4.5:1.
- **A verdict is written out beside a glyph**, so colour alone never carries it.
- **The header bar holds only what changes the document**: one view switcher, one main menu, and every page action on the toolbar below it.
- **Twenty icon glyphs on the symbolic grid** replace the words, each keeping its tooltip, which is also its accessible name.

### Performance

- **Time and memory have separate budgets**, 40 MB for a page and 512 MB for a twenty-page packet, both enforced.
- **A `memory` benchmark counts what a drawn page holds**, because criterion measures time and not bytes.
- **The benchmarks run at twenty pages**, where a cost per page that had stopped being linear would show.

### Rendering

- **A run read downwards keeps its glyphs upright**, and a line set downwards states no `DeltaX`.
- **`GlyphRun::glyph_matrix` is the one way a glyph is drawn**, so a fix cannot reach one backend and miss the other two.

### Export

- **A PDF export keeps every page**, where the CLI had been overwriting each page with the last.
- **Exported glyphs carry the document's own font**, instead of falling back to live text.
- **Every page keeps its own physical box**, so a receipt and its A4 continuations are both drawn whole.

### Removed

- **`steleofd-font` no longer shapes text.** `rustybuzz` went with `shape_text`, because clause 11.4's `CGTransform` already says which glyph to draw. This is what makes the version `0.3.0`.
- **`ttf-parser` is gone**, replaced by `skrifa` after RustSec recorded its author declaring it unmaintained. The five font golden pages render byte for byte unchanged.

### Tooling

- **Gates for advisories, fuzzing and edit round-trips**, which a 690-package tree had none of.
- **A semver job whose bump is decided when a break merges**, not when it ships.
- **Nineteen crates publish and eight do not**, each stating a `description`, enforced by `check_publish.py`.
- **The Windows reader is cross-compiled from Ubuntu** with `mingw-w64`.
- **The performance gate runs the benchmarks** rather than reading whatever the last run left in `target/criterion`.
- **An export test proves a glyph is a path, not an embedded face**, replacing the render crate's stale TODO.
- **`corpus` is now `fixtures` and `samples`**, through to the namespace inside the archives.

### Known limits

- No crate is published to a registry yet, so this release is a source tag and a set of artefacts rather than a `cargo add steleofd-core`.
- Blending is deliberately unsupported: the standard defines no blend mode, and `Alpha` is the whole of its transparency model.
- Office formats and OCR are planned, not built.
- iOS is not in the release matrix, and the release artefacts are unsigned.

## [0.2.0] - 2026-10-19

The first release since `0.1.1` that changes what the engine draws and what a
signature is allowed to say.

### Signatures

- **A signer can be checked against an anchor the caller names.** `verify_signature_under` adds a `TrustPolicy`, because GB/T 38540-2020 7.3 e) 1) requires a certification path and names no root.
- **Two outcomes were added, so a verdict says no more than it checked.** `Unchecked` and `Untrusted` reach the C, JSON, wasm, server and reader surfaces, where the first had been reported as `Tampered`.
- **`verify_signature` keeps its meaning**: whether the digest and the certificate hold, and nothing about who issued them.

### Rendering

- **`ReadDirection` and `CharDirection` now place text**, so a vertical heading or a right-to-left line is no longer drawn along `+x`.
- **An offset list shorter than the steps places glyphs rather than moving them.**
- **A code that returns to the left edge is drawn as rows.**
- **Escaped markup in text survives**, where a `&lt;` had been dropped as a parser event.
- **A named monospaced family is read as fixed width**, whether or not `FixedWidth` is set.
- **An advance is measured in a thousandth of an em**, whatever the face's units-per-em.

### Robustness

- **A font whose table directory does not fit is a parse error, not a crash.**
- **A password-protected entry asks for a password** instead of reading as corruption.
- **A name an archive reader would collapse is refused at write time.**

### Tooling

- **Gates for advisories, fuzzing and edit round-trips**, and the dependency upgrades that clear the known advisories (`zip` 9, `lopdf` 0.45, `zbus_xml` 5.2).
- **A semver job whose bump is decided when a break merges**, not when it ships.
- **Nineteen crates publish and eight do not**, each stating a `description`, enforced by `check_publish.py`.
- **The Windows reader is cross-compiled from Ubuntu** with `mingw-w64`.
- **The performance gate runs the benchmarks** rather than reading whatever the last run left in `target/criterion`.
- **An export test proves a glyph is a path, not an embedded face**, replacing the render crate's stale TODO.
- **`corpus` is now `fixtures` and `samples`**, through to the namespace inside the archives.

### Known limits

- No crate is published to a registry yet, so this release is a source tag and a set of artefacts rather than a `cargo add steleofd-core`.
- Blending is deliberately unsupported: the standard defines no blend mode, and `Alpha` is the whole of its transparency model.
- Office formats and OCR are planned, not built.
- iOS is not in the release matrix, and the release artefacts are unsigned.

## [0.1.1] - 2026-10-18

A repair release for `0.1.0`: no behaviour changed, and both defects were in what the release said about itself.

- **The release page carried no notes**, because the first release had nothing to diff against. The notes now come from this file through `scripts/release_notes.py`.
- **`repository` was a placeholder** naming `example.com`; it now names this repository.

## [0.1.0] - 2026-10-17

The first release, cut from `0.1.0-rc.1`: the two tags differ in the documentation and in no code at all.

### What this is

SteleOFD implements [GB/T 33190-2016](https://openstd.samr.gov.cn/bzgk/gb/newGbInfo?hcno=3AF6682D939116B6F5EED53D01A9DB5D), the Chinese national standard for fixed-layout electronic documents, in safe Rust: the tree carries `#![forbid(unsafe_code)]`, and the only C library under it is fontconfig. Twenty-six crates sit in layers that only ever point downward, and [`docs/Architecture.md`](docs/Architecture.md) draws them out.

### Reading

- Open a document, flatten its pages to a display list, and render at any zoom without loss.
- All three chapter 8.3 fill models, every colour space the standard defines, and `RadialShd` carried intact to each backend.
- Large documents load in shards, so a long file is not resident all at once.
- Font fallback resolves through a `FontConfig` list, and a glyph no face covers is reported rather than stretched.
- Font subsetting rebuilds an embedded face down to the glyphs the document uses (28 648 to 2 804 bytes on the sample).

### Writing and conversion

- `steleofd-convert` replays a display list into SVG, PDF and PNG, so export and re-parse stay one code path.
- `steleofd-edit` gives command-pattern editing with undo and permission checks, `steleofd-search` indexes display-list text, and `steleofd-print` rasterizes at print resolution.

### Signatures

- SM3, SM4, SM2 and X.509 primitives in `steleofd-crypto`, with the international algorithms behind the same interface.
- `steleofd-sign` verifies SM3+SM2 check values, reading the check method and signed-value path from the schema rather than assuming them.
- Seal and incremental update, so a signed document can carry a countersignature (`18.2.4`).
- GB/T 20520 timestamp tokens read as a CMS `SignedData` and given four verdicts through `Document::verify_timestamps`.

### Archiving

- `steleofd-archive` audits a container for archival readiness, and `archive()` produces the GB/T 42133-2022 profile (`6.2.2`) with `DocType="OFD-A"`.

### Delivery

| Form | Crate | What it gives you |
|---|---|---|
| CLI | `steleofd-cli` | The engine as the `steleofd` command |
| WebAssembly | `steleofd-wasm` | The engine in a browser page, over the same frame code |
| C ABI | `steleofd-ffi` | The engine behind a committed header, `steleofd.h` |
| HTTP | `steleofd-server` | Documents over HTTP: a persistent store, per-tenant API keys, rate limiting, graceful drain and OpenAPI docs held to the router |
| Desktop and mobile | `steleofd-reader` | A window over the engine: drag to open, print, annotations, a two-revision comparison, full keyboard navigation |

The reader is built for every platform `egui` runs on, and every artefact ships beside a `SHA256SUMS` manifest.

### Safety

- OFD files arrive from arbitrary sources, so the container and parser are hardened first: Zip Slip and zip-bomb defences, a nesting ceiling in the XML reader, and a pixel budget guarding hostile page boxes.
- `proptest` suites, a crash-free invariant over every parser entry point, and `cargo-fuzz` targets for the ZIP and XML readers, all in CI.
- A golden-image suite, the annex A schemas rebuilt from the standard, and the clause register gated.
- `cargo-semver-checks` against the last tag, a coverage floor, and the minimum Rust version built in CI.

### Interoperability

A nine-page golden book is compared against another vendor's reader and a second implementation's source, which showed four gaps in the oracle and three in our own reading; the evidence is in [`docs/Interop.md`](docs/Interop.md). The third-party set of 28 documents opens cleanly.

### Known limits

- No crate is published to a registry yet.
- Blending is deliberately unsupported: the standard defines no blend mode, and `Alpha` is the whole of its transparency model.
- Office formats and OCR are planned, not built.
- iOS is not in the release matrix, and the release artefacts are unsigned.

[Unreleased]: https://github.com/emfox/SteleOFD/compare/v0.2.0...main
[0.2.0]: https://github.com/emfox/SteleOFD/releases/tag/v0.2.0
[0.1.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.1
[0.1.0]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0
[0.1.0-rc.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0-rc.1
