# Roadmap

SteleOFD has all 26 crates of the original plan built, tested and pushed. The
original P0-P9 sequence is finished; this document is what comes after it.

The plan is organised by horizon rather than by layer, because the remaining
work is no longer "build the next crate" but "make what exists trustworthy,
then make it fast, then make it complete". Each horizon states its exit
criterion, so it is always obvious whether it is done.

## Where the project stands

| Measure | Value |
| --- | --- |
| Crates | 26 (L0 foundation through L5 delivery) |
| Source | 30,943 lines of Rust, measured with `find crates -name '*.rs'` |
| Tests | 770 passing, plus 13 inline unit-test modules |
| Delivery forms | `cli`, `wasm`, `ffi`, `server`, `reader`, all on the `core` facade |
| Standard clauses verified | 47 of 47 in GB/T 33190-2016 (46 `verified`, 1 `partial`) |
| Languages | `en-US` and `zh-CN`, 79 message keys, parity enforced in CI |
| CI | fmt, clippy, build, test, all-features test, i18n, language, conformance, XSD validation, corpus freshness, fuzz smoke run |

## Honest gaps

These are the things a new contributor would otherwise discover the hard way.
Each one is a real, verified finding, not a guess.

1. **Every one of the 47 standard clauses now has a test asserting it**, up from
   20 before N2. One stays honestly short of `verified`: `8.4` is `partial`
   because only a `Path` clip area is read and a `Text` area is rejected with an
   explicit error. The register in `compliance/gb-t-33190.toml` is the source of
   truth and lists the status of each by name.
2. **`criterion` is declared in the workspace but never used.** `proptest` is
   now in use (N3-a/N3-b), but benchmarks still do not exist: zero references to
   `criterion` outside the root `Cargo.toml`.
3. **The OFD corpus is one file.** `compliance/corpus/minimal.ofd` is the only
   document the XSD validation and the test suites actually read.
4. ~~**The signed-fixture builder is duplicated four times** across the `sign`,
   `wasm`, `ffi` and `server` test suites, each with its own hand-rolled base64,
   DER and SM2 certificate construction.~~ Closed by N1: `steleofd-testkit`
   holds all of it and the four suites import from there.
5. **`README.zh-CN.md` is behind the English README.** It still shows P3 and P5
   as unfinished and names a next step that no longer exists.
6. **Six crates have no dedicated integration test directory.** `convert`
   (2,848 lines), `xml`, `model`, `font`, `display` and `render` are covered
   only by inline unit tests, not by tests that drive the public API as a
   consumer would.
7. **No release automation.** No `CHANGELOG`, no version-bump or publish
   workflow, no `cargo-semver-checks` gate. `Cargo.toml` declares `0.1.0`
   everywhere and nothing tracks whether a change breaks a public API.
8. **The reader exists but is barely a reader.** `crates/steleofd-reader`
   (U1-a, U1-b) opens a document, rasterizes pages, navigates and caches, and
   consumes the `locales/*/reader.ftl` strings. What is still missing is
   everything below the surface: search, signatures, annotations, export,
   keyboard shortcuts, accessibility, and a layout that actually honours the
   facing and continuous choices in the menu. The window's zones and frame
   flow are specified in [`UI-SPEC.md`](UI-SPEC.md); the rules there are what
   the remaining reader work will be built against.

---

## Near term: earn the word "done"

Nothing here adds capability. Everything here makes the 25 existing crates
trustworthy to someone who did not write them. Exit criterion: a new
contributor can clone, test, and trust the project without asking a question.

### N1. A shared test kit (N1-a … N1-c)

Extract the four copies of the fixture builder into a `steleofd-testkit`
crate, so a document shape is defined once and each delivery crate pins the
same bytes.

