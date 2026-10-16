# Roadmap

SteleOFD has all 26 crates of the original plan built, tested and pushed. The
original P0-P9 sequence is finished; this document is what comes after it.

The plan is organised by horizon rather than by layer, because the remaining
work is no longer "build the next crate" but "make what exists trustworthy,
then make it fast, then make it complete". Each horizon states its exit
criterion, so it is always obvious whether it is done.

Within each horizon the open tasks are ordered by **what the project is for**,
then by what unblocks what. The README says OFD is the legal carrier for
official documents, licences, invoices and archives, so the ranking puts
legal validity and archival correctness first, rendering correctness second,
and performance and looks last. Finishing an item is not what earns it an
early slot; hurting a real document is what costs it one.

Ranking rule, applied to every open task:

1. **Wrong output on a real document** — a reader that renders a legal
   document incorrectly is worse than one that renders it slowly.
2. **Blocks the thing above it** — a task that unblocks several others moves
   up regardless of its own size.
3. **Enforcement** — a check that stops a regression is worth more than the
   feature it guards, because the feature will be rewritten and the check
   will not.
4. **Everything else** — order within this band is by size, smallest first.

Finished tasks are one line: struck through, no explanation. Open tasks carry
the note a person would need to start them.

## Where the project stands

| Measure | Value |
| --- | --- |
| Crates | 26 (L0 foundation through L5 delivery) |
| Source | 38,687 lines of Rust, measured with `find crates -name '*.rs'` |
| Tests | 927 passing, plus 33 inline unit-test modules |
| Delivery forms | `cli`, `wasm`, `ffi`, `server`, `reader`, all on the `core` facade |
| Standard clauses | 74 registered in GB/T 33190-2016: 68 `verified`, 2 `partial` (`8.4`, `13.1`), 4 `deviation` with a stated reason |
| Languages | `en-US` and `zh-CN`, 88 message keys, parity enforced in CI |
| CI | fmt, clippy, build, test, all-features test, i18n, language, conformance, XSD validation, corpus freshness, fuzz smoke run |

## Honest gaps

These are the things a new contributor would otherwise discover the hard way.
Each one is a real, verified finding, not a guess.

1. **Two clauses are `partial`, four are `deviation`.** `8.4` is `partial`
   because only a `Path` clip area is read and a `Text` area is rejected with
   an explicit error. `13.1` is `partial` because a `CompositeObject` is
   flattened to a stand-in image op with empty bytes — the vector resource's
   own block is never resolved, so a placed vector graphic draws nothing.
   The four `deviation` entries (`8.1.6`, `8.3.5`, `8.3.6`, `8.3.7`) each
   carry their reason in the register. The register in
   `compliance/gb-t-33190.toml` is the source of truth and lists the status of
   each by name.
