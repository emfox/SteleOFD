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

- **The window is built on a published guideline.** `Ui-spec.md` named no
  application-UI guideline, so a new contributor had to infer the rules from the
  window that happened to exist. It now names the GNOME Human Interface
  Guidelines, lists the clauses this window follows and where each one lands,
  and records the two it does not take and why: `egui` draws windows on five
  platforms, and a GNOME-only widget set would leave four of them with two
  toolkits in one frame.
- **A theme, a text size and high contrast.** The theme offers follow-the-system,
  light and dark rather than two, because a reader who told their desktop to go
  dark has already answered. The text size scales the whole type scale from one
  base, so `130%` moves every style and none can drift out of proportion. High
  contrast is a switch on top of the theme rather than a fourth theme, so a
  reader who needs it keeps the light-or-dark choice they made.
- **Every colour and size is a token.** Colour literals had been spread through
  the window — a red for a failure, a green for a good signature, an amber that
  meant "cannot tell" — and each of them was right in one theme and unreadable
  in the other. They now live in one table with a light and a dark column, and a
  test proves each text-on-panel pair clears 4.5:1 and that high contrast clears
  7:1 without losing the hue a reader recognises.
- **A verdict is never carried by colour alone.** The signature panel writes the
  verdict out and sets a glyph beside it, so it survives a colour-blind reader, a
  monochrome display and a printout.
- **Every control says what it does.** A button that is drawn rather than
  written, so each of them now has a tooltip; and every other button in the two
  header rows.

- **The window is drawn on the guideline's grid, not on the toolkit's.** Spacing
  is four multiples of six points rather than whatever `egui` defaults to, and
  controls are one height with one corner radius. A reader who raises the text
  size gets the spacing raised with it, because a window that kept its gaps
  while the words grew would crowd them.
- **Groups are separated by space, not by rules.** Thirteen separator lines drew
  boxes around parts of panels; the guideline asks for blank space and a heading
  instead, and there are now none.
- **The header bar is a header bar.** Its controls are drawn without a frame —
  the toolbar style the guideline gives header controls — so the bar reads as one
  surface, with the frame appearing under the pointer. The language and the
  export are menus at the end of the bar rather than one button per option, which
  is what makes the bar narrow enough to leave room for dragging and for the
  document itself.
- **Controls now sit where the guideline says each kind belongs.** The header
  bar keeps only what changes the document, a view switcher and one main menu;
  every page action moved to the single toolbar below it, including the export
  buttons that used to live above them for no better reason than being about the
  document. Search, signatures and annotations became an exclusive view switcher
  rather than three independent switches, so picking one cannot leave the reader
  with two panels and half a page.
- **One main menu instead of two menu buttons.** The language and the appearance
  settings were competing for the same corner. Both are now groups inside a
  single menu, ordered the way the guideline orders a primary menu — settings,
  language, then shortcuts and the build's name — with no close or quit entry,
  since the header bar already closes the document.
- **The controls are drawn as icons rather than spelled out.** Twenty glyphs on
  the desktop's 16-unit symbolic grid replace the words and text characters the
  window had used for the same ideas: `◀` and `▶` in three panels, the `✓` `✗` `?`
  `!` beside a verdict, and the labelled export, fit and paging buttons. They are
  geometry, not image files, so a theme switch recolours them and the build
  carries no rasters to blur on a scaled screen. Each keeps its tooltip, which is
  also its accessible name — a drawing has no words of its own, and a screen
  reader reads the drawing's control by that name.

### Performance

- **Time and memory have separate budgets.** `Performance.md` stated five
  timings and no memory ceiling, while the documents this is for are scanned
  invoices of tens of pages and a single A4 is about 26 MB once decoded. A page
  now has a 40 MB ceiling, a twenty-page packet has 512 MB, and both are
  enforced: a budget nobody measures is a budget nobody has.
- **A page's memory is counted, not guessed.** A new `memory` benchmark wraps
  the allocator and prints what a raster holds once it is drawn, which is the
  number the cache has to size itself against. Criterion measures time, so the
  reading is a printout; `check_performance.py` reads it beside the timings.
- **The benchmarks run at twenty pages.** One page is a preview and eight is a
  chapter; twenty is the packet the per-document ceiling is stated for, and the
  first count where a cost per page that had stopped being linear would show.

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
  they read it from. The raster backend kept its own copy for one commit, which
  drew a vertical heading in the right place with every glyph lying on its
  side; a test now turns a glyph a quarter and checks the ink turns with it.

### Export

- **A PDF export keeps every page.** The CLI converted one page at a time and
  assigned each result to the same variable, so the file it wrote held the last
  page only — and under the document's name, reporting success. It now writes
  the selection as one document, so `-p` picks the page to export instead of
  being ignored.
- **Exported glyphs carry the document's own font.** The document-level export
  built its options from the default, which names no font source, so every run
  fell back to live text: readable only where the reader's machine had the same
  face, and wrong everywhere else. The document's font manager now goes with
  it, and `to_pdf_pages_with_stats` reports what the conversion lost.
- **Every page keeps its own physical box.** A receipt is 210 by 140 mm and its
  continuations are A4; one size for the file either cropped a continuation or
  left a band of white under the first page.

### Removed

- **`steleofd-font` no longer shapes text.** The `shape` module, `shape_text`,
  `ShapeOptions`, `ShapedRun` and `ShapedGlyph` are gone, and with them
  `rustybuzz`. A document states its glyphs through clause 11.4's
  `CGTransform`, and a run without one is its code points mapped through the
  face's `cmap` in string order, so a shaper was a second opinion nothing asked
  for — it could disagree with the file about which glyph to draw. This removes
  public items, which is what makes the version `0.3.0` rather than `0.2.1`.
- **`ttf-parser` is gone.** It had been declared unmaintained by its author and
  RustSec recorded it as `RUSTSEC-2026-0192`, so every font a document supplies
  was read by a parser nobody would fix. Fonts are now read through `skrifa`.
  The five queries this layer makes — code point to glyph, units per em,
  advance, outline, reverse `cmap` — answer the same values: the five font
  golden pages render unchanged, and they were committed before the swap
  precisely so that could be asserted rather than assumed. The one difference
  is that an outline no longer repeats the closing line before `Close`, since
  `Close` already returns to the start.

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
