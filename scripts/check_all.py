#!/usr/bin/env python3
"""Run every check this repository expects, from one command.

CI runs these as separate steps on a Linux runner that already has cargo, lxml
and the fetched corpus. A working copy rarely does, so this script runs what the
machine can run, says why it skipped the rest, and never reports a skip as a
pass.

Usage:
    python3 scripts/check_all.py                # everything this machine can run
    python3 scripts/check_all.py --quick        # no cargo, no fetched samples
    python3 scripts/check_all.py --list         # what would run, and why
    python3 scripts/check_all.py --only policy --only i18n
    python3 scripts/check_all.py --skip rust
    python3 scripts/check_all.py --require-all  # a skip is a failure
    python3 scripts/check_all.py --verbose      # stream each check's output

Exit code: 0 when nothing failed, 1 when a check failed, and with --require-all
also when a check had to be skipped.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYTHON = sys.executable

# Cheap checks first, so a policy mistake is reported before a build is spent.
GROUPS = ("policy", "repo", "schema", "rust", "corpus", "perf")

# Groups that need a Rust toolchain, a network fetch or a benchmark run. --quick
# leaves them out; everything else can finish on a fresh clone.
SLOW_GROUPS = ("rust", "corpus", "perf")

CORPUS_FILES = "compliance/corpus"


@dataclass(frozen=True)
class Check:
    """One gate: the steps to run, and what the machine must have for them."""

    name: str
    group: str
    steps: tuple[tuple[str, ...], ...]
    needs: tuple[str, ...] = ()
    unset: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()


def py(*args: str) -> tuple[str, ...]:
    """A sibling script, run by the interpreter running this one."""
    return (PYTHON, *args)


def corpus_targets() -> list[str]:
    """Every corpus archive and XML source, in the order CI passes them."""
    root = ROOT / CORPUS_FILES
    found = [
        p for p in root.rglob("*")
        if p.is_file() and p.suffix.lower() in (".ofd", ".xml")
    ]
    return [str(p.relative_to(ROOT)) for p in sorted(found)]


CHECKS: tuple[Check, ...] = (
    # Section 4 of docs/CONVENTIONS.md: the policy gates.
    Check("language", "policy", (py("scripts/check_language.py"),)),
    Check("i18n", "policy", (py("scripts/check_i18n.py", "--strict"),)),
    Check(
        "commits",
        "policy",
        (py("scripts/check_commits.py", "--grandfathered",
            "scripts/commit-baseline.txt"),),
        needs=("git",),
    ),
    # Artefacts the repository carries: a manifest tool and a build product.
    Check("checksums", "repo", (py("scripts/release_checksums.py", "selftest"),)),
    Check("corpus-pack", "repo", (py("scripts/pack_corpus.py", "--check"),)),
    # Annex A, over the corpus that ships with the tree.
    Check(
        "annex-a",
        "schema",
        (py("scripts/validate_ofd.py", "--allow-foreign", *corpus_targets()),),
        needs=("lxml", "corpus-sources"),
    ),
    # The Rust gate, in the order CI runs it.
    Check("fmt", "rust", (("cargo", "fmt", "--all", "--check"),), needs=("cargo",)),
    Check(
        "clippy",
        "rust",
        (("cargo", "clippy", "--workspace", "--all-targets", "--all-features",
          "--", "-D", "warnings"),),
        needs=("cargo",),
        # Only this workspace is held to -D warnings; a dependency's lint is not
        # this build's to fail. CI passes an empty value for the same reason.
        env=(("RUSTFLAGS", ""),),
    ),
    Check(
        "build",
        "rust",
        (("cargo", "build", "--workspace", "--all-targets", "--all-features"),),
        needs=("cargo",),
    ),
    Check("test", "rust", (("cargo", "test", "--workspace"),), needs=("cargo",)),
    Check(
        "test-all-features",
        "rust",
        (("cargo", "test", "--workspace", "--all-features"),),
        needs=("cargo",),
    ),
    Check(
        "golden",
        "rust",
        (("cargo", "test", "-p", "steleofd-convert", "--test", "golden"),),
        needs=("cargo",),
    ),
    Check(
        "reader-window",
        "rust",
        (("cargo", "test", "-p", "steleofd-reader", "--test", "window"),),
        needs=("cargo",),
        unset=("DISPLAY", "WAYLAND_DISPLAY"),
    ),
    Check(
        "compliance",
        "rust",
        (py("scripts/check_compliance.py", "--check-only"),),
        needs=("cargo",),
    ),
    Check(
        "compliance-matrix",
        "rust",
        (
            py("scripts/check_compliance.py"),
            ("git", "diff", "--exit-code", "--", "docs/COMPLIANCE.md"),
        ),
        needs=("cargo", "git"),
    ),
    # The fetched corpora. Each one reports a missing sample, so it is only
    # meaningful once the download has run.
    Check(
        "corpus-check",
        "corpus",
        (py("scripts/fetch_corpus.py", "--check"),),
        needs=("ofdrw-samples",),
    ),
    Check(
        "corpus-parse",
        "corpus",
        (py("scripts/fetch_corpus.py", "--parse"),),
        needs=("cargo", "ofdrw-samples"),
    ),
    Check(
        "gb-check",
        "corpus",
        (py("scripts/fetch_gb_samples.py", "--check"),),
        needs=("gb-samples",),
    ),
    Check(
        "gb-parse",
        "corpus",
        (py("scripts/fetch_gb_samples.py", "--parse"),),
        needs=("cargo", "gb-samples"),
    ),
    # The budget in docs/PERFORMANCE.md, read from a benchmark run's output.
    Check(
        "performance",
        "perf",
        (py("scripts/check_performance.py"),),
        needs=("criterion",),
    ),
)

SAMPLE_DIRS = {
    "ofdrw-samples": ("tests/fixtures/ofdrw", "scripts/fetch_corpus.py"),
    "gb-samples": ("tests/fixtures/gb-t-33190", "scripts/fetch_gb_samples.py"),
}


def precondition(name: str) -> tuple[bool, str]:
    """Whether this machine can run the check, and why not when it cannot."""
    if name in ("cargo", "git"):
        if shutil.which(name) is None:
            return False, f"{name} is not on PATH"
        return True, ""
    if name == "lxml":
        if importlib.util.find_spec("lxml") is None:
            return False, "lxml is missing (python3 -m pip install lxml)"
        return True, ""
    if name == "criterion":
        if not (ROOT / "target" / "criterion").is_dir():
            return False, "no target/criterion (run `cargo bench` first)"
        return True, ""
    if name == "corpus-sources":
        if not (ROOT / CORPUS_FILES).is_dir():
            return False, f"{CORPUS_FILES} is missing"
        return True, ""
    directory, script = SAMPLE_DIRS[name]
    if not (ROOT / directory).is_dir():
        return False, f"{directory} is absent (run python3 {script})"
    return True, ""


def blocked(check: Check) -> str:
    """The first unmet precondition, or an empty string when nothing blocks."""
    for need in check.needs:
        ok, reason = precondition(need)
        if not ok:
            return reason
    return ""


def environment(check: Check) -> dict[str, str]:
    """The process environment, with this check's overrides applied."""
    env = dict(os.environ)
    for key in check.unset:
        env.pop(key, None)
    env.update(dict(check.env))
    return env


