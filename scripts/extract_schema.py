#!/usr/bin/env python3
"""Rebuild the normative XSD files of GB/T 33190-2016 annex A.

Annex A prints the schemas staircase-indented. Punctuation glyphs such as
`<`, `/`, `:` and `>` are drawn with a taller bounding box than the letters,
so grouping words by their top edge splits a single printed line in two. This
script groups words by the *centre* of their bounding box instead, which puts
punctuation and letters back on the same row, orders each row left to right,
and re-inserts the spaces that the extractor dropped inside long runs.

Usage:
    python3 scripts/extract_schema.py <standard.pdf> [output_dir]
"""

from __future__ import annotations

import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

FIRST_PAGE = 96
LAST_PAGE = 132
# Annex A.6 is printed as `Definition.xsd`, but A.1-A.3 include it as
# `Definitions.xsd`; the schema set is only consistent under the latter name.
RENAMES = {"Definition": "Definitions"}
# Header and footer bands of the printed page, in points.
TOP_MARGIN = 95.0
BOTTOM_MARGIN = 780.0
# Two words whose vertical centres differ by less than this belong to one row.
ROW_TOLERANCE = 6.0
# Horizontal gap above which a space is restored between two words. Kept
# generous: the extractor already glues letters together, so real spaces are
# recovered from the vocabulary rather than from geometry.
GAP = 4.0

# Vocabulary used to split runs such as `attributename="Version"type=` back into
# separate tokens. Longer entries first so that `maxOccurs` wins over `Occurs`.
TOKENS = (
    # XML Schema elements.
    "all", "annotation", "any", "anyAttribute", "appinfo", "attribute",
    "attributeGroup", "choice", "complexContent", "complexType", "documentation",
    "element", "enumeration", "extension", "field", "fractionDigits", "group",
    "import", "include", "key", "keyref", "length", "list", "maxExclusive",
    "maxInclusive", "maxLength", "minExclusive", "minInclusive", "minLength",
    "pattern", "redefine", "restriction", "schema", "selector",
    "sequence", "simpleContent", "simpleType", "totalDigits", "union", "unique",
    "whiteSpace",
    # XML Schema attributes.
    "abstract", "attributeFormDefault", "base", "block", "default", "encoding",
    "elementFormDefault", "final", "fixed", "form", "id", "itemType",
    "maxOccurs", "memberTypes", "minOccurs", "mixed", "name", "namespace",
    "nillable", "processContents", "ref", "schemaLocation", "source",
    "substitutionGroup", "targetNamespace", "type", "use", "value", "version",
    "xpath", "xmlns:xs", "xmlns",
    # XML Schema built-in types and fixed values.
    "anySimpleType", "anyType", "anyURI", "base64Binary", "boolean", "byte",
    "date", "dateTime", "decimal", "double", "duration", "float", "hexBinary",
    "ID", "IDREF", "IDREFS", "int", "integer", "language", "long", "NCName",
    "NMTOKEN", "NMTOKENS", "nonNegativeInteger", "normalizedString", "positiveInteger",
    "QName", "short", "string", "time", "token", "unbounded", "unsignedByte",
    "unsignedInt", "unsignedLong", "unsignedShort",
    "optional", "prohibited", "qualified", "required", "unqualified",
    "collapse", "false", "preserve", "replace", "true",
)


def words(pdf: Path) -> list[str]:
    """Words of annex A in reading order."""
    proc = subprocess.run(
        ["pdftotext", "-bbox", "-f", str(FIRST_PAGE), "-l", str(LAST_PAGE), str(pdf), "-"],
        capture_output=True,
        text=True,
        check=True,
    )
    ns = "{http://www.w3.org/1999/xhtml}"
    root = ET.fromstring(proc.stdout)
    rows: list[list[tuple[float, float, str]]] = []
    for page in root.iter(f"{ns}page"):
        placed: list[tuple[float, float, float, str]] = []
        for word in page.iter(f"{ns}word"):
            y_min = float(word.get("yMin", "0"))
            y_max = float(word.get("yMax", "0"))
            centre = (y_min + y_max) / 2.0
            if not TOP_MARGIN <= centre <= BOTTOM_MARGIN:
                continue
            placed.append((centre, float(word.get("xMin", "0")),
                           float(word.get("xMax", "0")), word.text or ""))
        rows.extend(group_rows(placed))
    stream: list[str] = []
    for row in rows:
        row.sort(key=lambda item: item[1])
        previous = None
        for x_min, x_max, text in row:
            if previous is not None and x_min - previous > GAP:
                stream.append(" ")
            stream.append(text)
            previous = x_max
        stream.append(" ")
    return stream


