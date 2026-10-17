# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is driven by the conventional-commit history: a `feat` lands under
**Added**, a `fix` under **Fixed**, and a breaking change is called out on its
own. It records what a downstream user would notice, not the internal
step-by-step.

## [Unreleased]

## [0.1.0-rc.1] - 2026-10-17

The first release candidate: the workspace builds, every crate is in place and
the original P0–P9 sequence is complete, but no crate has been published yet.
The interfaces may still move before `0.1.0`. This is the first cut with a
reader for every platform `egui` runs on, so the release page is worth a look.

### Added

- **`steleofd-package`** — open OFD containers and expose archive entries, with
  Zip Slip and zip-bomb defences.
- **`steleofd-xml`** — a typed pull parser for OFD documents, prefix-agnostic and
  with a nesting-depth ceiling.
- **`steleofd-model`** — the OFD document data model, mirroring GB/T 33190-2016.
- **`steleofd-parser`** — load OFD documents and flatten pages to display lists;
  readers for the video, composite, action and mark clauses.
- **`steleofd-render`** — a raster backend for display lists, with a pixel
  budget guarding hostile page boxes.
- **`steleofd-crypto`** — SM3, SM4, SM2 and X.509 primitives.
- **`steleofd-sign`** — read signature lists and verify SM3+SM2 check values.
- **`steleofd-edit`** — command-pattern document editing with undo and
  permissions.
- **`steleofd-search`** — index display list text for full-text search.
- **`steleofd-archive`** — audit containers for archival readiness.
- **`steleofd-convert`** — replay display lists into SVG, PDF and PNG.
- **`steleofd-print`** — plan print jobs and rasterize them at print resolution.
- **`steleofd-core`** — open documents behind a feature-trimmed facade.
- **`steleofd-cli`** — drive the engine from the `steleofd` command.
- **`steleofd-wasm`** — expose the engine to the browser.
- **`steleofd-ffi`** — expose the engine to C hosts through a committed header.
- **`steleofd-server`** — serve documents over HTTP.
- **`steleofd-reader`** — a native window over the engine, open by drop.
- **Reader, completed** — print from the current view, an annotation list, seal
  appearance rendering, a two-revision comparison view and full keyboard
  navigation, all over the same `core` facade.
- **`steleofd-crypto` / `steleofd-sign`** — GB/T 20520 timestamp tokens, read as
  a CMS `SignedData` around a `TSTInfo`, with a four-way verdict (`Absent`,
  `Unbound`, `Unverified`, `Bound`) exposed as `Document::verify_timestamps`.
- **`steleofd-archive`** — `archive()` produces the GB/T 42133-2022 archival
  profile: it strips the optional structures clause 6.2.2 names, drops the
  attachments that leaves unreachable, and declares `DocType="OFD-A"`.
- **Font subsetting** — `steleofd-font::subset` rebuilds a face down to the
  glyphs a document uses, carrying whole only a face it cannot rewrite.
- **Release manifests** — `release.yml` builds the reader for every platform
  `egui` runs on. The browser build is a page over the same frame, the Android
  build is an arm64 archive signed with a throwaway key, and both start from
  the crate's own entry point, so the window, the renderer and the engine are
  the same code. Each artefact ships beside a `SHA256SUMS` manifest; the builds
  are unsigned, so the checksum is the trust anchor.
- **Conformance** — annex A schemas rebuilt from the standard and validated in
  CI; the GB/T 33190 clause register gate.
- **`proptest` suites** for `package`, `xml` and `model`, and a crash-free
  invariant test over every parser entry point.
- **`cargo-fuzz` targets** for the ZIP and XML readers, with a CI smoke job.

### Fixed

- **`steleofd-package`** — `build_archive` refused to write an entry name its own
  reader would reject, so a document could be stored and never read back.
- **`steleofd-parser`** — the video, composite and action clauses, which had been
  read incorrectly against the schema.
