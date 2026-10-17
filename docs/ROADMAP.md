# What is left

SteleOFD builds, tests and releases on every platform `egui` runs on, and the
register holds no `partial` clause. What this file lists is the work that is
still open, and nothing else: the record of what the finished work took is in
[`CHANGELOG.md`](../CHANGELOG.md).

The previous roadmap carried four horizons of closed tasks. Keeping them made
the open items hard to find, so they were folded into the changelog and this
file took their place.

## Ranking

Open items are ordered by **what the project is for**, then by what unblocks
what. The README says OFD is the legal carrier for official documents, licences,
invoices and archives, so the order puts wrong output on a real document first,
then distribution and legal completeness, then the rest.

1. **Wrong output on a real document** — a reader that renders a legal document
   incorrectly is worse than one that is slow.
2. **Blocks the thing above it** — an item that unblocks several others moves up
   regardless of its own size.
3. **Enforcement** — a check that stops a regression outlives the feature it
   guards.
4. **Everything else** — smallest first.

## Open

| # | Item | Why it ranks here | Done when |
| --- | --- | --- | --- |
| O1 | **A corpus from more than one producer.** `compliance/corpus` carries our own documents plus the standard's fetched samples; the 28-document third-party set is fetched, not committed. | A construct our writer never emits is one we have only ever read through somebody else's file. This is the highest-value open item because it grows the number of real documents every other row is checked against. | The committed corpus holds documents from at least two producers, and CI validates all of them against the annex A schemas. |
| O2 | **Signed builds.** Every artefact ships unsigned, with `SHA256SUMS` as the only trust anchor. | A person installing an `.exe` or an `.apk` for a legal document gets a system warning instead of a signature. | The desktop and Android builds are signed and the release notes name the key rather than the checksum. |
| O3 | **A package for the platforms people deploy on.** The release page has loose executables, not a `.deb`, `.rpm`, `.dmg` or Windows installer. | Installs and upgrades are how an OA deployment adopts a tool. It sits below O1 and O2 because the executable already runs. | At least one Linux package and one macOS bundle are published per tag and install without a manual step. |
| O4 | **The last export parity gap.** SVG 1.1 has no mesh gradient, so a Gouraud mesh keeps its mean colour there. | One recorded limit, in one export format, and no SVG construct that removes it. | Either an SVG output that carries the mesh, or the limit documented as final in `docs/INTEROP.md`. |
| O5 | **The constructs `office` and `ocr` name.** The two `convert` features are declared placeholders; nothing consumes them. | Chapter-level conversion is absent rather than wrong, so it ranks below the corpus and distribution work. | DOCX or OOXML conversion reads a document without a fidelity loss the changelog has to hedge, or the features are deleted. |
| O6 | **A worked example per delivery form, compiled in CI.** | The crates are documented; what is missing is a start-to-finish example a third party can copy. | Five examples build in CI, one per delivery form, and each is linked from the README. |
| O7 | **A public extension surface** for colour profiles, font providers and render backends. | An organisation cannot plug in its own without forking. It follows the examples because an extension point without a worked example is a guess. | The three provider traits are public, documented, and exercised by the examples above. |
| O8 | **Other shells.** A Tauri shell and a Qt or GTK binding, each a separate crate over `core`. | Both are alternatives to the shipped `egui` reader, not gaps in it. | A second shell opens a document and draws a page through `core` alone. |
| O9 | **Track GB/T 33190 revisions** and keep the annex A schemas regenerable from the standard text. | Nothing is pending today; this is the standing task that keeps it that way. | A new revision of the standard regenerates the schemas with the existing scripts and the register stays consistent. |
| O10 | **Keep the two READMEs level.** They drifted twice, and every number in them is a copy. | Real, but it is maintenance rather than capability. | A check in CI compares the numbers the Chinese README states against the English one. |

A browser shell over the `wasm` bindings is not listed: the reader now builds
for `wasm32` and ships as a page in each release, which is what it would have
asked for.

## Standing constraints

- **`8.1.6` `PrintQuality` stays a `deviation`.** Annex A carries no such
  element, so there is nothing in a document to assert against. The reason
  travels in the register and the row is not open.
- **Blending is deliberately unsupported.** GB/T 33190-2016 defines no blend
  mode; `Alpha` is the whole of its transparency model. Recorded in
  `docs/INTEROP.md` §6.
- **Far-term work is not scheduled.** The rows above describe a direction, not
  dates.

## How to propose a change

Open an issue, state the problem, name the exit criterion. A task without a
verifiable exit criterion does not belong here.
