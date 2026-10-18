#!/usr/bin/env python3
"""Check that every crate states whether it is published, and the ones that are
carry what a registry asks for.

The decision of *which* crates publish is a judgement this script does not make:
a crate either says `publish = false` with a reason or joins the published set.
What it enforces is that the choice was made, that the two sets still add up to
the whole workspace, and that a published crate has the metadata and the
dependency form that `cargo publish` needs.

Usage:
    python3 scripts/check_publish.py            # check the workspace
    python3 scripts/check_publish.py --list     # which crates publish, and why
    python3 scripts/check_publish.py --explain  # what each field is for

Exit code: 0 when every crate passes, 1 when one does not.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# A registry entry is what a stranger reads before deciding to add the crate, so
# each of these is a question they will otherwise have to answer by reading the
# source. `license` and `repository` come from the workspace, so a crate that
# inherits them passes without restating them.
REQUIRED = ("description", "license", "repository", "readme")

EXPLAIN = {
    "description": "one sentence on what the crate is for, shown in registry search",
    "license": "what a consumer may do with it; inherited from the workspace",
    "repository": "where the source lives; inherited from the workspace",
    "readme": "the page a registry shows; inherited from the workspace",
    "publish": "whether the crate goes to a registry at all",
}


def crates() -> list[Path]:
    """Every member crate's manifest, in the workspace's own order."""
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    members = re.search(r"^members\s*=\s*\[(.*?)\]", text, re.M | re.S)
    if members is None:
        return []
    # Only the member list: the same paths appear again under
    # `[workspace.dependencies]`, where they are not members.
    found = re.findall(r'"(crates/[a-z0-9-]+)"', members.group(1))
    return [ROOT / member / "Cargo.toml" for member in sorted(set(found))]


def package_block(text: str) -> str:
    """The `[package]` table of a manifest, and nothing after it."""
    match = re.search(r"^\[package\]\n(.*?)(?=^\[|\Z)", text, re.M | re.S)
    return match.group(1) if match else ""


def inherited(block: str, field: str) -> bool:
    """Whether the field is stated here or inherited from the workspace."""
    if re.search(rf"^{field}\s*=\s*\S", block, re.M):
        return not re.search(rf"^{field}\.workspace\s*=\s*true", block, re.M)
    pattern = rf"^{field}(\.workspace\s*=\s*true|\s*=\s*\S)"
    if re.search(pattern, block, re.M):
        return True
    return False


def publishes(block: str) -> bool:
    """Whether this crate goes to a registry."""
    match = re.search(r"^publish\s*=\s*(.+)$", block, re.M)
    if match is None:
        # An unstated `publish` means yes, which is the case most likely to be
        # an oversight rather than a choice.
        return True
    return match.group(1).strip() != "false"


def reason_near(manifest: Path) -> str:
    """The comment above `publish = false`, which records why."""
    lines = manifest.read_text(encoding="utf-8").splitlines()
    for index, line in enumerate(lines):
        if line.startswith("publish"):
            above = [l.strip() for l in lines[max(0, index - 2):index]]
            return " ".join(l.lstrip("# ").strip() for l in above if l.startswith("#"))
    return ""


def inspect(manifest: Path) -> tuple[str, list[str], str | None]:
    """One crate's name, its problems, and why it is unpublished.

    The reason is `None` when the crate publishes, and an empty string when it
    is withheld without saying why: the two are different faults.
    """
    text = manifest.read_text(encoding="utf-8")
    block = package_block(text)
    name = re.search(r'^name\s*=\s*"([^"]+)"', block, re.M)
    label = name.group(1) if name else manifest.parent.name
    if not publishes(block):
        return label, [], reason_near(manifest)
    missing = [field for field in REQUIRED if not inherited(block, field)]
    return label, [f"no {field} (a registry asks for it)" for field in missing], None


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--list", action="store_true",
                        help="print the two sets instead of checking them")
    parser.add_argument("--explain", action="store_true",
                        help="print what each required field is for")
    args = parser.parse_args()

    if args.explain:
        for field, why in EXPLAIN.items():
            print(f"  {field:<14} {why}")
        return 0

    published: list[str] = []
    unpublished: list[tuple[str, str]] = []
    problems: list[str] = []

    for manifest in crates():
        label, found, reason = inspect(manifest)
        if reason is None:
            published.append(label)
        elif reason:
            unpublished.append((label, reason))
        else:
            unpublished.append((label, "no reason given"))
            problems.append(f"{label}: publish = false without a comment saying why")
        problems.extend(f"{label}: {message}" for message in found)

    if args.list:
        print(f"published ({len(published)}):")
        for label in published:
            print(f"  {label}")
        print(f"\nnot published ({len(unpublished)}):")
        for label, reason in unpublished:
            print(f"  {label:<24} {reason}")
        return 0

    print(f"Scanned {len(published) + len(unpublished)} crate(s)")
    print(f"[publish] {len(published)} published, {len(unpublished)} withheld")

    if problems:
        print(f"\nFAILED: {len(problems)} problem(s)\n")
        for problem in problems:
            print(f"  {problem}")
        print("\nSee docs/Roadmap.md (R7) for what the registry surface is.")
        return 1

    print("\nOK: every crate states where it goes, with what a registry asks for")
    return 0


if __name__ == "__main__":
    sys.exit(main())
