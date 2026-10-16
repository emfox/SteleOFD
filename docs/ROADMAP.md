# Roadmap

SteleOFD has all 26 crates of the original plan built, tested and pushed. The
original P0-P9 sequence is finished; this document is what comes after it.

The plan is organised by horizon rather than by layer, because the remaining
work is no longer "build the next crate" but "make what exists trustworthy,
then make it fast, then make it complete". Each horizon states its exit
criterion, so it is always obvious whether it is done.

Finished tasks are one line: struck through, no explanation. Open tasks carry
the note a person would need to start them.

## Where the project stands

| Measure | Value |
| --- | --- |
| Crates | 26 (L0 foundation through L5 delivery) |
| Source | 38,687 lines of Rust, measured with `find crates -name '*.rs'` |
| Tests | 922 passing, plus 33 inline unit-test modules |
| Delivery forms | `cli`, `wasm`, `ffi`, `server`, `reader`, all on the `core` facade |
| Standard clauses | 74 registered in GB/T 33190-2016, 70 covered, 4 `deviation` with a stated reason |
| Languages | `en-US` and `zh-CN`, 88 message keys, parity enforced in CI |
| CI | fmt, clippy, build, test, all-features test, i18n, language, conformance, XSD validation, corpus freshness, fuzz smoke run |

## Honest gaps

These are the things a new contributor would otherwise discover the hard way.
Each one is a real, verified finding, not a guess.

1. **Two clauses stay short of `verified`.** `8.4` is `partial` because only a
   `Path` clip area is read and a `Text` area is rejected with an explicit
   error; `8.4` is the one place the register and the code disagree on
   purpose. The register in `compliance/gb-t-33190.toml` is the source of
   truth and lists the status of each by name.
