# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is driven by the conventional-commit history: a `feat` lands under
**Added**, a `fix` under **Fixed**, and a breaking change is called out on its
own. It records what a downstream user would notice, not the internal
step-by-step.

## [Unreleased]

Everything below is unreleased: the workspace builds and every crate is in
place, but no crate has been published yet. The engine is functional;
the interfaces may still move before `0.1.0`.

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

[Unreleased]: https://github.com/emfox/SteleOFD/commits/main
