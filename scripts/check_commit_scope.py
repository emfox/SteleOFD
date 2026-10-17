#!/usr/bin/env python3
"""Report commits that carry more than one logical change.

Section 3.1 of docs/CONVENTIONS.md asks for one logical change per commit. A
script cannot read intent, so this reports the shape a squashed commit leaves:
one commit whose changed lines are spread across areas that no single change
explains.

Spreading across areas is ordinary. A behaviour change and its test live in
different files, an i18n change reaches every caller, and a rename touches every
mention. Those commits share a reason, and the reason is visible as a small,
even spread. The shape of a squash is different: whole features changed in
several places, with the bulk of the lines somewhere unrelated.

That is what the line counts are for. A commit that touches five areas with
three lines each is one change with many mentions; a commit that rewrites two
hundred lines in one crate and forty in another while the subject names neither
is two changes sharing a subject.

Usage:
    check_commit_scope.py [--limit N] [--verbose] [REF]

Exit code: 0 when nothing is worth a second look, 1 otherwise. This is advisory
review input, not a merge gate: it names commits to read again, and whether the
split was right stays a human decision.
"""

import argparse
import re
import subprocess
import sys

# A commit needs this many changed lines outside its largest area before a
# second area can be called a second change. Below it, the other areas are
# mentions: a call site, a lock file, a test that follows the change.
MIN_SECONDARY_LINES = 40

# Files generated from another change in the same commit, so they never count
# as an area of their own: regenerating one is part of touching its source.
GENERATED = {"docs/COMPLIANCE.md", "Cargo.lock"}

SUBJECT_RE = re.compile(r"^([a-z]+)(?:\(([^)]+)\))?: (.+)$")


def area(path: str) -> str:
    """The top-level area a path belongs to."""
    parts = path.split("/")
    if parts[0] == "crates" and len(parts) > 1:
        return f"crates/{parts[1]}"
    if parts[0] == "crates":
        return "crates"
    if len(parts) > 1 and parts[0] in ("scripts", "compliance", "docs",
                                       ".github", "locales", "fuzz", "tests"):
        return parts[0]
    return parts[0]


def commits(ref: str) -> list[tuple[str, str, dict[str, int]]]:
    """Every commit as (short sha, subject, {area: changed lines})."""
    raw = subprocess.run(
        ["git", "log", "--no-merges", "--no-renames", "--numstat",
         "--format=%x1e%h%x1f%s", ref],
        capture_output=True, text=True, check=True).stdout
    found: list[tuple[str, str, dict[str, int]]] = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        header, _, rest = record.partition("\n")
        sha, _, subject = header.partition("\x1f")
        lines: dict[str, int] = {}
        for line in rest.split("\n"):
            fields = line.split("\t")
            if len(fields) != 3:
                continue
            added, removed, path = fields
            # A binary file states "-" instead of a count.
            count = (int(added) if added.isdigit() else 0) + \
                    (int(removed) if removed.isdigit() else 0)
            if path in GENERATED:
                continue
            key = area(path)
            lines[key] = lines.get(key, 0) + count
        found.append((sha, subject, lines))
    return found


def review(subject: str, lines: dict[str, int]) -> list[str]:
    """Why this commit looks like more than one change, if it does."""
    if len(lines) < 2:
        return []

    ranked = sorted(lines.items(), key=lambda kv: kv[1], reverse=True)
    primary, primary_lines = ranked[0]
    secondary = [(a, n) for a, n in ranked[1:] if n >= MIN_SECONDARY_LINES]
    if not secondary:
        return []

    match = SUBJECT_RE.match(subject)
    scope = match.group(2) if match else None
    named = f"crates/steleofd-{scope}" if scope else None

    # The subject names the area it changes. A second area of that size is a
    # second change, unless the subject is the only place the scope appears.
    reasons = []
    if named and primary != named and primary_lines >= MIN_SECONDARY_LINES:
        reasons.append(f"subject names {named} but the bulk of the change "
                       f"({primary_lines} lines) is in {primary}")

    heavy = [(a, n) for a, n in secondary if n >= primary_lines * 0.5]
    if heavy:
        total = sum(lines.values())
        outside = sum(n for _, n in ranked[1:])
        # A move or a rename concentrates its lines in a few areas because the
        # areas are the thing being moved. A squash spreads them, because
        # unrelated work does not concentrate.
        if len(heavy) >= 2 and outside * 100 // max(total, 1) >= 50:
            share = outside * 100 // max(total, 1)
            reasons.append(
                f"{share}% of {total} changed lines falls outside {primary}, "
                f"spread over {len(heavy)}+ areas: "
                + ", ".join(f"{a} ({n})" for a, n in heavy))
    return reasons


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ref", nargs="?", default="HEAD")
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    try:
        entries = commits(args.ref)
    except subprocess.CalledProcessError as err:
        print(f"cannot read git history: {err}", file=sys.stderr)
        return 2

    # A shallow clone pins a synthetic root whose diff is the whole tree; it
    # says nothing about how the repository was written.
    roots = set()
    try:
        roots = set(subprocess.run(
            ["git", "rev-list", "--max-parents=0", args.ref],
            capture_output=True, text=True, check=True).stdout.split())

        shallow = subprocess.run(
            ["git", "rev-parse", "--is-shallow-repository"],
            capture_output=True, text=True, check=True).stdout.strip()
    except subprocess.CalledProcessError:
        shallow = "false"

    flagged = []
    for sha, subject, lines in entries:
        if shallow == "true" and any(r.startswith(sha) for r in roots):
            continue
        reasons = review(subject, lines)
        if reasons:
            flagged.append((sha, subject, lines, reasons))

    shown = flagged[:args.limit]
    for sha, subject, lines, reasons in shown:
        print(f"{sha} {subject}")
        for reason in reasons:
            print(f"    ! {reason}")
        if args.verbose:
            for name, count in sorted(lines.items(), key=lambda kv: -kv[1]):
                print(f"      {count:>6}  {name}")

    total = len(entries)
    if flagged:
        print(f"\n{len(flagged)} of {total} commit(s) carry lines a single "
              f"change does not explain (section 3.1)")
        if len(flagged) > len(shown):
            print(f"{len(flagged) - len(shown)} more not shown; "
                  f"raise --limit to see them")
        return 1
    print(f"OK: {total} commit(s) each read as a change of their own")
    return 0


if __name__ == "__main__":
    sys.exit(main())