2. **The documented format support outran the code, until now.** Five cargo
   features (`render`'s `backend-canvas`/`backend-svg`/`backend-pdf`, and
   `convert`'s `office`/`ocr`) were declared and referenced nowhere, while
   `README` advertised "HTML and Office" conversion. The three empty render
   backends are gone; `office` and `ocr` stay as named placeholders with a
   comment saying nothing consumes them. M6 is where they would land.
3. **The OFD corpus is one file.** `compliance/corpus/minimal.ofd` is the only
   document the XSD validation and the test suites actually read.
4. **`README.zh-CN.md` is behind the English README.** It still shows P3 and P5
   as unfinished and names a next step that no longer exists.
5. **Six crates have no dedicated integration test directory.** `xml`, `model`,
   `font`, `display` and `render` are covered only by inline unit tests, not by
   tests that drive the public API as a consumer would.
6. **No release automation.** No `CHANGELOG`, no version-bump or publish
   workflow, no `cargo-semver-checks` gate. `Cargo.toml` declares `0.1.0`
   everywhere and nothing tracks whether a change breaks a public API.
7. **The reader exists but is barely a reader.** It opens, rasterizes,
   navigates, searches, verifies, exports and takes the keyboard. What is still
   missing is annotation display, document comparison and a packaged build.
   See M7.

---

## Near term: earn the word "done"

Nothing here adds capability. Everything here makes the existing crates
trustworthy to someone who did not write them. Exit criterion: a new
contributor can clone, test, and trust the project without asking a question.

### N1. A shared test kit (N1-a … N1-c)

- **N1-a** `[x]` ~~`steleofd-testkit` carries the OFD zip builder, the base64/DER/SM2-certificate helpers, and the signed and unsigned fixtures.~~
- **N1-b** `[x]` ~~`sign`, `wasm`, `ffi` and `server` import from it; about 800 lines of duplicated builders deleted.~~
- **N1-c** `[x]` ~~It is a `[dev-dependencies]` entry everywhere and declares `publish = false`.~~

### N2. Close the untested-clause gap, lowest risk first (N2-a … N2-e)

- **N2-a** `[x]` ~~Structural clauses `5.4`, `6.1`, `7.1`, `7.2`, `19.1`, `20.1`, `A.1`.~~
- **N2-b** `[x]` ~~Drawing parameters `8.2.1`, `8.2.2`, `8.2.4`, `8.2.5`, `9.2`; three parser bugs fixed.~~
- **N2-c** `[x]` ~~Text and font `11.1`, `11.3`, `11.4`, `7.8`; four flattening gaps fixed.~~
- **N2-d** `[x]` ~~Graphics features `8.3.4`, `8.4`, `9.1`.~~
- **N2-e** `[x]` ~~Feature tail `12`, `13`, `14.1`, `15`, `16`, `17`, `18.2.2`, `18.2.3`; clause count closes at 47 of 47.~~

A clause moves to `verified` only when a linked test actually asserts it. The
gate script stays the single source of truth; `docs/COMPLIANCE.md` is
regenerated, never hand-edited.

### N3. Property and fuzz testing (N3-a … N3-d)

- **N3-a** `[x]` ~~Property tests for `package`, five invariants; found and fixed a name-normalisation round-trip defect.~~
- **N3-b** `[x]` ~~Property tests for `xml` and `model`: reading may fail, never panic; a parsed value survives a `Display` round trip.~~
- **N3-c** `[x]` ~~`cargo-fuzz` targets `package_open` and `xml_cursor` under `fuzz/`, nightly, 20,000 runs in CI.~~
- **N3-d** `[x]` ~~A crash-free invariant test feeding random bytes to every public entry point.~~

### N4. Integration tests for the six thin crates (N4-a … N4-f)

- **N4-a** `[x]` ~~`convert`: 11 tests.~~
- **N4-b** `[x]` ~~`xml`: 21 tests.~~
- **N4-c** `[x]` ~~`model`: 18 tests.~~
- **N4-d** `[x]` ~~`font`: 18 tests.~~
- **N4-e** `[x]` ~~`display`: 17 tests.~~
- **N4-f** `[x]` ~~`render`: 11 tests.~~

### N5. Documentation and release hygiene (N5-a … N5-d)

- **N5-a** `[x]` ~~`README.zh-CN.md` level with `README.md` again.~~ (behind once more; see Honest gaps 4)
- **N5-b** `[x]` ~~`CHANGELOG.md`, following Keep a Changelog, driven by the conventional-commit history.~~
- **N5-c** `[x]` ~~`cargo-semver-checks` in CI, comparing against the latest `v*` tag.~~
- **N5-d** `[x]` ~~A release workflow on a `v*` tag, building `cdylib`, `staticlib` and wasm artefacts.~~

### N6. Corpus and CI (N6-a … N6-c)

- **N6-a** `[x]` ~~`compliance/corpus/features/` joins `minimal/`; found two real defects and grew `--allow-foreign`.~~
- **N6-b** `[x]` ~~The `coverage` job: `cargo-llvm-cov` over the workspace, baseline 87.56% lines with an 85% floor.~~
- **N6-c** `[x]` ~~CI pins toolchain 1.99.0 rather than following `stable`.~~

### N7. The desktop reader, first surface (U1-a … U1-c)

- [x] **U1-a** ~~A window, a menu bar and a document area in `crates/steleofd-reader`, pure `egui`/`eframe` on `core`.~~
- [x] **U1-b** ~~Open via dialog and drag-and-drop, document facts, page counter and navigation wired to `reader-*` strings.~~
- [x] **U1-c** ~~`tests/window.rs` drives the real widget tree through `egui_kittest` with no display.~~

Exit criterion: a person can open an `.ofd` in a native window and read every
page.

### N8. Layout, zoom and search (U2-a … U2-c)

- **U2-a** `[x]` ~~`reader-layout-*` is live; `viewer::visible_pages` is a pure function of layout, page and count.~~
- **U2-b** `[x]` ~~Rendering moved off the draw path into `render_worker`; real zoom and generation-based cancellation.~~
- **U2-c** `[x]` ~~The search panel as zone Z5, hits reduced to page, snippet and rectangle, canvas highlight.~~

### N9. Verification, export and access (U3-a … U3-c)

- **U3-a** `[x]` ~~A signature panel as zone Z6, with three verdicts so an unverifiable signature is amber, not red.~~
- **U3-b** `[x]` ~~Export the visible selection as one PDF or one PNG per page, off the draw path.~~
- **U3-c** `[x]` ~~Full keyboard navigation and a locale switch from the existing catalogues.~~

### N10. Second-implementation corpus (S1-a, S1-b)

- **S1-a** `[x]` ~~`scripts/fetch_corpus.py` pins documents from the reference Java implementation; nothing committed.~~
- **S1-b** `[x]` ~~Twenty-eight documents opened, six constructs fixed, each covered by `tests/third_party.rs`.~~

### N11. Image resources reach the page (S1-c … S1-g)

- **S1-c** `[x]` ~~The `MultiMedia` table is read at open; media bytes resolved and attached to each placement.~~
- **S1-d** `[x]` ~~The image placement transform was composed on the wrong side; case-insensitive entry matching added.~~
- **S1-e** `[x]` ~~JBIG2 through `hayro-jbig2` with `simd` off; `rust-version` moved to 1.92.~~
- **S1-f** `[x]` ~~A clip is the union of every area of every clip, not just the first.~~
- **S1-g** `[x]` ~~`KNOWN_UNPARSED` is empty: all sixteen samples open and convert in three formats.~~

### N12. Where an object's content lands (S2-a)

- **S2-a** `[x]` ~~Content space is the unit square plus a CTM that scales it onto the boundary; clips, masks and the page position all corrected.~~

---

## Mid term: make it fast and complete

Exit criterion: the engine holds a performance budget it is measured against,
and every clause the project claims to implement is either verified or
explicitly documented as a deviation.

### M1. Performance baselines (M1-a … M1-c)

- **M1-a** `[x]` ~~Four benchmark targets over a generated document, reporting per-page and per-object costs.~~
- **M1-b** A documented performance budget: pages per second for render, and the document size the parser handles within a stated latency. `docs/PERFORMANCE.md` holds the measured numbers; the budget is what turns them into a pass/fail line.
- **M1-c** A CI job that runs the benchmarks and fails only on a large, deliberate regression.

### M2. Font pipeline hardening (M2-a … M2-c)

- **M2-a** `[x]` ~~Faces resolved by declared id, bare CFF read, and the CJK/Latin/RTL matrix in `docs/FONTS.md`.~~
- **M2-b** Subset embedded fonts instead of carrying whole faces. A document that embeds a 20 MB CJK collection pays for it in every conversion.
- **M2-c** `[ ]` **mostly open.** The chain is built (`build_fallback_chain`, wired into `Document::open`), every backend reports runs that reach the page short a glyph (`missing_glyph_runs`), a run whose face will not load costs its own run rather than the page (`skipped_font_runs`), and the raster backend now matches the vector ones. What is left is the reader that acts on the chain: nothing re-shapes a run against a fallback face yet, so a code point the run's own face lacks is still dropped rather than looked up in the other faces. Two smaller limits ride along: the registry is filled only from the ids the pages name, so a declared-but-unused face never joins the chain, and a substitute is chained only where one can be resolved. See `docs/FONTS.md` gap 4.

### M3. Rendering completeness (M3-a … M3-d)

- **M3-a** Colour spaces beyond sRGB, and CMYK for print. The declaration side is done: clause 7.13's `ColorSpaces` table is read at open and `ParsedDocument::color_spaces` carries each space's type, depth, profile and palette. What is left is the painting side — no backend consumes the table yet, so an image naming a CMYK space is still decoded as sRGB.
- **M3-b** Blending and transparency modes.
- **M3-c** Bring each test-verified drawing feature to full parity. The first is done: `RadialShd` end centre and radius now travel with the op and each backend reduces the cone to what it can paint.
- **M3-d** A golden-image suite: render the corpus to PNG, compare against committed references, gate on a pixel-difference threshold.
- **M3-e** Move the JBIG2 codec out of `display`. The IR crate carries a full decoder (`display/src/jbig2.rs`, via `hayro-jbig2`) that only `render` and `convert` call; the IR layer should hold data structures, not codecs. A small `steleofd-codec` crate (or an existing lower one) is the home. Deferred because it touches `display`'s public surface and three downstream import paths.
- **M3-f** The three shading and fill models still standing as deviations: `Pattern` (8.3.5), `GouraudShd` (8.3.6) and `LaGouraudShd` (8.3.7). Each needs a painter, not just a reader — a tiling model for `Pattern`, a triangle and a lattice mesh for the two Gouraud forms. The reader currently recognises and refuses them (`unsupported_shading`), which is honest but not coverage. `8.1.6` PrintQuality stays a deviation for a different reason: the element is absent from the vendored Annex A schema entirely, so there is nothing to read.

### M4. Signature depth (M4-a … M4-c)

- **M4-a** `18.2.2` signature scope and `18.2.3` signature appearance, verified and implemented rather than tolerated.
- **M4-b** Seal and incremental update, so a signed document can be extended without breaking its signature.
- **M4-c** Timestamp countersignature verification.

### M5. Server production readiness (M5-a … M5-e)

- **M5-a** Replace the in-memory store with a persistent one.
- **M5-b** Authentication and per-tenant authorisation.
- **M5-c** Rate limiting and a real request timeout budget.
- **M5-d** OpenAPI documentation generated from the route definitions.
- **M5-e** Graceful drain, so a rolling deploy does not cut a render in half.

### M6. Interoperability (M6-a … M6-c)

- **M6-a** Test against documents produced by other real OFD implementations, not only our own builder.
- **M6-b** Round-trip tests against the standard's own examples, if the working group publishes any.
- **M6-c** Office conversion. The `office` feature on `convert` and `core` is a named placeholder that compiles and changes nothing; this is where it gains an implementation. OCR (the `ocr` feature) rides with it.

### M7. The reader, completed

- **M7-a** Annotation display and navigation, once clause 15 is verified.
- **M7-b** Signature appearance rendering, once `18.2.3` is implemented.
- **M7-c** A document comparison view for two revisions of the same file.
- **M7-d** Print, straight from the reader's current view.
- **M7-e** Signed, installable builds for the platforms people actually deploy on, and an offline story.
- **M7-f** Move the reader's interface strings out of `steleofd-i18n`. The engine crate defines product-facing ids (`ReaderFitWidth`, `ReaderSearchTitle`, …); they belong with the reader, so the engine catalogue holds engine messages only and the two change at their own pace. Deferred because it moves `i18n`'s public API, the CI parity script and every reader call site at once.

### M8. Repository hygiene

- **M8-a** Split the aging files. `convert/src/pdf.rs` (1,442), `parser/src/content.rs` (1,365), `render/src/raster.rs` (1,312) and `reader/src/app.rs` (1,055) have outgrown one screen of scrolling. Pure reorganisation, no behaviour change; worth one deliberate pass rather than a drip.

---

## Far term: become the reference implementation

Exit criterion: a third party can build an application on SteleOFD without
reading its source.

### F1. Developer experience

Task-based documentation with a worked example per delivery form, and examples
that are compiled in CI so they cannot rot. A screenshot-driven tour of the
desktop reader, generated from the same build the tests exercise.

### F2. Extensibility

A public API for custom colour profiles, font providers and render backends,
so an organisation can plug in its own without forking the engine. The command
stream render backend (a browser re-zooms vectors instead of resampling a
bitmap) is a candidate for this surface; it is not declared until it exists.

### F3. Standards movement

Track the revisions of GB/T 33190 and the related OFD series, and keep the
annex A schemas regenerable from the standard text rather than hand-edited.

### F4. Ecosystem

Package releases for the platforms people actually deploy on, a Java binding
alongside the C ABI for the OA systems that need one, and a stable plugin
point for third-party signature providers.

### F5. Other shells for the same engine

`egui` is a build decision, not a belief. The engine is a library, and these
are the other ways an organisation may reasonably want to put a front end on
it. Each is a separate crate that depends on `core`; none of them changes the
engine.

- **F5-a** A Tauri shell, for organisations that want a web front end with a
  native window. It buys CSS layout and accessibility for free, at the cost of
  a second toolchain and webkit2gtk in CI.
- **F5-b** A Qt or GTK binding, for desktops that embed the reader in an
  existing native application rather than shipping a standalone one.
- **F5-c** A browser shell, the reader built on the `wasm` bindings, for the
  cases where a document may not leave the user's machine as a file — served
  from an intranet with the document never uploaded.

F5-c is the one that overlaps with the existing `wasm` crate. `wasm` is the
engine crossing into a browser; a browser *reader* is a product built on it.

---

## Sequencing

Near term runs in order, because N1 unblocks N2 and N4, and N2 is what makes
the conformance number in the README mean something. N3 and N5 can run
alongside it, since they touch different crates.

The reader (N7 to N9) depends on the `core` facade, which is finished, so it
does not wait on N2: the interface needs a working renderer, not a
conformance number. U1 can proceed while the clause work runs. M7 depends on
N2, because annotations and signature appearance are clauses 15 and 18.2.3,
and there is no point drawing a panel for a feature that is not yet verified.

Mid term starts when the near-term exit criterion is met. M1 first, because a
performance budget cannot be enforced without a baseline to measure against.

Far term is deliberately not scheduled. It describes a direction, not dates.

## How to propose changes

Roadmap changes follow the same rule as code: open an issue, state the
problem, name the exit criterion. A task without a verifiable exit criterion
does not belong here.