- **N1-a** `[x]` ~~`crates/steleofd-testkit` carries the OFD zip builder, the base64/DER/SM2-certificate helpers, and the signed and unsigned fixtures. It brings its own two self-tests, one of which verifies the signed fixture with the engine's own `verify_signature`, so the fixture cannot drift from the engine without a red test.~~
- **N1-b** `[x]` ~~`sign`, `wasm`, `ffi` and `server` now import from it, and about 800 lines of duplicated builders were deleted. The suites kept their case counts, except `sign`, which regained two `18.2.4` cases the migration had dropped in silence — the conformance register is what caught that.~~
- **N1-c** `[x]` ~~It is a `[dev-dependencies]` entry everywhere and declares `publish = false`; `cargo tree -p <crate> -e normal` finds it in none of the four release graphs.~~

### N2. Close the untested-clause gap, lowest risk first (N2-a … N2-e)

Twenty-seven clauses, ordered so that each step is independently useful and
the cheap structural ones land before the expensive rendering ones.

- **N2-a** `[x]` ~~The structural clauses that need no rendering — `5.4`, `6.1`, `7.1`, `7.2`, `19.1`, `20.1` and `A.1`. The version clause caught the model's silent default: a document with no `Version` attribute reports `1`, not an error.~~
- **N2-b** `[x]` ~~Drawing parameters: `8.2.1` draw parameter structure, `8.2.2` line join, `8.2.4` dash pattern, `8.2.5` line cap, `9.2` fill rule. Writing these against the annex A schemas turned up three real parser bugs, all now fixed: the cap and join attributes are named `Cap` and `Join`; `DashPattern` is an `ST_Array` attribute with a `DashOffset` sibling rather than a child element; and the `Rule` attribute on `PathObject` was never read, so every path was filled non-zero.~~
- **N2-c** `[x]` ~~Text and font: `11.1` font, `11.3` text positioning, `11.4` glyph transformation, `7.8` outlines. Four more real gaps surfaced here — `DeltaY` was not parsed, `DeltaX` was parsed but never applied, `HScale` was ignored, and `TextObject.Stroke`/`Fill` meant text could never be outlined. All four are now read and flattened.~~
- **N2-d** `[x]` ~~Graphics features: `8.3.4` gradient, `8.4` clip area, `9.1` path object. Gradients meant the parser had to stop skipping `FillColor` children, and clip areas are emitted as a `ClipPath` between a `Save`/`Restore` pair. `8.4` stays `partial`: only a `Path` area is read, and a `Text` area is rejected with an explicit error rather than silently dropped.~~
- **N2-e** `[x]` ~~The feature tail: `12` video, `13` composite object, `14.1` action description, `15` annotations, `16` custom tags, `17` extended information, `18.2.2` signature scope, `18.2.3` signature appearance. Reading the schemas first paid off three times — `18.2.2` was wrong in the fixture as well as the parser, `18.2.3` was wrong about `SignedValue` being a path rather than the proof inline, and `12`/`13` turned out to be resource clauses rather than page-object ones. All are now read, and the clause count closes at `47 of 47`.~~
- **N2-e extras** ~~`Action` is modelled as the schema declares it: `Event` is a required enumeration, so an unknown spelling is a parse error rather than a silent default, and the body is a choice of `Goto`, `URI`, `GotoA`, `Sound` or `Movie`, each read into its own shape. `MovieOperator` and `ActionEvent` keep the standard's spelling on one side and a Rust enum on the other, so callers never match on strings.~~

A clause moves to `verified` only when a linked test actually asserts it. The
gate script stays the single source of truth; `docs/COMPLIANCE.md` is
regenerated, never hand-edited.

### N3. Property and fuzz testing (N3-a … N3-d)

This is where `proptest` finally gets used, on the parsers that face hostile
input.