2. **The documented format support outran the code, until now.** Five cargo
   features (`render`'s `backend-canvas`/`backend-svg`/`backend-pdf`, and
   `convert`'s `office`/`ocr`) were declared and referenced nowhere, while
   `README` advertised "HTML and Office" conversion. The three empty render
   backends are gone; `office` and `ocr` stay as named placeholders with a
   comment saying nothing consumes them. M5-f is where they would land.
3. **The OFD corpus is one file.** `compliance/corpus/minimal.ofd` is the only
   document the XSD validation and the test suites actually read. Every
   conformance test builds its own fixture with our own writer, so a construct
   our writer never emits is a construct we have never read. M0-c and M5-f.
4. **`README.zh-CN.md` is behind the English README.** It still shows P3 and P5
   as unfinished and names a next step that no longer exists.
5. **The reader exists but is barely a reader.** It opens, rasterizes,
   navigates, searches, verifies, exports and takes the keyboard. What is still
   missing is print, annotation display, document comparison and a packaged
   build. See M6.
6. **A placed `CompositeObject` draws nothing.** The reader for
   `CompositeGraphicUnits` exists and is tested, but nothing calls it at open,
   so a document that places a vector graphic gets a stand-in image op with
   empty bytes. Clause 13.1 is `partial` for this reason. See M0-a.
7. **The server is the weakest delivery form.** `cli`, `wasm`, `ffi` and
   `reader` are all usable; `server` has an in-memory store, no authentication
   and no drain. See M4.

---

## Near term: earn the word "done" — finished

Nothing here added capability. Everything here made the existing crates
trustworthy to someone who did not write them. Exit criterion was: a new
contributor can clone, test, and trust the project without asking a question.
The criterion is met; every task below is closed, and the section is kept as
the record of what it took.

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

This is where every open task lives. Order within it is by the ranking rule at
the top of this document: wrong output first, then what unblocks what, then
enforcement, then the rest. Exit criterion: the engine holds a performance
budget it is measured against, and every clause the project claims to
implement is either `verified` or `deviation` with a stated reason — no
`partial` left.

### M0. Correctness on real documents (M0-a … M0-c)

The top band. Each of these is a document that renders wrong today, not a
feature that is absent, which is why they precede everything else.

- **M0-a** `[x]` ~~Resolve a placed `CompositeObject` to its vector resource.
  Clause 13.1 is `verified`: the resource table is read at open and a placed
  unit draws its own content.~~
- **M0-b** `[x]` ~~Read a `Text` clip area, closing `8.4`. A text area emits
  `DrawOp::ClipText` and every backend resolves the glyph outlines.~~
- **M0-c** `[x]` ~~Tolerance tests over the corpus. The standard's own clause
  samples are fetched and parsed in CI, and the third-party corpus is already
  covered; GIF, undecodable images and a `./`-rooted `DocRoot` were the three
  defects it found.~~

### M1. Enforcement before features (M1-a … M1-c)

A guard is worth more than the feature it protects: the feature gets rewritten,
the guard does not.

- **M1-a** `[x]` ~~A golden-image suite: render to PNG, compare against
  committed references, gate on a pixel-difference threshold. It found the
  gradient defect the op-level tests could not see: a shape whose fill was a
  shading painted black unless its standalone gradient op was matched first.~~
- **M1-b** `[x]` ~~A documented performance budget: a stated ceiling per stage
  for the one-page case, set well above the measured baseline so only a large,
  deliberate regression crosses it.~~
- **M1-c** `[x]` ~~A CI job that runs the benchmarks and fails only on a large,
  deliberate regression: `scripts/check_performance.py` reads criterion's
  output and checks each budgeted stage against the ceiling in
  `docs/PERFORMANCE.md`.~~

### M2. Fonts, the other correctness surface (M2-a … M2-b)

Faces are resolved by declared id and bare CFF is read (done; the CJK/Latin/RTL
matrix is in `docs/FONTS.md`).

- **M2-a** `[x]` ~~Act on the fallback chain in the reader.~~ Every backend now
  resolves a run code through `FontManager::outline_for_code`, which substitutes
  from the chain when the run's own face lacks the character; a code already
  resolved to a glyph id is never substituted. When no wide face is registered
  the chain adds one from `default_cjk_family`, which is what a document naming
  a CJK face it does not ship needs — `helloworld.ofd` went from a blank page to
  its text. Two limits stand and are recorded in `docs/FONTS.md` gap 4: the
  registry is filled only from the ids the pages name, and a substitute is
  chained only where one can be resolved (not wasm, not `system-fonts` off).
- **M2-b** `[x]` ~~Subset embedded fonts instead of carrying whole faces.~~ A
  document that embeds a 20 MB CJK collection pays for it in every conversion.
  `steleofd-font::subset` rebuilds `glyf`, `loca`, `cmap`, `maxp`, `hmtx`,
  `hhea` and `head` down to the glyphs it is asked for and renumbers them; a
  face it cannot rewrite without losing it (CFF, or no `cmap`) is carried
  whole. `register_container_fonts` drives it from the display list, so a run
  carrying code points keeps its face intact rather than being guessed at.
  `Orbitron-Bold.ttf` measured 28 648 → 2 804 bytes, an order of magnitude.

### M3. Signature depth (M3-a … M3-c)

`18.2.2` and `18.2.3` are already `verified`; what remains is the depth a
legal carrier needs.

- **M3-a** Seal and incremental update, so a signed document can be extended
  without breaking its signature. This is the one that matters legally: a
  document that cannot be signed twice cannot carry a countersignature.
- **M3-b** Timestamp countersignature verification.
- **M3-c** The archival profile. `steleofd-archive` checks an OFD against the
  archival rules; it needs the profile document it checks against named, and
  the four `deviation` clauses above are the natural first inputs. GB/T
  42133-2022 now supplies the profile, and the four checks a reader can decide
  from the container are in (`ARCH-101` … `ARCH-104`, covered by
  `compliance/gb-t-42133.toml`). What remains is everything 42133 asks of an
  authoring tool: subsetting fonts, stripping optional structures, removing
  redundant clips and converting images, none of which a read-only checker can
  do.

### M4. Server production readiness (M4-a … M4-e)

Ranks here rather than lower because the `server` crate is one of the four
delivery forms the README advertises, and it is the one furthest from being
deployable.

- **M4-a** Replace the in-memory store with a persistent one.
- **M4-b** Authentication and per-tenant authorisation.
- **M4-c** Rate limiting and a real request timeout budget.
- **M4-d** Graceful drain, so a rolling deploy does not cut a render in half.
- **M4-e** OpenAPI documentation generated from the route definitions.

### M5. Rendering completeness (M5-a … M5-f)

Absent features, not wrong ones, so they sit below the correctness band.

- **M5-a** `[x]` ~~Paint the colour spaces clause 7.13 declares. The declaration
  side is done: the `ColorSpaces` table is read at open and
  `ParsedDocument::color_spaces` carries each space's type, depth, profile and
  palette. What is left is the painting side — no backend consumes the table
  yet, so an image naming a CMYK space is still decoded as sRGB.~~ The painting
  side is done for colour values: a `FillColor` or `StrokeColor` naming a space
  has its channels read in that space at flatten time, a four-channel list
  being CMYK and a single channel grey, and an `Index` picks the palette entry.
  A colour whose space is absent or unknown keeps its ordinary sRGB reading, so
  nothing that worked before changes. An image's own colour space rides in its
  encoded bytes and is converted by the decoder, so it needed no table lookup.
- **M5-b** `[x]` ~~The three shading and fill models still standing as
  deviations: `Pattern` (8.3.5), `GouraudShd` (8.3.6) and `LaGouraudShd`
  (8.3.7). Each needs a painter, not just a reader.~~ All three are done.
  The two Gouraud models resolve to one triangle mesh (a strip of triples, or a
  `VerticesPerRow` lattice) and the raster backend writes each triangle's
  interpolated colours per pixel before masking them to the filled path. A
  `Pattern`'s cell is a page block, so it is flattened into the paint and
  rasterized once, then tiled across the area with its `ReflectMethod`, again
  masked to the path. `8.1.6` PrintQuality stays a deviation for a different
  reason: the element is absent from the vendored Annex A schema entirely, so
  there is nothing to read.
- **M5-c** Blending and transparency modes. Deferred: the 2016 schema has no
  blend-mode element or attribute — only the per-object `Alpha` the engine
  already honours — so this needs a non-standard extension and a decision about
  it, not an implementation.
- **M5-d** Bring each test-verified drawing feature to full parity. `RadialShd`
  end centre and radius travel with the op and each backend reduces the cone to
  what it can paint. The Gouraud mesh and the `Pattern` cell now do too: a mesh
  reaches PDF as a `ShadingType 4` free-form triangle shading, one colour per
  vertex, so the file interpolates rather than showing a mean colour, and a
  `Pattern` reaches SVG as a native `<pattern>` spanning one step. Two limits
  remain and are recorded rather than hidden: SVG has no mesh gradient and no
  mirrored repeat, so a mesh keeps its mean colour and a reflecting pattern its
  cell's first fill there.
- **M5-e** Move the JBIG2 codec out of `display`. The IR crate carries a full
  decoder (`display/src/jbig2.rs`, via `hayro-jbig2`) that only `render` and
  `convert` call; the IR layer should hold data structures, not codecs. A
  small `steleofd-codec` crate (or an existing lower one) is the home.
  Deferred because it touches `display`'s public surface and three downstream
  import paths.
- **M5-f** Interoperability. Test against documents produced by other real OFD
  implementations (M0-c is the corpus half of this), round-trip the
  standard's own examples if the working group publishes any, and the Office
  conversion the `office` feature on `convert` and `core` names as a
  placeholder. OCR rides with it. The nine-page golden book was rendered by
  another vendor's reader and compared page by page. Five pages agree to
  anti-aliasing; the four that differ were re-checked against the standard's
  own text, because the vendor is one of the four bodies that drafted it and a
  difference is not evidence of a fault on their side. That audit holds: each
  of the four is a gap in the oracle, measured, not assumed.
  `AxialShd` and `RadialShd` come out as one flat black — its gradient page has
  a single non-white colour and its radial page two colours, where ours carry
  a ramp of hundreds. A page box wider than `CommonData.PageArea` is ignored:
  it renders the 297x100 mm page into a 1.67 ratio box, ours into 2.97. And on
  the clip page it draws the clip area as a bare outline rather than a mask,
  so the object it should confine is never filled — the outline sits exactly on
  the clip rectangle our own render confines the fill to, and tables 34 and 35
  make that fill the required result.
  The comparison paid for itself in the other direction too: it exposed three
  defects in our own fixtures and reader — `FillRule` for `Rule`,
  `Fill`/`Stroke` defaults taken from the wrong model, and path coordinates
  read as absolute where the standard makes them offsets from the boundary
  corner. A second reader was then cross-checked on the same four pages, so no
  conclusion rests on one comparison: `ofdrw` 2.4.0, whose `AWTMaker` fills a
  `Fill="true"` path and strokes a `Stroke="true"` one exactly as tables 34
  and 35 say, takes the page box from the page's own area before
  `CommonData.PageArea`, and — telling in the other direction — carries no
  shading code at all. On every one of the four points it stands with us.
  The cross-check also undid one of our own conclusions: `Clips` does read a
  `TransFlag`, but it is an OFD 1.2 extension, not in the 2016 XSD, and the
  spec default is `true` where real files, the official reader samples among
  them, write `false`. We had guessed the default backwards. The attribute is
  gone; the clip stays in object space unconditionally, which is what the 2016
  text says and what every sample does. The full four-way comparison, with the
  per-page evidence and the second reader's source citations, is in
  [INTEROP.md](INTEROP.md).

### M6. The reader, completed (M6-a … M6-f)

The reader works; these are the parts of a reader people expect that it does
not have. Ranked by how often a real reader needs them.

- **M6-a** `[x]` ~~Print, straight from the reader's current view. `steleofd-print`
  exists and nothing calls it from the reader, so this is wiring, not writing.~~
  `print_worker` mirrors `export_worker`; the menu prints the pages on screen.
- **M6-b** `[x]` ~~Annotation display and navigation. Clause 15 is verified, so the
  model is there and only the view is missing.~~
  The document names its index, the parser joins marks back to pages, and the
  reader lists them by page and turns to the one picked.
- **M6-c** `[x]` ~~Signature appearance rendering — the visual half of the `18.2.3`
  model that is already read.~~
  `read_seals` resolves the seal image beside the descriptor and joins each
  `StampAnnot` to it; `Document::seals` maps the `PageRef` id to a page index,
  and the reader stamps each seal onto its page's raster. An image a package
  does not ship keeps its placement with empty bytes rather than vanishing.
- **M6-d** Signed, installable builds for the platforms people actually deploy
  on, and an offline story.
- **M6-e** A document comparison view for two revisions of the same file.
- **M6-f** Move the reader's interface strings out of `steleofd-i18n`. The
  engine crate defines product-facing ids (`ReaderFitWidth`,
  `ReaderSearchTitle`, …); they belong with the reader, so the engine catalogue
  holds engine messages only. Deferred because it moves `i18n`'s public API,
  the CI parity script and every reader call site at once.

### M7. Repository hygiene

- **[x] M7-a** ~~Split the aging files. `parser/src/content.rs` (1,560),
  `convert/src/pdf.rs` (1,442), `render/src/raster.rs` (1,306) and
  `reader/src/app.rs` (1,055) have outgrown one screen of scrolling. Pure
  reorganisation, no behaviour change; worth one deliberate pass rather than a
  drip.~~ Done: each file became a module root with its parts beside it, and
  the largest is now under 700 lines. Every function body was checked to be
  byte-identical, and the reader's `#[cfg(test)]` block stayed in the root so
  test names did not move.

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

The near term is finished; everything open is mid term. Within it the bands
run in order:

**M0 first.** These are documents that render wrong today, and the two
`partial` clauses are what stands between the conformance number and the word
"done". M0-a and M0-b are independent of each other and of everything else.

**M1 second, but start it in parallel with M0.** A golden-image suite is worth
more the earlier it exists, because it is what stops M0-a and M0-b from
regressing the moment they land. M1-b and M1-c depend on M1-a for something to
compare against, and M1-b needs M1-a's output to define a pixel budget.

**M2 third.** M2-a needs nothing from M0 or M1, so it can run alongside them.
M2-b depends on M2-a in the sense that subsetting a face the chain cannot use
buys nothing.
**M3 fourth.** M3-a is independent. M3-c is the reason M0 matters: an archival
check over a document that renders wrong is checking the wrong bytes.

**M4 through M7 last.** M4 is independent of everything above it and could
start any time the maintainers want a deployable server. M5 depends on M1-a,
because every item in it is a rendering change and none of them can be
verified without golden images. M6-a, M6-b, M6-c, M7-a, M2-b and M5-b are done,
so what is left is the rest of M5's rendering work, the reader items that
need a decision (M6-d, M6-e), and the deeper signature items in M3.

Far term is deliberately not scheduled. It describes a direction, not dates.

## How to propose changes

Roadmap changes follow the same rule as code: open an issue, state the
problem, name the exit criterion. A task without a verifiable exit criterion
does not belong here.
