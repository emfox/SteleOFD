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
| `0.5+` | R3 the Android build that crashes on open, and R7 which crates are published |
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
the first rule ranks at the top, and it sits below the publishing work only
because diagnosing it needs a device or an emulator that no other row does; a
version promise would be one the project cannot keep.

## Open

### 0.5+

| # | Item | Target | Why it ranks here | Done when |
| --- | --- | --- | --- | --- |
| R3 | **The Android build crashes on open.** The archive installs, then exits before a page is drawn. One cause is found and fixed: the Android `eframe` features asked for Wayland and X11, which no device has, so the loaded library quit. The build now proves that target compiles on every push, and the entry point reports on logcat. What remains is that no run on a device has confirmed it. | `0.5+` | A crash outranks a slow page, so it sits above the packaging work; but the last step needs a device or an emulator, which no other row does, so it is not promised to a version. | The arm64 archive from the release page opens `ofdrw/999.ofd` on a device and on an emulator, and the crash has a regression test that fails without the fix. |
| R7 | **The crates are not on a registry yet.** The surface is decided: nineteen crates publish and eight do not, each stating its `publish` field and a `description`, and `check_publish.py` enforces both sets in CI. What remains is the act. `cargo package -p steleofd-core` still fails because the crates it depends on are not there, so publishing has to go leaf first through the dependency layers, and a version already on the registry cannot be replaced. The semver gate is now set for it: a break needs a minor version and the check compares against the parent commit, so the bump a break costs is decided when it merges rather than when it ships. | Publishing is how the engine reaches anyone not building an application around it. It is held back by a decision rather than by order: a published API is fixed per version, so it waits until the features and the surface have settled. | `cargo add steleofd-core` builds against crates.io at the version then shipped, the release notes name the nineteen crates, and a CI job fails when a published crate's metadata stops being complete. |

### Unversioned

| # | Item | Why it ranks here | Done when |
| --- | --- | --- | --- |
| R8 | **Packaging, for every form that is deployed.** The release page has loose executables: no `.rpm`, `.dmg` or Windows installer, no container image for `steleofd-server`, and the 77 MB static C archive ships beside the shared one whether or not anyone asked. One `.deb` exists, built by `scripts/uos_package.py` for UOS Desktop V20 and its glibc 2.28 baseline rather than for Debian at large, and not yet published per tag. A package for one distribution does not answer this row. | Installs and upgrades are how an OA deployment adopts a tool, and the server is the form most likely to be deployed first. It sits below the correctness work because the executable already runs. | At least one Linux package not tied to a single distribution, one macOS bundle, one Windows installer and one container image are published per tag, and each installs without a manual step. |
| R9 | **Samples from more than one producer.** `compliance/fixtures` carries our own documents plus the standard's fetched samples; the 28-document third-party set is fetched, not committed. | A construct our writer never emits is one we have only ever read through somebody else's file, so this grows what every other row is checked against — but the fetched set already covers the common producers. | The committed fixtures hold documents from at least two producers, and CI validates all of them against the annex A schemas. |
| R12 | **Which constructs `office` and `ocr` would name, if implemented.** A DOCX or OOXML conversion in one direction, and OCR in the other, each needs its own model before it needs code. | Nobody has asked for either, and a capability nobody asked for is a guess at a requirement rather than a gap. | Either a reader that opens a DOCX or OOXML document with the fidelity the changelog can state without a hedge, or the entry is closed as unwanted. |
| R13 | **A worked example per delivery form, compiled in CI.** | The crates are documented; what is missing is a start-to-finish example a third party can copy. It comes after R7, because an example is worth little if nobody can add the crate. | Five examples build in CI, one per delivery form, and each is linked from the README. |
| R21 | **The accessibility stack is one eframe upgrade behind.** `accesskit_unix` reaches `zbus`, whose own authors have deprecated `zbus_names` and `zvariant` into it; RustSec records the deprecation as unmaintained, and the fix is a newer `eframe`, not a change this project can make. `deny.toml` names those three advisories, and `RUSTSEC-2026-0192` from `sctk-adwaita` above them, which R26 carries, so the audit stays green in the meantime. | It fails the `deps` gate for a reason that is not this project's to fix, so it sits here rather than above — but an ignore list is a debt, not a settlement. | `cargo deny check advisories` passes with those four IDs removed from the ignore list. |
| R25 | **No raster cache across windows and frames.** Every page drawn is rasterized again, and nothing evicts a bitmap once a distant page is left behind. The memory budget is stated per page and per document, so the ceiling is met; what is missing is the other direction — a window that is redrawn repeatedly pays for the ink each time. | The budgets in `docs/Performance.md` hold without it, so this waits for a measurement rather than for a reason. | A cache with a stated eviction rule, and a benchmark line that shows the redraw cost it removes. |
| R26 | **`ttf-parser` is still in the tree, through window decorations.** This project reads fonts through `skrifa` since R22, and `deny.toml` keeps `RUSTSEC-2026-0192` for a path it did not choose: `winit` 0.30 draws its own frame on Wayland through `sctk-adwaita` 0.10, which reaches `ab_glyph`, which parses with `owned_ttf_parser` and therefore `ttf-parser`. The advisory is unmaintained rather than a known flaw, and nothing here hands it a document: the faces it parses are the desktop's own window-title typeface. `steleofd-reader` selects it by turning on `winit`'s `wayland-csd-adwaita`. | It is a debt recorded where the fix lives rather than work this project can do alone: dropping the feature costs the window frame on a compositor that draws no decorations of its own, so the settlement is the `eframe` upgrade R21 already waits on. Kept separate from R21 because the two retire on their own versions even where one upgrade clears both. | `ttf-parser` is absent from `Cargo.lock`, and `RUSTSEC-2026-0192` is gone from `deny.toml` with nothing added in its place. |
| R14 | **A public extension surface** for colour profiles, font providers and render backends. | An organisation cannot plug in its own without forking. It follows the examples because an extension point without a worked example is a guess. | The three provider traits are public, documented, and exercised by the examples above. |
| R15 | **Other shells.** A Tauri shell and a Qt or GTK binding, each a separate crate over `core`. | Both are alternatives to the shipped `egui` reader, not gaps in it. | A second shell opens a document and draws a page through `core` alone. |
| R16 | **Track GB/T 33190 revisions** and keep the annex A schemas regenerable from the standard text. | Nothing is pending today; this is the standing task that keeps it that way. | A new revision of the standard regenerates the schemas with the existing scripts and the register stays consistent. |

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
- **`RUSTSEC-2026-0192` no longer reaches this project's own parsing.**
  `steleofd-font` reads fonts through `skrifa`, and `ttf_face` fuzzes that. The
  advisory stays in `deny.toml` for one reachable path that is not ours to
  choose: `sctk-adwaita` draws winit's own window decorations through
  `ab_glyph`. It is open as R26, which is where that path and its fix live.

## How to propose a change

Open an issue, state the problem, name the exit criterion. A task without a
verifiable exit criterion does not belong here.
