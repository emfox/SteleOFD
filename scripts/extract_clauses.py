#!/usr/bin/env python3
"""Read the clause list of GB/T 33190-2016 out of the printed standard.

The conformance register has to name the clauses the standard actually
numbers, so the numbers are read out of the standard rather than from
anyone's reading of it.

A heading is printed in the heading face and starts at one of a handful of
left edges, one per depth. The components leading up to a deep number are
printed on baselines of their own to the left of the heading (`8.`, then
`3.`, then `3 <title>` for `8.3.3`), so a heading is assembled from the row
that carries the title and whatever component rows sit directly above it on
the nearer edges.

The contents pages are read as well and the two lists are compared: a body
scan that misses a heading, or reads one twice, shows up as a difference
between them.

Usage:
    python3 scripts/extract_clauses.py <standard.pdf> [--json out.json]
    python3 scripts/extract_clauses.py <standard.pdf> --register compliance/gb-t-33190.toml
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import fitz

# 1-based PDF page range that holds the body of clauses 1-20.
FIRST_BODY = 6
LAST_BODY = 132
# The contents pages, printed I to III, that list the clauses up to 20.
TOC_PAGES = (3, 4)
# The heading face; body text is set in FZSSK and table cells in E-BZ.
HEADING_FACE = "FZHTK--GBK1-0"
# A heading starts at one of these left edges, one per depth.
INDENTS = (64.9, 73.3, 78.5, 81.7, 87.0, 90.2)
# Runs whose baselines are closer than this share a row; a heading never
# climbs into the line above it the way a subscript does.
BASELINE = 8.0
# Runs on one baseline belong to one row when they sit this close together.
PART = 1.0
# The left edge of the page's text column; anything further right is a table
# cell or a figure label rather than a heading.
COLUMN = 100.0
# `8.3.4.1 <title>`, `A.1 OFD.xsd`, `A.2 Document.xsd`
NUMBER = re.compile(r"^(\d+(?:\.\d+)*|[A-Z]\.\d+(?:\.\d+)*)([^\d.].*)$")
# A heading number has at most A.B.C.D, and no component above two digits.
DEPTH = 4
# Heading titles are Chinese; a line that runs on into a figure or a table
# number (`<title> ... 58 ...`) is body text set in the heading face.
TITLE = re.compile(r"[^\u4e00-\u9fffA-Za-z]")


def indent_of(x0: float) -> int | None:
    """The edge at `x0`, counted from the left, or `None` if it is not one."""
    for edge, indent in enumerate(INDENTS, start=1):
        if abs(x0 - indent) < 0.5:
            return edge
    return None


def page_rows(page: fitz.Page) -> list[tuple[float, float, str, bool, bool]]:
    """The page's text rows as `(y0, x0, text, face, heading)`, top to bottom.

    Runs on one baseline are joined in reading order. `heading` is true when
    the row is a heading or a leading component of one, which is what tells
    `2 <title>` apart from the table row `FontFile`, since a label that sits
    in a narrow table column is typeset from further right.
    """
    rows: list[list] = []
    for block in page.get_text("dict")["blocks"]:
        for line in block.get("lines", []):
            text = "".join(span["text"] for span in line["spans"])
            x0, y0, x1, _y1 = line["bbox"]
            if not text.strip() or text.startswith("GB/T"):
                continue
            face = any(span["font"] == HEADING_FACE for span in line["spans"])
            # A run set in the heading face with no white space before it is
            # the inner digit of a deep number (`1` of `8.3.4.1`), which the
            # typesetting lifts a little to line the figure up in height.
            inner = face and text == text.lstrip()
            if rows and abs(rows[-1][0] - y0) < (BASELINE if inner else PART):
                rows[-1][1].append((x0, x1, text, face))
            else:
                rows.append([y0, [(x0, x1, text, face)]])

    out = []
    for y0, runs in rows:
        runs.sort()
        text, face = spread(runs)
        x0 = runs[0][0]
        edge = indent_of(x0)
        out.append((y0, x0, text, face, face and x0 < COLUMN and edge is not None))
    return out


def spread(runs: list[tuple[float, float, str, bool]]) -> tuple[str, bool]:
    """`(text, face)` of one row, from its `(x0, x1, text, face)` runs."""
    text, right, face = "", None, False
    for x0, x1, run, set_face in runs:
        if right is not None and x0 - right > PART:
            text += " "
        text += run
        right = x1 if right is None else max(right, x1)
        face = face or set_face
    return re.sub(r"\s+", " ", text).strip(), face


def parse(text: str) -> tuple[str, str] | None:
    """`(clause number, title)` of a heading row, or `None` if it is not one.

    Term entries (`3.1 fixed layout`) share the face and the edge with
    headings but run the number into the term, so only a number the row
    sets apart with white space opens a clause.
    """
    match = NUMBER.match(text.strip())
    if match is None:
        return None
    number, title = match.group(1), match.group(2).strip()
    if number.count(".") >= DEPTH or any(len(part) > 2 for part in number.split(".")):
        return None
    if not title or TITLE.match(title):
        return None
    return number, title


def assemble(rows: list[tuple[float, float, str, bool, bool]]) -> list[tuple[str, str]]:
    """`(clause, title)` for every heading on a page, in reading order.

    A heading is one row: the number is typeset in the heading face as far
    as its last component and the leading components before it on the row
    are plain. A row of the heading face that carries no number of its own
    is a leading component printed on a baseline of its own (`8.3.4.` above
    `1 <title>` in a narrow column), and is folded into the heading that
    follows it.
    """
    out: list[tuple[str, str]] = []
    lead = ""
    for _y0, _x0, text, _face, heading in rows:
        if heading and re.fullmatch(r"[\d.]+", text):
            lead = text
            continue
        # The schemas of annex A are set as listings, in the body face.
        parsed = parse(text) if heading or re.match(r"[A-Z]\.\d+ ", text) else None
        if parsed is not None:
            number, title = parsed
            out.append((lead + number if number.count(".") else number, title))
            lead = ""
    return out


def headings(pdf: Path) -> list[dict]:
    """Every numbered clause heading of the body, in reading order."""
    doc = fitz.open(pdf)
    out: list[dict] = []
    for page_number in range(FIRST_BODY, LAST_BODY + 1):
        for clause, title in assemble(page_rows(doc[page_number - 1])):
            out.append(
                {
                    "clause": clause,
                    "title": title,
                    "level": clause.count(".") + 1,
                    "pdf_page": page_number,
                }
            )
    return out


def toc(pdf: Path) -> list[dict]:
    """The clause list as the contents pages print it, used as a check."""
    doc = fitz.open(pdf)
    out: list[dict] = []
    for page_number in TOC_PAGES:
        for _y0, _x0, text, _face, _heading in page_rows(doc[page_number - 1]):
            match = NUMBER.match(text.strip())
            if match:
                # The contents set the page number after the title.
                title = re.sub(r"\s+\d+$", "", match.group(2).strip())
                out.append({"clause": match.group(1), "title": title})
    return out


def register_claims(register: Path) -> list[tuple[str, str]]:
    """`(clause, title)` pairs the conformance register claims."""
    text = register.read_text(encoding="utf-8")
    return re.findall(r'^clause = "([^"]+)"\ntitle = "([^"]+)"', text, re.M)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pdf", type=Path)
    parser.add_argument("--json", type=Path, help="write the clause list here")
    parser.add_argument("--register", type=Path, help="check a conformance register against the standard")
    args = parser.parse_args()

    found = headings(args.pdf)
    if args.json:
        args.json.write_text(json.dumps(found, ensure_ascii=False, indent=2), encoding="utf-8")
    for item in found:
        print(f"{item['clause']:<10}{'  ' * (item['level'] - 1)}{item['title']}  (p.{item['pdf_page']})")
    print(f"{len(found)} clauses")

    printed = {item["clause"]: item["title"] for item in toc(args.pdf)}
    title_of = {item["clause"]: item["title"] for item in found}
    mismatched = [c for c, t in printed.items() if title_of.get(c) != t]
    print(f"contents list {len(printed)} clauses, {len(printed) - len(mismatched)} of them read back alike")
    for clause in mismatched:
        print(f"  {clause}: contents {printed[clause]!r}, body {title_of.get(clause)!r}")

    if args.register is None:
        return 1 if mismatched else 0
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
