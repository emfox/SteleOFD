# What is left

SteleOFD builds, tests and releases on every platform `egui` runs on, and the
register holds no `partial` clause. What this file lists is the work that is
still open, and nothing else: what a release offers is in
[`CHANGELOG.md`](../CHANGELOG.md).

The previous roadmap carried four horizons of closed tasks. Keeping them made
the open items hard to find, so they were deleted and this file took their
place.

## When

`0.1.1` is gone: it repaired what `0.1.0` said about itself and changed no
behaviour. The rows under a version are the ones a release has claimed, and
everything else is marked `unversioned`: it is agreed to be worth doing, but no
release has claimed it, and putting a number on it today would be a guess
dressed as a plan.

Publishing is the one item held back by a decision rather than by order: it
waits until the features and the API have settled, because a version already
on a registry cannot be replaced. `0.5` is where that is expected, not a
promise.

| Version | Theme |
| --- | --- |
| `0.3` | R2 the reader rebuilt on a published application-UI guideline, R6 what the budgets do not measure |
| `0.3+` | R3 the Android build that crashes on open |
| `0.5+` | R7 which crates are published |
| unversioned | R8 packaging, and the rest |

## Ranking

Open items are ordered by **what the project is for**, then by what unblocks
what. The README says OFD is the legal carrier for official documents, licences,
invoices and archives, so the order puts wrong output on a real document first,
then what a signature is asked to prove, then distribution, then the rest.

1. **Wrong output on a real document** — a reader that renders a legal document
   incorrectly is worse than one that is slow. A crash is the same failure in
   its worst form: it produces no output at all.
2. **A verification that says more than it checked** — a signature reported
   valid is the reason the format is used at all.
3. **Blocks the thing above it** — an item that unblocks several others moves up
   regardless of its own size.
4. **Enforcement** — a check that stops a regression outlives the feature it
   guards.
5. **Everything else** — smallest first.

The version column is not the ranking read downwards. R3 is the crash, which by
the first rule ranks at the top, and it sits last only because diagnosing it
needs a device or an emulator that no other row does; a `0.3+` promise would be
one the project cannot keep.

## Open

### 0.3

| # | Item | Target | Why it ranks here | Done when |
| --- | --- | --- | --- | --- |
| R2 | **The reader's interface.** It obeys `Ui-spec.md`, which was written for the window that exists. That spec was built on no published application-UI guideline, and the window has outgrown it: panels, the toolbar, the dialogs, and settings that do not exist yet — a theme, a type scale, high contrast — need a rulebook a new contributor can look up rather than one we invented. | `0.3` | It does not affect output, so it ranks below a wrong verdict; but it is the surface every other delivery form is judged by. | A chosen guideline is named in `Ui-spec.md` with the clauses the window follows, the reader is rebuilt against it with nothing the old spec required removed without a note, and a theme, a type scale and a contrast setting exist because the guideline asks for them. |
| R6 | **What the budgets do not measure.** `Performance.md` carries five timings for one page each, and no memory ceiling at all — while the documents this is for are scanned invoices of tens of pages, where a single A4 at 300 dpi is about 26 MB once decoded. Nothing measures a many-page document, and the render cache is deferred "until measured", which has not been scheduled. | `0.3` | It follows R2 because a memory ceiling defines what the cache may hold, and the cache is the window's business. | `Performance.md` states a peak-memory ceiling per page and per document, `check_performance.py` enforces it, and a benchmark over a twenty-page document exists beside the one-page ones. |

### 0.5+

| # | Item | Target | Why it ranks here | Done when |
| --- | --- | --- | --- | --- |
| R7 | **The crates are not on a registry yet.** The surface is decided: nineteen crates publish and eight do not, each stating its `publish` field and a `description`, and `check_publish.py` enforces both sets in CI. What remains is the act. `cargo package -p steleofd-core` still fails because the crates it depends on are not there, so publishing has to go leaf first through the dependency layers, and a version already on the registry cannot be replaced. The semver gate is now set for it: a break needs a minor version and the check compares against the parent commit, so the bump a break costs is decided when it merges rather than when it ships. | Publishing is how the engine reaches anyone not building an application around it. It is held back by a decision rather than by order: a published API is fixed per version, so it waits until the features and the surface have settled. | `cargo add steleofd-core` builds against crates.io at the version then shipped, the release notes name the nineteen crates, and a CI job fails when a published crate's metadata stops being complete. |

### 0.3+

| # | Item | Target | Why it ranks here | Done when |
| --- | --- | --- | --- | --- |
| R3 | **The Android build crashes on open.** The archive installs, then exits before a page is drawn. | `0.3+` | A crash outranks a slow page, so it sits above the packaging work; but it needs a device or an emulator to diagnose, which no other row does, so it is not promised to a version. | The arm64 archive from the release page opens `ofdrw/999.ofd` on a device and on an emulator, and the crash has a regression test that fails without the fix. |

### Unversioned

