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

### Rendering

- **A run read downwards keeps its glyphs upright.** Table 47 measures both
  angles from the page's `+x`, and the old placement turned the glyphs by the
  gap between them inside a frame that was itself turned by the read angle, so
  the two turns cancelled: the advances ran along the page instead of down it
  and a vertical heading read sideways. The line now carries the read angle and
  each glyph keeps its own, which puts the buyer and password labels of a
  tax-control code on the vertical bar the document draws for them.
- **A line set downwards states no `DeltaX`.** Figure 63 says so, and a run
  whose offsets only run down the page now reads that way instead of being
  given a horizontal line. `DeltaY` becomes the step along it.
- **`GlyphRun::glyph_matrix` is the one way a glyph is drawn.** The SVG, PDF
  and raster backends each built the same scale-turn-place matrix, so a fix in
  one of them could miss the others; `GlyphRun::glyph_angle` is the new field
  they read it from.

## [0.2.0] - 2026-10-19

The first release since `0.1.1` that changes what the engine draws and what a
signature is allowed to say. Both matter more than anything else here: a
document set in the wrong place is read wrong, and a signature reported valid
is the reason the format is used at all.

### Signatures

- **A signer can be checked against an anchor the caller names.**
  `verify_signature_under` adds a `TrustPolicy`, and
  `steleofd_crypto::TrustAnchors` carries the roots. GB/T 38540-2020 7.3 e) 1)
  requires a certification path and names no root, so the caller supplies it;
  the path check walks each certificate's own signature under its issuer's key
  up to one of those anchors, reading the key identifiers, `basicConstraints`,
  `keyUsage` and signature algorithm that GM/T 0015-2023 5.2.4 asks for. A
  failure at one link stays distinct from an expired certificate, because 7.3
  e) 2) separates them.
- **Two outcomes were added, so a verdict says no more than it checked.**
  `Unchecked` covers a reference naming a digest this build does not compute
  and a certificate that had already expired when the document says it was
  signed — proof of nothing either way — which used to come back as `Tampered`
  and read as an edit of the document. `Untrusted` covers a signer outside the
  caller's anchors. The C struct, the JSON shape, the wasm shape, the server
  DTO and the reader's colour each carry both, with one message per locale,
  because an outcome no shell can express is a verdict only Rust callers see.
- **`verify_signature` keeps its meaning.** It answers whether the digest and
  the certificate hold, and nothing about who issued them, which is what
  `docs/Trust.md` now says instead of the old claim that the issuer is never
  checked.

### Rendering

- **`ReadDirection` and `CharDirection` now place text.** A run was laid out
  along `+x` and read left to right whatever table 47 asked for, so a vertical
  heading or a right-to-left line was drawn as if the document had said nothing.
  `DeltaY` was counted and never used for where a row sits; it now steps a run
  across the line, which is also how a downward line advances when its glyphs
  stay upright.
- **An offset list shorter than the steps places glyphs rather than moving
  them.** A producer writes one offset per column, so the list is shorter than
  the number of steps between characters; read as steps it set a line of text
  as one blob. Such a list now gives where each glyph sits, and `DeltaY` beside
  it places a new row instead of slanting the one above.
- **A code that returns to the left edge is drawn as rows.** A `DeltaX` step
  back ends a row and `DeltaY` says how far down the next one sits, but one run
  carried the return as an advance, so a four-row code came out on a single
  line. The code is now cut into one run per row.
- **Escaped markup in text survives again.** A `&lt;` written in a value
  arrived as its own parser event and was dropped, so a tax-control code lost
  thirteen characters and the reader drew ninety-nine where the document
  carried a hundred and twelve. Entity references are now resolved as text.
- **A named monospaced family is read as fixed width.** Clause 11.1 leaves
  `FixedWidth` optional, and a `Courier New` line was answered with the CJK
  substitute's proportional glyphs, so the columns the offsets build collided.
  A family whose name says mono now asks for a monospaced substitute whether or
  not the flag is set.
- **An advance is measured in a thousandth of an em before shaping.** A face
  whose units-per-em is not 1000 (2048, say) had its width left in raw units,
  so its glyphs ran wider than the offsets stepped them.

### Robustness

- **A document that names a font the backend cannot read is refused, not
  fatal.** `ttf-parser` asserts that a table directory fits inside its file and
  inside 32 bits, and the directory is the document's to choose, so a crafted
  file ended the process where the reader expected an error. Such a font is now
  a parse error like any other.
- **A password-protected entry asks for a password.** The ZIP reader said
  `PASSWORD_REQUIRED` and the mapping filed it as corruption, so a protected
  document read as a damaged one. The encrypted state is now recorded at open
  and tested, rather than recorded and never mentioned again.
- **A name an archive reader would collapse is refused at write time.**
  `build_archive` accepted `A//x.xml`, wrote it, and reported `A/x.xml` on the
  way back, so the writer lost an entry it had just written.

### Tooling

- **Gates for the three things a 690-package tree had none of.** An advisory
  now stops the build, the three parsers a document feeds are fuzzed, and each
  editing command is checked by reopening what it wrote. Dependency upgrades
  that clear the known advisories landed with them: `zip` 4 to 9, `lopdf` 0.36
  to 0.45, `zbus_xml` to 5.2, while `rsa`, `pkcs12` and `indicatif` were
  dropped as unreachable.
- **A break now spends a stated version.** The semver job reads its policy from
  the crates it governs and compares against the parent commit, so the bump a
  break costs is decided when it merges rather than when it ships.
- **Every crate states where it goes.** Nineteen publish and eight do not, each
  carrying the `description` a registry shows, and `check_publish.py` holds the
  two sets to the whole workspace on every push.
- **The Windows reader is cross-compiled from Ubuntu** with `mingw-w64`, which
  matches the target `cargo build` produces locally instead of a Chocolatey
  install on a Windows runner, and the jobs that read the fetched samples now
  fetch them before running.

### Naming

- **`corpus` is now `fixtures` and `samples`.** Nothing in the tree is a text
  corpus, and the word also means a body. Every reference follows the rename,
  including the author, provider and tag namespace inside the fixture archives,
  which is the one place a rename has to reach into bytes.

### Known limits

- No crate is published to a registry yet, so this release is a source tag and
  a set of artefacts rather than a `cargo add steleofd-core`. Publishing waits
  until the surface settles, because a version on a registry cannot be
  replaced.
- Blending is deliberately unsupported: the standard defines no blend mode, and
  `Alpha` is the whole of its transparency model.
- Office formats and OCR are planned, not built.
- iOS is not in the release matrix — it needs signing on macOS, and the project
  holds no keys.
- The release artefacts are unsigned, so trust rests on the checksums.

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
third-party set of 28 documents opens cleanly, and the standard's own clause
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

[Unreleased]: https://github.com/emfox/SteleOFD/compare/v0.2.0...main
[0.2.0]: https://github.com/emfox/SteleOFD/releases/tag/v0.2.0
[0.1.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.1
[0.1.0]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0
[0.1.0-rc.1]: https://github.com/emfox/SteleOFD/releases/tag/v0.1.0-rc.1
