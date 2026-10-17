# Documentation and Language Policy

SteleOFD is an international open-source project. To keep the codebase consistent
and reviewable by contributors worldwide, all engineering artifacts use English.

## 1. Language rules

| Scope | Language | Notes |
| --- | --- | --- |
| Rust/Python/C source: identifiers, comments, docs | English only | No CJK anywhere in `.rs`, `.py`, `.toml`, `.c`, `.h`, `.js`, `.ts` |
| Commit messages, branch names, PR titles and descriptions | English only | Conventional Commits format |
| Code review comments, issue titles and bodies | English only | |
| File names, directory names, config keys | English only | Except fetched test samples, which keep the name their producer gave them ([4.1](#41-exemptions-from-the-language-policy)) |
| `README` | Bilingual | `README.md` (English, default) + `README.zh-CN.md`, cross-linked |
| Other project documents (`docs/`) | English preferred | Chinese allowed only when a document is explicitly user-facing, and must be named `*.zh-CN.md` |
| UI strings shown to end users | Any locale | Must live in `locales/<tag>/*.ftl`, never hardcoded |

### User-facing text is never hardcoded

Engine and business layers must not contain literal user-facing text. See
[docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) for the engine-layer-zero-text
rule and the error-code plus parameters pattern.

## 2. Comment density

Comments explain *why*, not *what*. If the code already says what it does, the
comment is noise.

| Position | Limit |
| --- | --- |
| Inline comment inside a function body | 1 line; 2 lines only when genuinely needed |
| Item doc comment (`///`, `//!`) on a struct, enum, field, variant, or function | 1 line; 2 lines maximum |
| Module-level header (`//!` at top of `lib.rs` or a large module) | 3 lines maximum |
| Trailing comment on one line | 1 line |

Hard rules:

- No comment block longer than 3 lines, anywhere.
- Do not restate the function signature in prose.
- Do not keep commented-out code; delete it, git remembers.
- Do not write changelog or history notes in comments; that belongs in commits.
- Prefer a clear name over a comment explaining a poor one.

Long-form design rationale belongs in `docs/`, not in source comments.

## 3. Commit messages

```
<type>(<scope>): <subject in English, imperative, no trailing period>
```

One line is the norm, and the subject is capped at **75 bytes** so it stays
one line in a terminal and in `git log`. A genuinely significant change may
add a short body of at most two lines explaining why. Anything longer belongs
in `docs/`.

```text
<type>(<scope>): <subject>

<why, at most two lines, only for significant changes>
```

Allowed types: `feat`, `fix`, `docs`, `chore`, `refactor`, `perf`, `test`, `build`, `ci`.

`<scope>` is normally the crate name without the `steleofd-` prefix, for example
`parser`, `display`, `font`, `i18n`, `cli`.

The subject alone should be enough to understand the change. Do not list every
file touched, do not restate the diff, and do not write a changelog entry.

### 3.1 One logical change per commit

A commit carries **one** logical change: a new behaviour, a new test, or a fix.
Not two, and not a fix tangled with the test that proves it is a fix.

Commit as the change is finished. Do not accumulate work in the tree and split
it afterwards: a split made from memory separates hunks that belong together
and joins hunks that do not, and neither failure is visible in the diff. The
history is written while the reason for each hunk is still known, or it is
written badly.

What counts as one change:

| Together, one commit | Together, still wrong |
| --- | --- |
| A behaviour and the test that fails without it | Two unrelated fixes |
| A register entry and the generated matrix, moved by the same test | A refactor and a behaviour change |
| A source change and its own doc correction | A source change and an unrelated doc typo |

A test that arrives before the behaviour it covers, because the behaviour is
not written yet, commits alone and says so. A behaviour whose test lives in a
later commit is a behaviour that shipped unproven.

Push when about eight commits have accumulated, or earlier when something needs
to be shared, reviewed or backed up. The count is a habit rather than a rule
this repository can check; what it protects is the ability to push a run of
commits that were each verified on their own.

## 4. Enforcement

Every check lives in `scripts/`, and `scripts/check_all.py` runs them all:

```bash
python3 scripts/check_all.py            # everything this machine can run
python3 scripts/check_all.py --quick    # no cargo, no fetched samples
python3 scripts/check_all.py --list     # what would run, and why
```

It runs what the machine can run and names the reason it skipped the rest, so a
missing toolchain is visible rather than silently green. `--require-all` turns a
skip into a failure; a CI runner has every tool, so nothing is skipped there.

CI runs the same checks as separate steps, so a failure lands on the run page
next to the check that produced it. Grouped by what they need:

| Group | Checks | Needs |
| --- | --- | --- |
| `policy` | `check_language.py`, `check_i18n.py --strict`, `check_commits.py` | `git` |
| `repo` | `release_checksums.py selftest`, `pack_corpus.py --check` | nothing |
| `schema` | `validate_ofd.py --allow-foreign` over `compliance/corpus` | `lxml` |
| `rust` | `cargo fmt`, `clippy`, `build` and `test`, the golden images, the headless reader window, `check_compliance.py`, and the matrix freshness diff | `cargo`, `git` |
| `corpus` | `fetch_corpus.py` and `fetch_gb_samples.py`, `--check` then `--parse` | the fetched samples, `cargo` |
| `perf` | `check_performance.py` against the budget in [PERFORMANCE.md](PERFORMANCE.md) | a `cargo bench` run |
| `review` | `check_commit_scope.py`, which names commits that read as more than one change | `git` |

Everything in `review` reports rather than blocks. A script can measure where
the lines landed; it cannot know why they landed there, and a move, a rename or
an i18n change that reaches its callers all look like a squash from the outside.
The check exists to put the candidates in front of a reader, so the run says
`note` and still passes.

Commit messages are checked at three points, so a violation is caught before
it becomes history rather than during a review:

| Where | What it checks |
| --- | --- |
| `scripts/hooks/commit-msg` | the message being written, before the commit exists |
| `.github/workflows/rust.yml` | the whole reachable history, on a push or a pull request that touches more than documentation |
| `python3 scripts/check_commits.py REF` | the same history, on demand and locally |

The hook is one line of shell and can be skipped with `--no-verify`; the CI step
is the one that cannot. That is why both exist.

Install the local hook once per clone:

```bash
sh scripts/install-hooks.sh
```

The rules it enforces are the ones above: an allowed type, a subject of at
most 75 bytes with no trailing period, a body of at most two non-blank lines,
and no CJK anywhere in the message.

`scripts/commit-baseline.txt` exists to exempt commits that already break
these rules and cannot be rewritten without breaking existing clones. **It is
currently empty**, and the whole history passes without it: the three
over-long messages were rewritten in place once the checker landed.

Adding a line to it is a deliberate decision to accept a violation, and it
should carry a reason. Prefer rewriting the commit — a published message is
the only thing in this repository that cannot be fixed later, and
`git push --force-with-lease` is a small price for a history that needs no
exemption list at all.

### 4.1 Exemptions from the language policy

`check_language.py` scans Rust and Python sources, `Cargo.toml` files, and
Markdown or reST documentation. It fails on CJK characters in any of them and on
comment blocks longer than three lines.

Seven exemptions exist, each deliberately narrow:

| Exemption | Where | Why |
| --- | --- | --- |
| Whole file | any `*.zh-CN.md` | Section 1 asks a Chinese document to declare the language in its name, so the name is the exemption and a new translation needs no list edit |
| Whole file | anything under `locales/` | Locale catalogues are the translation data |
| Whole file and name | fetched test samples under `tests/fixtures/` | A sample is named by the producer that wrote it, and renaming one severs the only link back to where it came from |
| Whole file | `docs/CONVENTIONS.md`, `scripts/check_language.py`, `scripts/check_i18n.py` | They exist to hold, quote or match Chinese |
| Whole file | `scripts/fetch_corpus.py`, `scripts/fetch_gb_samples.py` | Their pin lists spell out the producer's own file names, exactly like the samples they fetch |
| Whole file | `crates/steleofd-i18n/src/format.rs` | Holds the locale day-period literals, which are Chinese and Japanese |
| Single line | the `README.md` language switcher | The bilingual pair must cross-link by the other language's own name |

The `format.rs` exemption covers the whole file rather than its string literals
alone: the CJK in it is three literals and no prose, and a Rust lexer inside the
checker would cost more than that risk is worth.

The sample exemption covers names as well as contents, and it is why the corpus
is fetched rather than committed: none of it is source, none of it is built, and
a sample that cannot be traced upstream is a sample nobody can explain.

New exemptions should be added to the allowlists in the script with a one-line
reason, so that every exception stays self-documenting.