- **`steleofd-sign`** — the signature descriptor's check method and signed-value
  path, read from the schema rather than assumed.
- **`steleofd-parser`** — a `PathObject`'s absent `Fill` and `Stroke` took the
  wrong defaults, so a path the standard leaves stroked-only was filled and a
  fill-only path was stroked. The `CT_Path` defaults are `Stroke` true and
  `Fill` false; the two other graphic units keep their own.

### Changed

- Commit subjects are measured in bytes and capped at 75; the commit-message rule
  is enforced in CI rather than trusted.

### Capability, by area

The section above says what a downstream user of the libraries would notice.
This one records the work each area of the engine received, which is what the
roadmap used to carry per task. It is grouped rather than chronological, and
it names the result rather than the milestone that produced it.

- **Correctness** — a placed `CompositeObject` draws its own vector resource
  (`13.1`); a `Text` clip area emits its own op so every backend resolves the
  glyph outlines (`8.4`); the standard's own clause samples are fetched and
  parsed in CI, which found GIF, undecodable-image and `./`-rooted `DocRoot`
  defects; the third-party corpus of 28 documents opened with six constructs
  fixed.
- **Enforcement** — a golden-image suite comparing against committed PNGs, which
  found a gradient painting black; a documented per-stage performance budget
  with a CI check that fails only on a large regression; `cargo-semver-checks`
  against the last tag; a coverage floor; a pinned toolchain; and the minimum
  Rust version built in CI and checkable locally, so `rust-version` is tested
  rather than restated by hand.
- **Fonts** — a fallback chain resolved through `FontManager::outline_for_code`,
  so a document naming a face it does not ship still shows its text; the family
  list moved into a configurable `FontConfig` rather than the search code; a
  miss is now reported instead of stretching a face over text it lacks; and
  `steleofd-font::subset` rebuilds an embedded face down to the glyphs a
  document uses (28 648 → 2 804 bytes on the sample).
- **Signatures** — seal and incremental update, so a signed document can carry a
  countersignature (`18.2.4`); GB/T 20520 timestamp countersignature
  verification with a four-way verdict (`Absent`, `Unbound`, `Unverified`,
  `Bound`); and the GB/T 42133-2022 archival profile (`6.2.2`).
- **Server** — a persistent store that survives a restart, API-key
  authentication with per-tenant scoping, rate limiting and a request timeout
  budget, a graceful drain, and OpenAPI documentation held to the same route
  list as the router.
- **Rendering** — colour spaces painted for fill and stroke values, index
  palettes honoured; the three chapter 8.3 fill models (`Pattern`, `GouraudShd`,
  `LaGouraudShd`); `RadialShd` end centre and radius carried to every backend,
  meshes reaching PDF as `ShadingType 4` and patterns reaching SVG as native
  `<pattern>`; the JBIG2 codec moved out of the IR crate into `steleofd-io`.
  Blending is recorded as deliberately unsupported: the standard defines no
  blend mode, and `Alpha` is the whole of its transparency model.
- **Reader** — print from the current view, an annotation list that turns to the
  page it names, seal appearance rendering on each page's raster, a two-revision
  comparison view, and keyboard navigation. It now runs on every platform
  `egui` does: desktop, a browser page over `wasm`, and an Android archive.
- **Interoperability** — a nine-page golden book compared against another
  vendor's reader and a second implementation's source, which showed four gaps
  in the oracle and three in our own reading; the full evidence is in
  [`docs/INTEROP.md`](docs/INTEROP.md).
- **Repository** — the four aging source files split into module roots with no
  behaviour change, the largest now under 700 lines; a shared `steleofd-testkit`
  replacing about 800 lines of duplicated builders; property and fuzz tests over
  every parser entry point; and integration tests for the six thin crates.

[Unreleased]: https://github.com/emfox/SteleOFD/compare/v0.1.0-rc.1...main
[0.1.0-rc.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0-rc.1
