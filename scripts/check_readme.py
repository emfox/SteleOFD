#!/usr/bin/env python3
"""Check that the two READMEs agree on the numbers they state.

`README.md` is the source of truth and `README.zh-CN.md` follows it. The two
drifted twice, and every count in them is a copy of something the workspace
knows, so the copies are what drifts: a crate joins the workspace, one README
is updated, and the other keeps claiming the old figure.

This script reads every count out of the English README, reads the same
sentence out of the Chinese one, and compares the numbers the two state. It
does not translate: a change that moves a number has to be made in both files,
and that is the whole point.

A number is compared where the English sentence and its Chinese counterpart
are found by the same anchor. An anchor is a short phrase that introduces a
count, written once here, so a sentence that is reworded in English fails
loudly rather than quietly dropping out of the comparison.

Usage:
    python3 scripts/check_readme.py            # compare the two READMEs
    python3 scripts/check_readme.py --explain  # what each count is anchored on

Exit code: 0 when the two agree, 1 when a count differs or an anchor is gone.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ENGLISH = ROOT / "README.md"
CHINESE = ROOT / "README.zh-CN.md"

# Each anchor is (what the count is, English phrase, Chinese phrase). The two
# phrases are the same sentence in each file with the number replaced by a
# capture group, so the figures land in the same place in both. The Chinese
# anchors are written as escapes because section 1 of `docs/Conventions.md`
# keeps source files free of CJK, the text they match included.
ANCHORS = (
    (
        "crates in the workspace",
        r"(?P<n>\b(?:[A-Za-z]+(?:-[A-Za-z]+)*|\d+)) crates\s+in\s+(?P<layers>\w+)\s+layers",
        r"(?P<n>\d+) \u4e2a crate\uff0c(?P<layers>[\u4e00\u4e8c\u4e09\u56db\u4e94\u516d\u4e03\u516b\u4e5d\u5341]+)\u4e2a\u5c42\u6b21",
    ),
    (
        "crates that publish",
        r"(?P<n>\b(?:[A-Za-z]+(?:-[A-Za-z]+)*|\d+)) crates\s+that\s+publish",
        r"(?P<n>\d+) \u4e2a\u53d1\u5e03",
    ),
    (
        "crates that do not publish",
        r"(?P<n>\b(?:[A-Za-z]+(?:-[A-Za-z]+)*|\d+))\s+that\s+do\s+not",
        r"(?P<n>\d+) \u4e2a\u4e0d\u53d1\u5e03",
    ),
    (
        "clauses registered",
        r"(?P<n>\d+) clauses\s+are\s+registered",
        r"\u5df2\u767b\u8bb0 (?P<n>\d+) \u6761",
    ),
)

EXPLAIN = {
    "crates in the workspace": "the member list in `Cargo.toml`, said in the module map",
    "crates that publish": "`scripts/check_publish.py --list`, said in the checks section",
    "crates that do not publish": "the other half of that list, same sentence",
    "clauses registered": "`compliance/gb-t-33190.toml`, said under what it reads",
}

UNITS = {
    "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6,
    "seven": 7, "eight": 8, "nine": 9, "ten": 10, "eleven": 11, "twelve": 12,
    "thirteen": 13, "fourteen": 14, "fifteen": 15, "sixteen": 16,
    "seventeen": 17, "eighteen": 18, "nineteen": 19, "twenty": 20,
}

CHINESE_UNITS = {
    "\u4e00": 1, "\u4e8c": 2, "\u4e09": 3, "\u56db": 4, "\u4e94": 5,
    "\u516d": 6, "\u4e03": 7, "\u516b": 8, "\u4e5d": 9, "\u5341": 10,
}


def to_number(word: str) -> int | None:
    """Read a count written as digits, as an English word, or as a Chinese one.

    A README says `27` in one language and `twenty-seven` in the other, and the
    comparison is of the figures, not of how each language spells them.
    """
    word = word.strip().lower()
    if word.isdigit():
        return int(word)
    if word in CHINESE_UNITS:
        return CHINESE_UNITS[word]
    parts = word.replace("-", " ").split()
    total = 0
    for part in parts:
        if part == "and":
            continue
        # `twenty-seven` reads as twenty plus seven, and both words are already
        # in `UNITS`, so nothing here multiplies.
        if part not in UNITS:
            return None
        total += UNITS[part]
    return total or None


def read(path: Path) -> str:
    """One README's text, with its indented blocks kept out of the search."""
    if not path.exists():
        raise SystemExit(f"missing: {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


def find(text: str, pattern: str, label: str, where: str) -> dict[str, str]:
    """The named groups of the one place `pattern` matches, or a failure.

    An anchor that matches twice is ambiguous, and an anchor that matches
    nowhere means the sentence was reworded: both stop the check rather than
    letting a count go uncompared.
    """
    found = list(re.finditer(pattern, text))
    if not found:
        raise SystemExit(
            f"{where}: no sentence matches the anchor for {label!r}.\n"
            f"  pattern: {pattern}\n"
            f"  Rewording a counted sentence fails this check on purpose: update\n"
            f"  the anchor in scripts/check_readme.py in the same change."
        )
    if len(found) > 1:
        raise SystemExit(
            f"{where}: the anchor for {label!r} matches {len(found)} times, so\n"
            f"  which one states the count is ambiguous."
        )
    return found[0].groupdict()


def compare(explain: bool) -> int:
    if explain:
        for label, _, _ in ANCHORS:
            print(f"{label}: {EXPLAIN[label]}")
        return 0

    english = read(ENGLISH)
    chinese = read(CHINESE)
    failures = []

    for label, en_pattern, zh_pattern in ANCHORS:
        en = find(english, en_pattern, label, "README.md")
        zh = find(chinese, zh_pattern, label, "README.zh-CN.md")
        en_value = to_number(en["n"])
        zh_value = to_number(zh["n"])
        if en_value is None:
            failures.append(f"{label}: README.md states {en['n']!r}, which is not a count")
            continue
        if zh_value is None:
            failures.append(f"{label}: README.zh-CN.md states {zh['n']!r}, not a count")
            continue
        if en_value != zh_value:
            failures.append(
                f"{label}: README.md says {en_value}, README.zh-CN.md says {zh_value}"
            )
            continue
        print(f"OK  {label}: {en_value}")

    if failures:
        print()
        for line in failures:
            print(f"FAIL  {line}")
        print("\nREADME.md is the source of truth; README.zh-CN.md follows it.")
        return 1

    print(f"\nOK: {len(ANCHORS)} count(s) agree between the two READMEs")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--explain", action="store_true", help="what each count is anchored on")
    args = parser.parse_args()
    return compare(args.explain)


if __name__ == "__main__":
    sys.exit(main())
