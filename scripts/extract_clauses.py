#!/usr/bin/env python3
"""Read the clause list of GB/T 33190-2016 out of the transcribed text.

The conformance register has to name the clauses the standard actually
numbers, so the numbers are read out of the standard rather than from
anyone's reading of it. The text this reads is `GBT33190-2016.zh_CN.md`: a
mechanical transcription of the issued PDF, stored beside it in the pinned
fixtures repository, where every numbered heading of the standard appears as
a Markdown heading at the depth its number has.

Taking the numbers from the transcription rather than from the PDF keeps the
geometry out of this repository. Assembling a heading there means knowing that
`8.3.3` is printed as three runs on three baselines and that a deep number is
split across four, which is the PDF transcriber's problem and stays with it.

Two readings are made and compared:

**The body**, where a heading is a `#` line that opens with a clause number.

**The contents pages**, where the same clauses are listed with a page number
after each title. A heading the body scan misses, or reads twice, shows up as
a difference between the two.

Usage:
    python3 scripts/extract_clauses.py <standard.zh_CN.md> [--json out.json]
    python3 scripts/extract_clauses.py <standard.zh_CN.md> --register compliance/gb-t-33190.toml
    python3 scripts/extract_clauses.py --fetch [--register ...]

`--fetch` reads the transcription out of the pinned fixtures commit instead of
from a path, so the check runs against the text the engine was written to.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from fixtures import FixtureError, Fixtures

# The set and file `--fetch` reads: see `fixtures.py` for the pinned commit.
FIXTURE_SET = "gb-t-33190-text"
FIXTURE_FILE = "GBT33190-2016.zh_CN.md"
# A heading number is `12`, `8.3.4.1` or, in annex A, `A.1`.
CLAUSE = re.compile(r"^(\d{1,2}(?:\.\d{1,2}){0,3}|[A-Z]\.\d{1,2})\s+(\S.*)$")
# A heading line, at any depth; the hashes do not have to agree with the
# number, since a transcription may set annex A one level shallower.
HEADING = re.compile(r"^(#{1,6})\s+(\S.*)$")
# The contents open with `## \u76ee \u6b21`, the two characters spaced apart.
CONTENTS = re.compile(r"^#+\s*\u76ee\s*\u6b21$")
# Annex opening, as `# <annex> A (<qualifier>) Schema`; the qualifier is dropped.
ANNEX = re.compile(r"^\u9644\s*\u5f55\s*([A-Z])\s*(?:[(\uff08][^)\uff09]*[)\uff09])?\s*(.*)$")
# `1 <title> 1`, `8.3.4.1 <title> 26`: a page number follows the title.
ENTRY = re.compile(r"^(\d{1,2}(?:\.\d{1,2}){0,3})\s+(\S.*?)\s+(\d{1,3})$")
# Fenced code, which holds schema listings and XML with no headings in it.
FENCE = re.compile(r"^\s*```")
# A clause number has at most A.B.C.D.
DEPTH = 4


def level_of(clause: str) -> int:
    """The depth of a clause number: `8` is 1, `8.3.4.1` is 4, `A.1` is 2."""
    return clause.count(".") + 1


def body(lines: list[str]) -> list[dict]:
    """`(clause, title, level)` for every numbered heading of the body."""
    out: list[dict] = []
    fenced = False
    for line in lines:
        if FENCE.match(line):
            fenced = not fenced
            continue
        if fenced:
            continue
        heading = HEADING.match(line)
        if heading is None:
            continue
        text = heading.group(2).strip()
        clause = CLAUSE.match(text)
        if clause is not None:
            number, title = clause.group(1), clause.group(2).strip()
            if number.count(".") < DEPTH:
                out.append({"clause": number, "title": title, "level": level_of(number)})
            continue
        # Annex A is labelled and then numbered (`A.1 OFD.xsd`), and the
        # numbered headings are collected by the branch above. What is left
        # here is the label, which the contents list as an entry too. Both
        # titles are kept as the transcription sets them, so the two readings
        # compare like for like: one is a label, the other a section name.
        annex = ANNEX.match(text)
        if annex is not None:
            out.append({"clause": annex.group(1), "title": annex.group(2).strip(), "level": 1})
    return out


def contents(lines: list[str]) -> list[dict]:
    """The clause list as the contents pages print it, used as a check."""
    out: list[dict] = []
    inside = False
    for line in lines:
        text = line.strip()
        if CONTENTS.match(text):
            inside = True
            continue
        if not inside:
            continue
        if not text.startswith("- "):
            # The contents end at the first line that is not an entry.
            if text:
                inside = False
            continue
        entry = ENTRY.match(text[2:].strip())
        if entry is not None:
            out.append({"clause": entry.group(1), "title": entry.group(2)})
    return out


def register_claims(register: Path) -> list[tuple[str, str]]:
    """`(clause, title)` pairs the conformance register claims."""
    text = register.read_text(encoding="utf-8")
    return re.findall(r'^clause = "([^"]+)"\ntitle = "([^"]+)"', text, re.M)


def read(text: str) -> list[dict]:
    """Every numbered clause the transcription carries, in reading order."""
    return body(text.splitlines())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("markdown", type=Path, nargs="?", help="the transcribed standard")
    parser.add_argument("--fetch", action="store_true", help="read the transcription from the pinned fixtures")
    parser.add_argument("--json", type=Path, help="write the clause list here")
    parser.add_argument("--register", type=Path, help="check a conformance register against the standard")
    args = parser.parse_args()

    if args.fetch == (args.markdown is not None):
        parser.error("name the transcription, or pass --fetch, but not both")
    if args.fetch:
        try:
            args.markdown = Path(FIXTURE_FILE)
            lines = Fixtures(FIXTURE_SET).read(FIXTURE_FILE).decode("utf-8").splitlines()
        except FixtureError as error:
            print(f"cannot read the transcription: {error}")
            return 1
    else:
        lines = args.markdown.read_text(encoding="utf-8").splitlines()

    found = read("\n".join(lines))
    if args.json:
        args.json.write_text(json.dumps(found, ensure_ascii=False, indent=2), encoding="utf-8")
    for item in found:
        print(f"{item['clause']:<10}{'  ' * (item['level'] - 1)}{item['title']}")
    print(f"{len(found)} clauses")

    printed = contents(lines)
    title_of = {item["clause"]: item["title"] for item in found}
    mismatched = [item["clause"] for item in printed if title_of.get(item["clause"]) != item["title"]]
    print(f"contents list {len(printed)} clauses, {len(printed) - len(mismatched)} of them read back alike")
    for clause in mismatched:
        printed_title = next(item["title"] for item in printed if item["clause"] == clause)
        print(f"  {clause}: contents {printed_title!r}, body {title_of.get(clause)!r}")

    unknown: list[str] = []
    if args.register is not None:
        claims = register_claims(args.register)
        unknown = [clause for clause, _ in claims if clause not in title_of]
        print(f"register claims {len(claims)} clauses, {len(claims) - len(unknown)} of them in the standard")
        for clause, title in claims:
            if clause in title_of and title_of[clause] != title:
                print(f"  title differs at {clause}: standard {title_of[clause]!r}, register {title!r}")
        for clause in unknown:
            print(f"  NOT IN THE STANDARD: {clause}")
    return 1 if unknown or mismatched else 0


if __name__ == "__main__":
    sys.exit(main())