def run(check: Check, verbose: bool) -> tuple[bool, str, float]:
    """Run every step in order; return (passed, captured output, seconds)."""
    started = time.perf_counter()
    captured: list[str] = []
    passed = True
    for step in check.steps:
        if verbose:
            print(f"    $ {' '.join(step)}", flush=True)
        done = subprocess.run(
            step,
            cwd=ROOT,
            env=environment(check),
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=not verbose,
        )
        if not verbose:
            captured.append(done.stdout or "")
            captured.append(done.stderr or "")
        if done.returncode != 0:
            passed = False
            break
    return passed, "".join(captured), time.perf_counter() - started


def selected(checks: tuple[Check, ...], only: list[str], skip: list[str],
             quick: bool) -> list[Check]:
    """The checks the flags leave in, keeping the declared order."""
    chosen: list[Check] = []
    for check in checks:
        if quick and check.group in SLOW_GROUPS:
            continue
        if only and check.name not in only and check.group not in only:
            continue
        if check.name in skip or check.group in skip:
            continue
        chosen.append(check)
    return chosen


def describe(check: Check) -> str:
    """The command a check runs, for --list."""
    steps = [
        " ".join("python3" if arg == PYTHON else arg for arg in step)
        for step in check.steps
    ]
    return " && ".join(steps)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--quick", action="store_true",
                        help="skip the groups that need cargo, a fetch or a bench")
    parser.add_argument("--list", action="store_true",
                        help="print the plan instead of running it")
    parser.add_argument("--only", action="append", default=[], metavar="NAME",
                        help="a check or group to run; repeatable")
    parser.add_argument("--skip", action="append", default=[], metavar="NAME",
                        help="a check or group to leave out; repeatable")
    parser.add_argument("--require-all", action="store_true",
                        help="treat a skipped check as a failure")
    parser.add_argument("--verbose", action="store_true",
                        help="stream each check's output as it runs")
    args = parser.parse_args()

    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(errors="replace")

    chosen = selected(CHECKS, args.only, args.skip, args.quick)
    if not chosen:
        print("nothing selected; --list shows what is available")
        return 2

    plan: list[tuple[Check, str]] = [(c, blocked(c)) for c in chosen]

    if args.list:
        for group in GROUPS:
            entries = [(c, why) for c, why in plan if c.group == group]
            if not entries:
                continue
            print(f"{group}:")
            for check, why in entries:
                state = f"SKIP  {why}" if why else "run"
                print(f"  {check.name:<20} {state}")
                if not why:
                    print(f"  {'':<20} {describe(check)}")
        return 0

    failed: list[str] = []
    skipped: list[tuple[str, str]] = []
    passed: list[str] = []
    started = time.perf_counter()
    group = ""

    for check, why in plan:
        if check.group != group:
            group = check.group
            print(f"\n== {group} ==")
        label = f"{check.group}/{check.name}"
        if why:
            skipped.append((label, why))
            print(f"  {check.name:<20} skip    {why}")
            continue
        ok, output, elapsed = run(check, args.verbose)
        if ok:
            passed.append(label)
            print(f"  {check.name:<20} ok      {elapsed:5.1f}s")
            continue
        failed.append(label)
        print(f"  {check.name:<20} FAIL    {elapsed:5.1f}s")
        if not args.verbose and output.strip():
            for line in output.strip().splitlines()[-40:]:
                print(f"      {line}")

    total = time.perf_counter() - started
    print("\n== summary ==")
    print(f"  {len(passed)} passed, {len(failed)} failed, "
          f"{len(skipped)} skipped in {total:.1f}s")

    if skipped:
        print("\n  skipped, with the reason:")
        for label, why in skipped:
            print(f"    {label:<28} {why}")
    if failed:
        print("\n  failed:")
        for label in failed:
            print(f"    {label}")
        print("\nNothing here is advisory: a failed check blocks a merge.")
        return 1
    if args.require_all and skipped:
        print("\n--require-all: a skipped check counts as a failure.")
        return 1
    print("\nOK: every check that could run passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