- **N3-a** `[x]` ~~Property tests for `package`, covering five invariants. The round-trip property found a real defect: `build_archive` used to write a name with a leading slash that its own `Package::open` then refused, so a document could be stored and never read back. The writer now validates names the way the reader does, and a regression test pins the five rejected shapes.~~
- **N3-b** `[x]` ~~Property tests for `xml` and `model`, asserting the same contract: reading may fail, it must never panic, and a value that parses survives a `Display` round trip. Writing the text property pinned down two behaviours that were previously only implicit — whitespace-only text is skipped, and outer whitespace is trimmed.~~
- **N3-c** `[x]` ~~`cargo-fuzz` targets for the ZIP and XML readers, in `fuzz/`: `package_open` walks every entry of any archive `Package::from_bytes` accepts, and `xml_cursor` turns a non-terminating walk into a visible failure rather than a stuck process. The crate lives outside the workspace because `cargo-fuzz` requires its own layout, the hand-written seed corpus is committed while libFuzzer's mutations are not, and CI runs both for 20,000 runs under nightly.~~
- **N3-d** `[x]` ~~A crash-free invariant test that feeds random bytes to every public entry point and asserts only that errors come back (`crates/steleofd-parser/tests/crash_free.rs`).~~

### N4. Integration tests for the six thin crates (N4-a … N4-f)

One task per crate, each driving the public API as a consumer rather than
reaching into internals. All six are done, 96 tests in total; each file is a new
`tests/*.rs` in its crate, so a struct whose fields exist but whose accessors
disagree now fails a test.

- **N4-a** `[x]` ~~`convert`: SVG shape and geometry, multi-page and empty PDFs, PNG encoding, and the pixel budget. 11 tests.~~
- **N4-b** `[x]` ~~`xml`: namespace prefixes, absent versus malformed attributes, the cursor walk, the depth ceiling and error positions. 21 tests.~~
- **N4-c** `[x]` ~~`model`: root validation, page lookup bounds, layer object counts and the out-of-page resource types. 18 tests.~~
- **N4-d** `[x]` ~~`font`: face loading, codepoint round trips, shaping, metrics, the fallback chain and the missing-font error. 18 tests.~~
- **N4-e** `[x]` ~~`display`: content bounds, blank detection, the state-stack ceiling, viewport sizing and culling. 17 tests.~~
- **N4-f** `[x]` ~~`render`: painted pixels for background, fill, clip and transform, plus the pixel budget and the degenerate page box. 11 tests.~~

### N5. Documentation and release hygiene (N5-a … N5-d)

- **N5-a** `[x]` ~~`README.zh-CN.md` is level with `README.md` again: the same roadmap and the same clause coverage.~~
- **N5-b** `[x]` ~~`CHANGELOG.md`, following Keep a Changelog and driven by the conventional-commit history.~~
- **N5-c** `[x]` ~~`cargo-semver-checks` in CI so a breaking public-API change fails the build rather than surprising a downstream user. The job compares against the latest `v*` tag; with no tag yet it reports that there is no published API to protect and skips, so it is honest now and active later.~~
- **N5-d** `[x]` ~~A release workflow triggered by a `v*` tag, building the `cdylib`, `staticlib` and wasm artefacts and attaching the generated C header and the wasm bindings to the GitHub release. The header check makes a stale committed header fail the release rather than ship.~~

### N6. Corpus and CI (N6-a … N6-b)

- **N6-a** `[x]` ~~`compliance/corpus/features/` joins `minimal/`: one eleven-entry document reaching the parts of annex A the minimal corpus never touched, which conforms to the normative schemas and opens, audits, renders, searches and converts through the CLI. Writing it found two real defects — `scripts/validate_ofd.py` routed a root of `Annotation` to a schema that declares `PageAnnot`, and `features/` pointed four entries at `Doc_0/`-prefixed paths that clause 5 makes relative to the document root — and forced two contract clarifications: `Signatures` identifiers are `xs:ID` so they cannot begin with a digit, and a `Signature`'s `BaseLoc` is a full archive path. A container may carry XML in its own vocabulary, so `validate_ofd.py` grew an explicit `--allow-foreign` opt-in. `scripts/pack_corpus.py --check` runs in CI so a source edit cannot leave the committed archive stale.~~

