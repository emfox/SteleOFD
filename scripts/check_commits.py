#!/usr/bin/env python3
"""Check commit messages against section 3 of docs/CONVENTIONS.md.

The rule is one line of subject, optionally followed by at most two lines of
body. Anything longer belongs in docs/, where it can be reviewed properly.

Usage:
    check_commits.py [--base REF | --message FILE] [REF]

    --base REF     check commits in REF..HEAD (the default in CI, where a
                   pull request makes the base known)
    --message FILE check one message file, for the commit-msg hook
    REF            check the history reachable from REF (default: HEAD)

Exits non-zero and lists every violation. The point is to fail the build
before the history is written, not to ask a human to remember a rule.
"""

import argparse
import re
import subprocess
import sys

# Section 3 of docs/CONVENTIONS.md: the allowed commit types.
TYPES = {
    "feat", "fix", "docs", "chore", "refactor", "perf", "test", "build", "ci",
}

# At most two lines of body, which is what "a short body" means in the rules.
MAX_BODY_LINES = 2

# A subject is one line; past this it stops being scannable in git log. The
# limit is counted in bytes, not characters, so it holds for any content.
MAX_SUBJECT_BYTES = 75

SUBJECT_RE = re.compile(r"^([a-z]+)(?:\(([^)]+)\))?: (.+)$")
CJK_RE = re.compile(r"[\u3000-\u303f\u4e00-\u9fff\uff00-\uffef]")


def check(subject: str, body: str) -> list[str]:
    """Report every rule the message breaks."""
    problems = []
    match = SUBJECT_RE.match(subject)
    if not match:
        problems.append("subject is not '<type>(<scope>): <subject>'")
    else:
        kind, _, text = match.groups()
        if kind not in TYPES:
            problems.append(f"type {kind!r} is not one of {sorted(TYPES)}")
        if text.endswith("."):
            problems.append("subject has a trailing period")
        size = len(subject.encode("utf-8"))
        if size > MAX_SUBJECT_BYTES:
            problems.append(
                f"subject is {size} bytes (max {MAX_SUBJECT_BYTES})")

    lines = [line for line in body.split("\n") if line.strip()]
    if len(lines) > MAX_BODY_LINES:
        problems.append(f"body has {len(lines)} lines (max {MAX_BODY_LINES})")
    if CJK_RE.search(subject + body):
        problems.append("message contains CJK")
    return problems


def messages_in(ref: str) -> list[tuple[str, str, str]]:
    """Every commit in `ref` as (short sha, subject, body)."""
    raw = subprocess.run(
        ["git", "log", "--no-merges", "--format=%h%x1f%s%x1f%b%x1e", ref],
        capture_output=True, text=True, check=True).stdout
    found = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        sha, subject, body = record.split("\x1f", 2)
        found.append((sha, subject, body))
    return found


def messages_in_range(base: str, head: str = "HEAD") -> list[tuple[str, str, str]]:
    """Every commit in base..head, the set a pull request proposes."""
    rev = f"{base}..{head}" if base else head
    raw = subprocess.run(
        ["git", "log", "--no-merges", "--format=%h%x1f%s%x1f%b%x1e", rev],
        capture_output=True, text=True, check=True).stdout
    found = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        sha, subject, body = record.split("\x1f", 2)
        found.append((sha, subject, body))
    return found


def report(entries: list[tuple[str, str, str]]) -> int:
    """Print every violation; return how many commits were rejected."""
    rejected = 0
    for sha, subject, body in entries:
        problems = check(subject, body)
        if not problems:
            continue
        rejected += 1
        print(f"{sha} {subject}")
        for problem in problems:
            print(f"    - {problem}")
    total = len(entries)
    if rejected:
        print(f"\n{rejected} of {total} commit(s) break section 3 of "
              f"docs/CONVENTIONS.md")
    else:
        print(f"OK: {total} commit(s) follow the convention")
    return rejected


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("ref", nargs="?", default="HEAD")
    parser.add_argument("--base", default=None,
                        help="check this ref..HEAD instead of all of REF")
    parser.add_argument("--message", default=None,
                        help="check one message file (the commit-msg hook)")
    parser.add_argument("--grandfathered", default=None,
                        help="a baseline file of short shas to exempt")
    args = parser.parse_args()

    exempt = set()
    if args.grandfathered:
        with open(args.grandfathered, encoding="utf-8") as handle:
            exempt = {line.split()[0] for line in handle
                      if line.strip() and not line.startswith("#")}

    if args.message:
        with open(args.message, encoding="utf-8") as handle:
            raw = handle.read()
        lines = raw.split("\n")
        # Skip comment lines git strips before storing the message.
        kept = [line for line in lines if not line.startswith("#")]
        subject = kept[0].strip()
        body = "\n".join(kept[1:]).strip()
        # The hook runs before the commit exists, so there is no sha to exempt.
        return 1 if report([("message", subject, body)]) else 0

    try:
        entries = (messages_in_range(args.base) if args.base
                   else messages_in(args.ref))
    except subprocess.CalledProcessError as err:
        print(f"cannot read git history: {err}", file=sys.stderr)
        return 2
    return 1 if report([e for e in entries if e[0] not in exempt]) else 0


if __name__ == "__main__":
    sys.exit(main())
