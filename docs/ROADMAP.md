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
| Source | 24,904 lines of Rust, measured with `find crates -name '*.rs'` |
| Tests | 576 passing, plus 13 inline unit-test modules |
| Delivery forms | `cli`, `wasm`, `ffi`, `server`, `reader`, all on the `core` facade |
| Standard clauses verified | 20 of 47 in GB/T 33190-2016 |
| Languages | `en-US` and `zh-CN`, 75 message keys, parity enforced in CI |
| CI | fmt, clippy, build, test, all-features test, i18n, language, conformance, XSD validation |

## Honest gaps

These are the things a new contributor would otherwise discover the hard way.
Each one is a real, verified finding, not a guess.

1. **27 of 47 standard clauses have no test asserting them.** The register in
   `compliance/gb-t-33190.toml` lists them by name; they are not yet covered.
   The untested set is not random: it clusters on drawing details (line joins,
   dash patterns, caps, gradients, clipping), text detail (positioning, glyph
   transformation, font selection), and the whole feature tail (video,
   composite objects, actions, annotations, custom tags, extensions).
2. **`proptest` and `criterion` are declared in the workspace but never used.**
   Zero references outside the root `Cargo.toml`. The property and benchmark
   testing that was planned does not exist yet.
3. **The OFD corpus is one file.** `compliance/corpus/minimal.ofd` is the only
   document the XSD validation and the test suites actually read.
4. **The signed-fixture builder is duplicated four times** across the `sign`,
   `wasm`, `ffi` and `server` test suites, each with its own hand-rolled base64,
   DER and SM2 certificate construction.
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

- **N1-a** Stand up `crates/steleofd-testkit` with the OFD zip builder, the
  base64/DER/SM2-certificate helpers, and the signed and unsigned fixtures.
- **N1-b** Move `sign`, `wasm`, `ffi` and `server` onto it, deleting the
  copies. The four suites must keep passing unchanged in count.
- **N1-c** Publish it as a dev-dependency only; it must never appear in a
  release build.

### N2. Close the untested-clause gap, lowest risk first (N2-a … N2-e)

Twenty-seven clauses, ordered so that each step is independently useful and
the cheap structural ones land before the expensive rendering ones.

- **N2-a** The structural clauses that need no rendering: `5.4` file
  extension, `6.1` container scheme, `7.1` namespace, `7.2` UTF-8 encoding,
  `19.1` version entry, `20.1` attachment list, `A.1` the OFD.xsd itself.
- **N2-b** Drawing parameters: `8.2.1` draw parameter structure, `8.2.2` line
  join, `8.2.4` dash pattern, `8.2.5` line cap, `9.2` fill rule.
- **N2-c** Text and font: `11.1` font, `11.3` text positioning, `11.4` glyph
  transformation, `7.8` outlines.
- **N2-d** Graphics features: `8.3.4` gradient, `8.4` clip area, `9.1` path
  object.
- **N2-e** The feature tail: `12` video, `13` composite object, `14.1` action
  description, `15` annotations, `16` custom tags, `17` extended information,
  `18.2.2` signature scope, `18.2.3` signature appearance.

A clause moves to `verified` only when a linked test actually asserts it. The
gate script stays the single source of truth; `docs/COMPLIANCE.md` is
regenerated, never hand-edited.

### N3. Property and fuzz testing (N3-a … N3-d)

This is where `proptest` finally gets used, on the parsers that face hostile
input.

- **N3-a** Property tests for `package`: any ZIP that `open` accepts must
  round-trip; a truncated one must error, never panic.
- **N3-b** Property tests for `xml` and `model`: no input may panic; every
  `Display` output must re-parse.
- **N3-c** `cargo-fuzz` targets for the ZIP and XML readers, with the CI job
  wired to a short deterministic run.
- **N3-d** A crash-free invariant test that feeds random bytes to every public
  entry point and asserts only that errors come back.

### N4. Integration tests for the six thin crates (N4-a … N4-f)

One task per crate, each driving the public API as a consumer rather than
reaching into internals.

- **N4-a** `convert`: PDF/PNG/SVG output shape, page selection, error paths
- **N4-b** `xml`: encode/decode round trip against the annex A schemas
- **N4-c** `model`: document assembly invariants
- **N4-d** `font`: lookup, fallback, and the missing-font error
- **N4-e** `display`: list construction and measurement
- **N4-f** `render`: raster output properties, budget enforcement, dpi clamp

### N5. Documentation and release hygiene (N5-a … N5-d)

- **N5-a** Bring `README.zh-CN.md` level with `README.md`: roadmap, crate
  list, coverage numbers, next steps.
- **N5-b** Add a `CHANGELOG.md` driven by conventional commits.
- **N5-c** Add `cargo-semver-checks` to CI so a breaking public-API change
  fails the build rather than surprising a downstream user.
- **N5-d** Add a publish workflow for the `cdylib`, `staticlib` and `wasm`
  artefacts, with the C header and wasm bindings attached.

### N6. Corpus and CI (N6-a … N6-b)

- **N6-a** Grow `compliance/corpus` beyond `minimal.ofd` with documents that
  exercise the features N2 covers, so XSD validation has real input.
- **N6-b** Add a coverage report (`cargo-llvm-cov`) to CI as a non-blocking
  signal, then make it blocking once the number is good enough to defend.

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

- [x] **U1-a** Stand up `crates/steleofd-reader`: an `eframe` application
  that depends on `core` directly, with a window, a menu bar and a document
  area. Builds on a headless runner with no system package beyond the
  fontconfig headers CI already installs.
- [x] **U1-b** Open a file through a native file dialog and through drag and drop
  onto the window, show the document facts, and wire the page counter and
  navigation to `reader-page-of`, `reader-blank-page` and `reader-open`. The
  window's zones and per-frame flow are fixed in [UI-SPEC.md](UI-SPEC.md), so
  the layout is a rule rather than a convention; the four-zone window is drawn
  from panels and the canvas state is decided in `App`, which keeps it
  testable without a display.
- [x] **U1-c** Prove the path in CI: build the reader headlessly and drive one
  open-and-navigate flow, so the interface cannot silently rot while the
  engine keeps moving. `tests/window.rs` runs the real widget tree through
  `egui_kittest` with no display and no X server, clicking the actual toolbar
  buttons and reading labels out of the catalogue rather than out of the test.
  It immediately earned its keep: it found a slider labelled with the raw
  `{ $percent }` template, and a page rasterized wider than the driver's
  texture limit at high zoom, which `egui` answers with a panic.

Exit criterion: a person can open an `.ofd` in a native window and read every
page.

### N8. Layout, zoom and search (U2-a … U2-c)

- **U2-a** Layout modes wired to the strings that already exist: single page,
  facing pages, continuous. This is where `reader-layout-*` stops being dead
  text.
- **U2-b** Zoom, fit width and fit page, with the zoom percentage coming from
  `reader-zoom`. Render on demand per page rather than eagerly, and cancel
  in-flight renders when the reader moves faster than the rasterizer.
- **U2-c** A search panel over the existing `search` capability, listing hits
  by page, jumping to a hit, and showing `reader-no-match` when there is
  none.

### N9. Verification, export and access (U3-a … U3-c)

- **U3-a** A signature panel that shows what `verify` returns, per signature,
  rather than a bare valid-or-not.
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
