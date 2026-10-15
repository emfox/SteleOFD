# Documentation and Language Policy

SteleOFD is an international open-source project. To keep the codebase consistent
and reviewable by contributors worldwide, all engineering artifacts use English.

## 1. Language rules

| Scope | Language | Notes |
| --- | --- | --- |
| Rust/C source: identifiers, comments, docs | English only | No CJK anywhere in `.rs`, `.toml`, `.c`, `.h` |
| Commit messages, branch names, PR titles and descriptions | English only | Conventional Commits format |
| Code review comments, issue titles and bodies | English only | |
| File names, directory names, config keys | English only | |
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

One line is the norm. A genuinely significant change may add a short body of
at most two lines explaining why. Anything longer belongs in `docs/`.

```text
<type>(<scope>): <subject>

<why, at most two lines, only for significant changes>
```

Allowed types: `feat`, `fix`, `docs`, `chore`, `refactor`, `perf`, `test`, `build`, `ci`.

`<scope>` is normally the crate name without the `steleofd-` prefix, for example
`parser`, `display`, `font`, `i18n`, `cli`.

The subject alone should be enough to understand the change. Do not list every
file touched, do not restate the diff, and do not write a changelog entry.

## 4. Enforcement

Five checks run in CI and are expected to pass before any merge:

```bash
python3 scripts/check_commits.py --grandfathered scripts/commit-baseline.txt
python3 scripts/check_i18n.py          # message key consistency across locales
python3 scripts/check_language.py      # English-only policy for code and docs
python3 scripts/check_compliance.py --check-only
python3 scripts/validate_ofd.py ...     # corpus against the annex A schemas
```

Commit messages are checked at three points, so a violation is caught before
it becomes history rather than during a review:

| Where | What it checks |
| --- | --- |
| `scripts/hooks/commit-msg` | the message being written, before the commit exists |
| `.github/workflows/rust.yml` | the commits a push or pull request proposes |
| `python3 scripts/check_commits.py REF` | the whole reachable history, on demand |

Install the local hook once per clone:

```bash
sh scripts/install-hooks.sh
```

The rules it enforces are the ones above: an allowed type, a subject of at
most 72 characters with no trailing period, a body of at most two non-blank
lines, and no CJK anywhere in the message.

`scripts/commit-baseline.txt` lists the five commits that already break these
rules. They predate the checker and are already published, so rewriting them
would break every existing clone. The file is append-only in practice: adding
a line to it is a deliberate decision to accept a violation, and it should
carry a reason.

### 4.1 Exemptions from the language policy

`check_language.py` scans Rust sources, `Cargo.toml` files, and Markdown or reST
documentation. It fails on CJK characters in any of them and on comment blocks
longer than three lines.

Three exemptions exist, each deliberately narrow:

| Exemption | Where | Why |
| --- | --- | --- |
| Whole file | `README.zh-CN.md`, `docs/CONVENTIONS.md`, the two checker scripts | These exist to hold or match Chinese |
| Whole file | anything under `locales/` | Locale catalogues are the translation data |
| String literals only | `steleofd-i18n/src/format.rs` | Holds `\u`-escaped locale date and time markers |
| Single line | the `README.md` language switcher | The bilingual pair must cross-link by the other language's own name |

New exemptions should be added to the allowlists in the script with a one-line
reason, so that every exception stays self-documenting.
