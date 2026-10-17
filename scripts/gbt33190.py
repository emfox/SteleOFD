#!/usr/bin/env python3
"""Take the clause list and the normative schemas of GB/T 33190-2016 from its text.

Two things in this repository have to agree with the standard: the conformance
register, which names clauses, and `compliance/schema/annex-a`, which holds the
XSD files annex A prints. Both are read out of the transcription,
`GBT33190-2016.zh_CN.md`, which the pinned fixtures repository holds beside the
issued PDF. Taking them from the transcription rather than from the PDF keeps
the page geometry where it belongs -- with the transcriber, which is stored
with the text because its only output is the text.

**Clauses.** A heading is a `#` line that opens with a clause number. The
transcription sets the depth the number has, and the hash count is not trusted
to agree with it, because annex A is set one level shallower than its numbers
imply. The contents pages are read as well, and the two readings are compared:
a heading the body scan misses, or reads twice, shows up as a difference.

**Schemas.** Annex A prints each schema as a listing under its own heading,
`A.1 OFD.xsd` and so on. The listings are read as they stand, comments and all,
and written out one file per heading. `Definition.xsd` is written as
`Definitions.xsd`, the name the three schemas that include it use.

Usage:
    python3 scripts/gbt33190.py clauses <standard.zh_CN.md> [--json out.json] [--register REG]
    python3 scripts/gbt33190.py schemas <standard.zh_CN.md> [--out DIR] [--check DIR]
    python3 scripts/gbt33190.py --fetch clauses --register compliance/gb-t-33190.toml
    python3 scripts/gbt33190.py --fetch schemas --check compliance/schema/annex-a

`--fetch` reads the transcription out of the fixtures commit, held to the
SHA-256 its manifest records; name a path instead to read a local copy. The
fetch needs `https_proxy` to leave the machine: `urllib` does not read
`ALL_PROXY`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import xml.etree.ElementTree as ElementTree
from pathlib import Path

from fixtures import FixtureError, Fixtures

# The set and file `--fetch` reads: see `fixtures.py` for the pinned commit.
FIXTURE_SET = "gb-t-33190-text"
FIXTURE_FILE = "GBT33190-2016.zh_CN.md"
# The annex that holds the schemas, and the directory they are kept in.
ANNEX_LETTER = "A"
SCHEMA_DIR = Path("compliance") / "schema" / "annex-a"
# Annex A.6 is printed as `Definition.xsd`, but A.1-A.3 include it as
# `Definitions.xsd`; the schema set is only consistent under the latter name.
RENAMES = {"Definition": "Definitions"}

# A heading number is `12`, `8.3.4.1` or, in annex A, `A.1`.
CLAUSE = re.compile(r"^(\d{1,2}(?:\.\d{1,2}){0,3}|[A-Z]\.\d{1,2})\s+(\S.*)$")
# A heading line, at any depth; the hashes do not have to agree with the
# number, since a transcription may set annex A one level shallower.
HEADING = re.compile(r"^(#{1,6})\s+(\S.*)$")
# A schema's heading, which opens its listing: `A.1 OFD.xsd`.
SCHEMA = re.compile(r"^([A-Z])\.(\d{1,2})\s+(\S+\.xsd)$")
# The tag that closes a schema, which is what ends its listing.
SCHEMA_END = re.compile(r"^</\s*xs:schema\s*>$")
# The contents open with `## <two characters for "contents">`, spaced apart.
CONTENTS = re.compile(r"^#+\s*\u76ee\s*\u6b21$")
# Annex opening, as `# <annex> A (<qualifier>) Schema`; the qualifier is dropped.
ANNEX = re.compile(r"^\u9644\s*\u5f55\s*([A-Z])\s*(?:[(\uff08][^)\uff09]*[)\uff09])?\s*(.*)$")
# `1 <title> 1`, `8.3.4.1 <title> 26`: a page number follows the title.
ENTRY = re.compile(r"^(\d{1,2}(?:\.\d{1,2}){0,3})\s+(\S.*?)\s+(\d{1,3})$")
# Fenced code, which holds the schema listings and the XML examples.
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


def schemas(lines: list[str]) -> list[tuple[str, str]]:
    """`(file name, text)` for every schema printed in annex A, in order."""
    out: list[tuple[str, str]] = []
    name, fenced, listing = "", False, []
    for line in lines:
        if FENCE.match(line):
            fenced = not fenced
            if fenced:
                continue
            # A fenced block belongs to the schema named above it, read from
            # its declaration to the tag that closes it. The body of the
            # standard prints blocks of XML with no schema named above them,
            # and those are skipped.
            if name and listing:
                out.append((name, extract(listing)))
                name, listing = "", []
            continue
        if not fenced:
            heading = HEADING.match(line)
            match = SCHEMA.match(heading.group(2).strip()) if heading else None
            if match is not None and match.group(1) == ANNEX_LETTER:
                stem = match.group(3).split(".")[0]
                name = RENAMES.get(stem, stem) + ".xsd"
            continue
        listing.append(line)
    return out


def extract(listing: list[str]) -> str:
    """The schema a fenced block holds, from its declaration to its close.

    The annex prints an XML example between the two halves of `OFD.xsd`, so
    the block is read for the declaration, not from its first line, and
    stops at the tag that closes the schema rather than at the fence.
    """
    start = next((n for n, line in enumerate(listing) if line.strip().startswith("<?xml")), 0)
    for n in range(start, len(listing)):
        if SCHEMA_END.match(listing[n].strip()):
            return "\n".join(listing[start : n + 1])
    return "\n".join(listing[start:])


def register_claims(register: Path) -> list[tuple[str, str]]:
    """`(clause, title)` pairs the conformance register claims."""
    text = register.read_text(encoding="utf-8")
    return re.findall(r'^clause = "([^"]+)"\ntitle = "([^"]+)"', text, re.M)


def report_clauses(lines: list[str], args: argparse.Namespace) -> int:
    """Print the clause list, compare it with the contents, and check the register."""
    found = body(lines)
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


def report_schemas(lines: list[str], args: argparse.Namespace) -> int:
    """Write the schemas out, or compare them with the copy the repository keeps."""
    found = schemas(lines)
    out_dir: Path = args.out or SCHEMA_DIR
    if args.check is not None:
        out_dir, readable = args.check, True
    else:
        out_dir.mkdir(parents=True, exist_ok=True)
        readable = False

    if not found:
        print("no schema in the transcription: is this the annex A text?", file=sys.stderr)
        return 1

    stale = 0
    for name, text in found:
        if not readable:
            (out_dir / name).write_text(text + "\n", encoding="utf-8")
            print(f"wrote {name} ({len(text)} chars)")
            continue
        path = out_dir / name
        if not path.exists():
            print(f"missing: {name}")
        elif same(path.read_text(encoding="utf-8"), text):
            continue
        else:
            print(f"differs: {name}")
        stale += 1

    names = {name for name, _ in found}
    for path in sorted(out_dir.glob("*.xsd")):
        if path.name not in names:
            print(f"not in the standard: {path.name}")
            stale += 1
    print(f"{len(found)} schema file(s) in {out_dir}")
    if readable and not stale:
        print("the schemas on disk are the ones the standard prints")
    return 1 if stale else 0


def same(held: str, found: str) -> bool:
    """Whether two schema files hold the same XML.

    The white space between the tags is the listing's own, and a copy of the
    file may set it differently, so the comparison is made on the tree: the
    elements and their attributes in order, with the text a listing carries.
    """
    return tree(held) == tree(found)


def tree(text: str) -> list[tuple[str, str, list[tuple[str, str]]]]:
    """A schema as `(tag, text, attributes)` records in document order.

    A listing sets a comment's marks apart from the text between them
    (`<! -- ... -->`), which no parser reads as a comment. The comment is
    taken out before the tree is built: what is compared is the schema.
    """
    without = re.sub(r"<![^-]*(--)?.*?-->", "", text, flags=re.S)
    try:
        root = ElementTree.fromstring(without)
    except ElementTree.ParseError as error:
        raise SystemExit(f"a schema does not parse: {error}")
    return [(one.tag, (one.text or "").strip(), sorted(one.attrib.items())) for one in root.iter()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("what", choices=("clauses", "schemas"), help="which reading to take")
    parser.add_argument("markdown", type=Path, nargs="?", help="the transcribed standard")
    parser.add_argument("--fetch", action="store_true", help="read the transcription from the pinned fixtures")
    parser.add_argument("--json", type=Path, help="(clauses) write the clause list here")
    parser.add_argument("--register", type=Path, help="(clauses) check a register against the standard")
    parser.add_argument("--out", type=Path, help=f"(schemas) write the files here, default {SCHEMA_DIR}")
    parser.add_argument("--check", type=Path, help="(schemas) compare with these files instead of writing")
    args = parser.parse_args()

    if args.fetch == (args.markdown is not None):
        parser.error("name the transcription, or pass --fetch, but not both")
    if args.fetch:
        try:
            lines = Fixtures(FIXTURE_SET).read(FIXTURE_FILE).decode("utf-8").splitlines()
        except FixtureError as error:
            print(f"cannot read the transcription: {error}")
            return 1
    else:
        lines = args.markdown.read_text(encoding="utf-8").splitlines()

    if args.what == "clauses":
        return report_clauses(lines, args)
    return report_schemas(lines, args)


if __name__ == "__main__":
    sys.exit(main())