- **N6-b** `[x]` ~~The `coverage` job is in CI: `cargo-llvm-cov` over the whole workspace with every feature on, and the measured baseline is **87.56% lines, 88.16% functions, 88.37% regions** with the floor set at **85%** — low enough that ordinary work never trips it, high enough that losing a module's tests does. The measurement uses the rustup `llvm-tools-preview` component, whose `llvm-cov`/`llvm-profdata` match the toolchain's LLVM; the system copies are LLVM 18 against rustc's LLVM 21 and fail with a raw-profile version mismatch.~~

### N7. The desktop reader, first surface (U1-a … U1-c)

The reader is the one thing this project is obviously for, and it does not
exist. Everything below the interface is finished: the engine opens a
document, reports its facts, rasterizes any page to PNG or SVG, searches it
and verifies its signatures. What is missing is the layer that calls those in
the right order and puts a person in front of the result.

It is a **native desktop application**, not a web page. It links the engine
directly, so opening a document involves no upload, no server and no copy of
the document leaving the machine — which is the point, for the government and
finance offices this format is used in.

The interface strings already exist. `locales/*/reader.ftl` carries ten of
them in both languages, from the page counter to facing-pages mode, and no
code reads them yet. The work is to build the surface they were written for,
not to invent a new one.

**The framework is `egui`/`eframe`, and the reason is the build.** It is
pure Rust, so the reader is one more workspace member rather than a second
toolchain, and `cargo build --workspace` keeps working unchanged. Its cost is
that accessibility has to be wired up by hand in U3-c.

- [x] **U1-a** ~~Stand up `crates/steleofd-reader`: an `eframe` application that depends on `core` directly, with a window, a menu bar and a document area. Builds on a headless runner with no system package beyond the fontconfig headers CI already installs.~~
- [x] **U1-b** ~~Open a file through a native dialog and through drag and drop, show the document facts, and wire the page counter and navigation to `reader-page-of`, `reader-blank-page` and `reader-open`. The window's zones and per-frame flow are fixed in [UI-SPEC.md](UI-SPEC.md), the four-zone window is drawn from panels, and the canvas state is decided in `App`, which keeps it testable without a display.~~
- [x] **U1-c** ~~Prove the path in CI: `tests/window.rs` runs the real widget tree through `egui_kittest` with no display and no X server, clicking the actual toolbar buttons and reading labels out of the catalogue. It immediately earned its keep — it found a slider labelled with the raw `{ $percent }` template, and a page rasterized wider than the driver's texture limit at high zoom, which `egui` answers with a panic.~~

Exit criterion: a person can open an `.ofd` in a native window and read every
page.

### N8. Layout, zoom and search (U2-a … U2-c)

- **U2-a** `[x]` ~~`reader-layout-*` is live: `viewer::visible_pages` is a pure function from a layout, a current page and a page count to the pages on screen, paging steps by the layout's stride, and `steleofd-testkit` gained `multipage_archive` since a one-page fixture has no arrangement to assert.~~
- **U2-b** `[x]` ~~Rendering moved off the draw path into `render_worker`, a thread with its own `Document` and a channel pair, so a page that has not landed is `Canvas::Pending` rather than a stall. Zoom is real: `Fit::Width`/`Fit::Page` resolve through `viewer::fit_dpi` against the space the canvas actually has, and cancellation is by generation. Writing this exposed three liveness bugs only a real frame loop produces, all fixed and covered: a superseded request swallowed the queue behind it, a request was skipped when nothing was left to answer it, and restamping the same request every frame kept the worker rendering rasters the next frame had already replaced — so "ask once per frame" is only safe if an identical ask is recognised as identical, which is what `RenderQueue::request` now guarantees.~~
- **U2-c** `[x]` ~~The search panel is live as zone Z5, drawn before the canvas so the canvas measures the space actually left for pages; hits are reduced to page, snippet and rectangle, selecting a hit turns to its page, and the canvas outlines every hit with the current one stronger. `reader-no-match` is deliberately narrow — an empty query, or one made with no document open, is not a failed search.~~

### N9. Verification, export and access (U3-a … U3-c)