def group_rows(placed: list[tuple[float, float, float, str]]) -> list[list[tuple[float, float, str]]]:
    """Cluster words into printed rows by the centre of their bounding box."""
    rows: list[list[tuple[float, float, str]]] = []
    current: list[tuple[float, float, float, str]] = []
    for item in sorted(placed, key=lambda word: word[0]):
        if current and item[0] - current[-1][0] > ROW_TOLERANCE:
            rows.append([(x_min, x_max, text) for _, x_min, x_max, text in current])
            current = []
        current.append(item)
    if current:
        rows.append([(x_min, x_max, text) for _, x_min, x_max, text in current])
    return rows


# Longest match first so that `namespace` wins over `name`.
TOKEN_RE = re.compile(
    "(" + "|".join(re.escape(token) for token in sorted(TOKENS, key=len, reverse=True)) + ")"
)


def respace(text: str) -> str:
    """Re-insert the spaces that staircase printing removed."""
    text = fix_prefix(text)
    text = drop_stray_numbers(text)
    parts = TOKEN_RE.split(text)
    rebuilt: list[str] = []
    for index, part in enumerate(parts):
        if index % 2 and rebuilt:
            # Only split where a vocabulary word starts an attribute, i.e. is
            # followed by `=`. Splitting elsewhere would cut camel-cased names
            # such as `PageID`, `Subtype` or `Annotations` in half.
            previous = rebuilt[-1][-1:]
            starts_attribute = index + 1 < len(parts) and parts[index + 1].startswith("=")
            if starts_attribute and part[:1].islower() and (
                previous.isalnum() or previous in "_\"-/"
            ):
                rebuilt.append(" ")
        rebuilt.append(part)
    text = "".join(rebuilt)
    text = re.sub(r"<\?\s+", "<?", text)
    text = re.sub(r"<!\s+--", "<!--", text)
    text = text.replace("><", "> <")
    text = re.sub(r"\b(ST|CT)_?([A-Z][a-z])", r"\1_\2", text)
    text = re.sub(r'"xs([A-Za-z])', '"xs:\\1', text)
    text = fix_comments(text)
    return re.sub(r"\s+", " ", text).strip()


def drop_stray_numbers(text: str) -> str:
    """Drop bare integers that the margin printed next to the code."""
    kept: list[str] = []
    quoted = False
    position = 0
    while position < len(text):
        char = text[position]
        if char == '"':
            quoted = not quoted
        if not quoted and char.isdigit():
            start = position
            while position < len(text) and text[position].isdigit():
                position += 1
            before = kept[-1][-1:] if kept else ""
            after = text[position : position + 1]
            if before in " \t\n" and after in " \t\n<>/":
                continue
            kept.append(text[start:position])
            continue
        kept.append(char)
        position += 1
    return "".join(kept)


def fix_prefix(text: str) -> str:
    """Staircase printing scatters the `xs:` prefix; glue it back together."""
    text = re.sub(r"<\s*:+\s*[/_>]*\s*(?:xs)?\s*", "<xs:", text)
    text = re.sub(r"(?<=<)xs(?=[A-Za-z])", "xs:", text)
    text = re.sub(r"(?<=</)(?!xs:)(?=[A-Za-z])", "xs:", text)
    return text


def fix_comments(text: str) -> str:
    def replace(match: re.Match[str]) -> str:
        body = match.group(1).strip().strip("-").strip()
        return f"<!-- {body} -->" if body else ""

    return re.sub(r"<!--?\s*(.*?)\s*-{0,2}\s*>", replace, text)


def split_fragments(stream: str) -> dict[str, str]:
    """Cut the word stream at annex headings such as `A.1 OFD.xsd`."""
    # Restore spacing first: the `<?xml` declaration is only recognisable once
    # the extractor-glued runs have been split.
    stream = respace(stream)
    marks = list(re.finditer(r"A\.\d*\s*([A-Za-z]+)\s*\.?\s*xsd", stream))
    schemas: dict[str, str] = {}
    for index, mark in enumerate(marks):
        start = mark.end()
        end = marks[index + 1].start() if index + 1 < len(marks) else len(stream)
        begin = stream.find("<?xml", start, end)
        if begin < 0:
            continue
        body = stream[begin:end]
        # Trim whatever the next annex section contributed.
        closing = list(re.finditer(r"</\s*xs:schema\s*>", body))
        if closing:
            body = body[: closing[-1].end()]
        name = RENAMES.get(mark.group(1), mark.group(1))
        schemas[f"{name}.xsd"] = body
    return schemas


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    pdf = Path(sys.argv[1])
    out_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/tmp/schema_out")
    out_dir.mkdir(parents=True, exist_ok=True)
    stream = "".join(words(pdf))
    schemas = split_fragments(stream)
    for name, text in schemas.items():
        (out_dir / name).write_text(text + "\n", encoding="utf-8")
        print(f"wrote {name} ({len(text)} chars)")
    print(f"{len(schemas)} schema file(s) in {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