| # | Item | Why it ranks here | Done when |
| --- | --- | --- | --- |
| R8 | **Packaging, for every form that is deployed.** The release page has loose executables: no `.deb`, `.rpm`, `.dmg` or Windows installer, no container image for `steleofd-server`, and the 77 MB static C archive ships beside the shared one whether or not anyone asked. | Installs and upgrades are how an OA deployment adopts a tool, and the server is the form most likely to be deployed first. It sits below the correctness work because the executable already runs. | At least one Linux package, one macOS bundle, one Windows installer and one container image are published per tag, and each installs without a manual step. |
| R9 | **Samples from more than one producer.** `compliance/fixtures` carries our own documents plus the standard's fetched samples; the 28-document third-party set is fetched, not committed. | A construct our writer never emits is one we have only ever read through somebody else's file, so this grows what every other row is checked against — but the fetched set already covers the common producers. | The committed fixtures hold documents from at least two producers, and CI validates all of them against the annex A schemas. |
| R10 | **The C ABI's threading contract is unwritten.** The header states what a borrowed pointer survives and what a null handle does; it says nothing about whether two threads may call into one handle, which an OA back end will ask before it adopts the library. | A missing sentence blocks an adoption, which is more than most of the rows below cost — but it is a sentence in a header rather than work in the engine. | The header states the threading and re-entrancy contract, and a test exercises whatever the sentence promises. |
| R11 | **The last export parity gap.** SVG 1.1 has no mesh gradient, so a Gouraud mesh keeps its mean colour there. | One recorded limit, in one export format, and no SVG construct that removes it. | Either an SVG output that carries the mesh, or the limit documented as final in `docs/Interop.md`. |
| R12 | **The constructs `office` and `ocr` name.** The two `convert` features are declared placeholders; nothing consumes them. | Chapter-level conversion is absent rather than wrong, and it is a feature nobody has asked for yet. | DOCX or OOXML conversion reads a document without a fidelity loss the changelog has to hedge, or the features are deleted. |
| R13 | **A worked example per delivery form, compiled in CI.** | The crates are documented; what is missing is a start-to-finish example a third party can copy. It comes after R7, because an example is worth little if nobody can add the crate. | Five examples build in CI, one per delivery form, and each is linked from the README. |
| R21 | **The accessibility stack is one eframe upgrade behind.** `accesskit_unix` reaches `zbus`, whose own authors have deprecated `zbus_names` and `zvariant` into it; RustSec records the deprecation as unmaintained, and the fix is a newer `eframe`, not a change this project can make. `deny.toml` names the three advisories so the audit stays green in the meantime. | It fails the `deps` gate for a reason that is not this project's to fix, so it sits here rather than above — but an ignore list is a debt, not a settlement. | `cargo deny check advisories` passes with those three IDs removed from the ignore list. |
| R22 | **The font stack is unmaintained.** `ttf-parser` and `rustybuzz` have both been declared unmaintained by their authors; RustSec's suggested replacements are `skrifa` and `harfrust`. The parsing and shaping APIs the renderer is built on would have to be rewritten, and the font parser is reachable with bytes a document supplies, which is why `ttf_face` fuzzes it. | A security fix to either crate will not come, and the parser takes hostile input — but the exposure today is fuzzed and the replacement is a rewrite, not a bump. | Both crates are replaced, `ttf_face` runs against the replacement, and the three font advisories leave `deny.toml`. |
| R14 | **A public extension surface** for colour profiles, font providers and render backends. | An organisation cannot plug in its own without forking. It follows the examples because an extension point without a worked example is a guess. | The three provider traits are public, documented, and exercised by the examples above. |
| R15 | **Other shells.** A Tauri shell and a Qt or GTK binding, each a separate crate over `core`. | Both are alternatives to the shipped `egui` reader, not gaps in it. | A second shell opens a document and draws a page through `core` alone. |
| R16 | **Track GB/T 33190 revisions** and keep the annex A schemas regenerable from the standard text. | Nothing is pending today; this is the standing task that keeps it that way. | A new revision of the standard regenerates the schemas with the existing scripts and the register stays consistent. |
| R17 | **Keep the two READMEs level.** They drifted twice, and every number in them is a copy. | Real, but it is maintenance rather than capability. | A check in CI compares the numbers the Chinese README states against the English one. |

A browser shell over the `wasm` bindings is not listed: the reader now builds
for `wasm32` and ships as a page in each release, which is what it would have
asked for.

An in-memory open that takes ownership of a caller's buffer is not listed
either. The C and wasm callers already hand over bytes and get one copy back;
removing that copy saves memory on a 30 MB document, and it is worth doing when
a caller says so rather than in advance.

## Standing constraints

- **`8.1.6` `PrintQuality` stays a `deviation`.** Annex A carries no such
  element, so there is nothing in a document to assert against. The reason
  travels in the register and the row is not open.
- **Blending is deliberately unsupported.** GB/T 33190-2016 defines no blend
  mode; `Alpha` is the whole of its transparency model. Recorded in
  `docs/Interop.md` §6.
- **`0.1.0` and `0.1.1` changed no behaviour.** The first rewrote the release
  notes by capability and renamed the documents to `Conventions.md`; the second
  fixed where the release notes come from and a placeholder `repository`.
- **Far-term work is not scheduled.** The rows above describe a direction, not
  dates.

## How to propose a change

Open an issue, state the problem, name the exit criterion. A task without a
verifiable exit criterion does not belong here.
