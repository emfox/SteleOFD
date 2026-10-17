# What is left

SteleOFD builds, tests and releases on every platform `egui` runs on, and the
register holds no `partial` clause. What this file lists is the work that is
still open, and nothing else: what a release offers is in
[`CHANGELOG.md`](../CHANGELOG.md).

The previous roadmap carried four horizons of closed tasks. Keeping them made
the open items hard to find, so they were deleted and this file took their
place.

## When

Three items have a version. Everything else is marked `unversioned`: it is
agreed to be worth doing, but no release has claimed it, and putting a number
on it today would be a guess dressed as a plan.

| Version | Theme |
| --- | --- |
| `0.2` | R1 — the CJK document that renders but lays out wrong |
| `0.3` | R2 — the reader rebuilt on a modern application-UI guideline |
| `0.3+` | R3 — the Android build crashes on open |
| unversioned | R4 and the rows below it |

## Ranking

Open items are ordered by **what the project is for**, then by what unblocks
what. The README says OFD is the legal carrier for official documents, licences,
invoices and archives, so the order puts wrong output on a real document first,
then distribution and legal completeness, then the rest.

1. **Wrong output on a real document** — a reader that renders a legal document
   incorrectly is worse than one that is slow. A crash is the same failure in
   its worst form: it produces no output at all.
2. **Blocks the thing above it** — an item that unblocks several others moves up
   regardless of its own size.
3. **Enforcement** — a check that stops a regression outlives the feature it
   guards.
4. **Everything else** — smallest first.

The version column is not the ranking read downwards. R1 comes first because
the output is wrong. R3 is the crash, which by the first rule would rank at the
top, and it sits third only because diagnosing it needs a device or an emulator
that no other row does; a `0.3+` promise would be one the project cannot keep.
R2 sits between them because the window is the surface every other delivery
form is judged by.

## Open

| # | Item | Target | Why it ranks here | Done when |
| --- | --- | --- | --- | --- |
| R1 | **CJK text lays out wrong.** `ofdrw/999.ofd` — a real VAT invoice from a Huhuang SDK producer, pinned in `emfox/OFD-fixtures` and fetched by `scripts/fetch_ofdrw.py` — now shows its Chinese text, because the font fallback finds a face that covers the code points, but the lines, spacing and positions are not what the document describes. | `0.2` | It is wrong output on a real document, which the ranking below puts first: a legal document that reads but sits crooked is still a document nobody can print. | The sample's lines, glyph advances and baselines match the document's own text objects on a golden image, and the fallback path is covered by that image rather than by a count. |
| R2 | **The reader's interface.** It obeys `Ui-spec.md`, which was written for the window that exists. That spec was not built on any published application-UI guideline, and the window has outgrown it: panels, the toolbar and the dialogs need a rulebook a new contributor can look up rather than one we invented. | `0.3` | It does not affect output, so it ranks below R1; but it is the surface every other delivery form is judged by, so it goes above the rest. | A chosen guideline is named in `Ui-spec.md` with the clauses the window follows, and the reader is rebuilt against it with no behaviour the old spec required removed without a note. |
| R3 | **The Android build crashes on open.** The archive installs, then exits before a page is drawn. | `0.3+` | A crash outranks a slow page, so it sits above the release and packaging work; but it needs a device or an emulator to diagnose, which no other row does, so it is not promised to a version. | The arm64 archive from the release page opens `ofdrw/999.ofd` on a device and on an emulator, and the crash has a regression test that fails without the fix. |
| O1 | **A corpus from more than one producer.** `compliance/corpus` carries our own documents plus the standard's fetched samples; the 28-document third-party set is fetched, not committed. | A construct our writer never emits is one we have only ever read through somebody else's file. This is the highest-value open item because it grows the number of real documents every other row is checked against. | The committed corpus holds documents from at least two producers, and CI validates all of them against the annex A schemas. |
| O2 | **Signed builds.** Every artefact ships unsigned, with `SHA256SUMS` as the only trust anchor. | `unversioned` | A person installing an `.exe` or an `.apk` for a legal document gets a system warning instead of a signature. | The desktop and Android builds are signed and the release notes name the key rather than the checksum. |
| O3 | **A package for the platforms people deploy on.** The release page has loose executables, not a `.deb`, `.rpm`, `.dmg` or Windows installer. | `unversioned` | Installs and upgrades are how an OA deployment adopts a tool. It sits below O1 and O2 because the executable already runs. | At least one Linux package and one macOS bundle are published per tag and install without a manual step. |
| O4 | **The last export parity gap.** SVG 1.1 has no mesh gradient, so a Gouraud mesh keeps its mean colour there. | `unversioned` | One recorded limit, in one export format, and no SVG construct that removes it. | Either an SVG output that carries the mesh, or the limit documented as final in `docs/Interop.md`. |
| O5 | **The constructs `office` and `ocr` name.** The two `convert` features are declared placeholders; nothing consumes them. | `unversioned` | Chapter-level conversion is absent rather than wrong, so it ranks below the corpus and distribution work. | DOCX or OOXML conversion reads a document without a fidelity loss the changelog has to hedge, or the features are deleted. |
| O6 | **A worked example per delivery form, compiled in CI.** | `unversioned` | The crates are documented; what is missing is a start-to-finish example a third party can copy. | Five examples build in CI, one per delivery form, and each is linked from the README. |
| O7 | **A public extension surface** for colour profiles, font providers and render backends. | `unversioned` | An organisation cannot plug in its own without forking. It follows the examples because an extension point without a worked example is a guess. | The three provider traits are public, documented, and exercised by the examples above. |
| O8 | **Other shells.** A Tauri shell and a Qt or GTK binding, each a separate crate over `core`. | `unversioned` | Both are alternatives to the shipped `egui` reader, not gaps in it. | A second shell opens a document and draws a page through `core` alone. |
| O9 | **Track GB/T 33190 revisions** and keep the annex A schemas regenerable from the standard text. | `unversioned` | Nothing is pending today; this is the standing task that keeps it that way. | A new revision of the standard regenerates the schemas with the existing scripts and the register stays consistent. |
| O10 | **Keep the two READMEs level.** They drifted twice, and every number in them is a copy. | `unversioned` | Real, but it is maintenance rather than capability. | A check in CI compares the numbers the Chinese README states against the English one. |

A browser shell over the `wasm` bindings is not listed: the reader now builds
for `wasm32` and ships as a page in each release, which is what it would have
asked for.

## Standing constraints

- **`8.1.6` `PrintQuality` stays a `deviation`.** Annex A carries no such
  element, so there is nothing in a document to assert against. The reason
  travels in the register and the row is not open.
- **Blending is deliberately unsupported.** GB/T 33190-2016 defines no blend
  mode; `Alpha` is the whole of its transparency model. Recorded in
  `docs/Interop.md` §6.
- **Neither `0.1.0` item is code.** The release notes are rewritten by
  capability rather than by commit type, and the documents are renamed to
  `Conventions.md` from `CONVENTIONS.md`. The first release that changes
  behaviour is `0.2`, through R1.
- **Far-term work is not scheduled.** The rows above describe a direction, not
  dates.

## How to propose a change

Open an issue, state the problem, name the exit criterion. A task without a
verifiable exit criterion does not belong here.