- **U3-a** `[x]` ~~A signature panel as zone Z6, drawn before the canvas so the canvas measures the space actually left for pages. It shows one row per signature with its verdict, signer, signing time and algorithm, and verification runs on the document once when the panel first opens rather than per signature, because asking the engine per signature re-reads the package each time. Three verdicts rather than two: the engine can only conclude good or bad and reports a signature it has no certificate for as an error, and flattening that into "tampered" would tell the reader their document was altered when nothing says it was — so an unverifiable signature is amber, not red.~~
- **U3-b** Export to PDF and PNG through the existing `convert` path, using
  the page range the reader is currently showing.
- **U3-c** Keyboard navigation and focus handling throughout, so the reader is
  fully operable without a mouse, plus a real locale switch that pulls from the
  existing catalogues instead of hardcoded strings. `egui` has thin
  accessibility support, so this task is wider here than it would be in a web
  reader and is not optional.

---

## Mid term: make it fast and complete

Exit criterion: the engine holds a performance budget it is measured against,
and every clause the project claims to implement is either verified or
explicitly documented as a deviation.

### M1. Performance baselines (M1-a … M1-c)

`criterion` is declared and unused; this is where it earns its place.

- **M1-a** Benchmarks for parsing, rasterizing, PDF conversion and search,
  with committed baselines so a regression is visible in the diff.
- **M1-b** A documented performance budget: pages per second for render, and
  the document size the parser handles within a stated latency.
- **M1-c** A CI job that runs the benchmarks and fails only on a large,
  deliberate regression.

### M2. Font pipeline hardening (M2-a … M2-c)

Fonts are where a document engine most often meets reality.

- **M2-a** Broaden the embedded and system font coverage, and test against a
  documented matrix of CJK, Latin and right-to-left faces.
- **M2-b** Subset embedded fonts instead of carrying whole faces.
- **M2-c** Make font fallback deterministic and observable, so the same input
  always produces the same glyphs.

### M3. Rendering completeness (M3-a … M3-d)

- **M3-a** Colour spaces beyond sRGB, and CMYK for print.
- **M3-b** Blending and transparency modes.
- **M3-c** The drawing features N2-b/N2-d verify in tests but only partially
  implement; bring each to parity as its clause is verified.
- **M3-d** A golden-image suite: render the corpus to PNG, compare against
  committed references, and gate on a pixel-difference threshold.

### M4. Signature depth (M4-a … M4-c)

- **M4-a** `18.2.2` signature scope, and `18.2.3` signature appearance, both
  verified and both implemented rather than merely tolerated.
- **M4-b** Seal and incremental update, so a signed document can be extended
  without breaking its signature.
- **M4-c** Timestamp countersignature verification.

### M5. Server production readiness (M5-a … M5-e)

The HTTP surface works; it is not yet something to put behind a load balancer.

- **M5-a** Replace the in-memory store with a persistent one, so a restart does
  not lose every open document.
- **M5-b** Authentication and per-tenant authorisation. Today any caller who
  knows a document id can read it.
- **M5-c** Rate limiting and a real request timeout budget.
- **M5-d** OpenAPI documentation generated from the route definitions.
- **M5-e** Graceful drain, so a rolling deploy does not cut a render in half.

### M6. Interoperability (M6-a … M6-b)

- **M6-a** Test against documents produced by other real OFD implementations,
  not only our own builder.
- **M6-b** Round-trip tests against the standard's own examples, if the
  working group publishes any.

### M7. The reader, completed

The first surface in N7 is deliberately minimal. This is where the reader
grows into the tool the standard's feature set actually implies.

- **M7-a** Annotation display and navigation, once clause 15 is verified
- **M7-b** Signature appearance rendering, once `18.2.3` is implemented
- **M7-c** A document comparison view for two revisions of the same file
- **M7-d** Print, straight from the reader's current view
- **M7-e** Signed, installable builds for the platforms people actually deploy
  on, and an offline story: a reader that works from a file share with no
  network, which is how these documents are often handled.

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
so an organisation can plug in its own without forking the engine.

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
