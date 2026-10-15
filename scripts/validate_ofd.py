#!/usr/bin/env python3
"""Validate OFD files against the normative schemas of GB/T 33190-2016 annex A.

Every XML entry of an OFD container is routed to the schema that declares its
root element, then validated. This is what turns "we think our documents are
conformant" into "the normative schema accepts them".

Usage:
    python3 scripts/validate_ofd.py <file.ofd|file.xml>...
    python3 scripts/validate_ofd.py --schema-dir DIR <files>
    python3 scripts/validate_ofd.py --allow-foreign <files>

`--allow-foreign` accepts XML entries whose root element annex A does not
declare. A document is free to carry payloads in vocabularies of its own (the
file a `CustomTag` points at, for instance); those are validated by whoever
defines the vocabulary, not here. The flag is a deliberate opt-in so that a
genuinely misplaced annex A entry still fails loudly by default.
"""

from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path
from typing import Iterable

from lxml import etree

# Root element -> schema that declares it (GB/T 33190-2016 annex A).
ROOT_SCHEMA = {
    "OFD": "OFD.xsd",
    "Document": "Document.xsd",
    "Page": "Page.xsd",
    "Res": "Res.xsd",
    "Annotations": "Annotations.xsd",
    "PageAnnot": "Annotation.xsd",
    "Signatures": "Signatures.xsd",
    "Signature": "Signature.xsd",
    "CustomTags": "CustomTags.xsd",
    "Extensions": "Extensions.xsd",
    "Attachments": "Attachments.xsd",
    "DocVersion": "Version.xsd",
}

OFD_NS = "http://www.ofdspec.org/2016"


def local_name(tag: object) -> str:
    """Strip the `{namespace}` prefix from an element tag."""
    return str(tag).split("}", 1)[-1] if isinstance(tag, str) else ""


def load_validators(schema_dir: Path) -> dict[str, etree.XMLSchema]:
    validators: dict[str, etree.XMLSchema] = {}
    for path in sorted(schema_dir.glob("*.xsd")):
        try:
            validators[path.name] = etree.XMLSchema(etree.parse(str(path)))
        except etree.XMLSchemaParseError as error:
            raise SystemExit(f"schema {path.name} is not usable: {error}") from error
    return validators


def validate(
    data: bytes,
    label: str,
    validators: dict[str, etree.XMLSchema],
    allow_foreign: bool = False,
) -> tuple[str, list[str]]:
    """Validate one entry; return ("ok"|"skip"|"fail", problems)."""
    try:
        document = etree.fromstring(data)
    except etree.XMLSyntaxError as error:
        return "fail", [f"{label}: not well-formed XML: {error}"]
    root = local_name(document.tag)
    if root in ("", "OFD") and document.nsmap.get(None, OFD_NS) != OFD_NS:
        return "fail", [f"{label}: root element is not in the {OFD_NS} namespace"]
    name = ROOT_SCHEMA.get(root)
    if name is None:
        if allow_foreign:
            return "skip", []
        return "fail", [f"{label}: root element <{root}> is not declared by annex A"]
    validator = validators.get(name)
    if validator is None:
        return "fail", [f"{label}: schema {name} is missing from the schema directory"]
    if validator.validate(document):
        return "ok", []
    return "fail", [f"{label}: {error.message}" for error in validator.error_log]


def entries(path: Path) -> Iterable[tuple[str, bytes]]:
    """XML entries of an OFD container, or the file itself when it is XML."""
    if path.suffix.lower() == ".xml":
        yield path.name, path.read_bytes()
        return
    with zipfile.ZipFile(path) as archive:
        for info in archive.infolist():
            if info.is_dir() or not info.filename.lower().endswith(".xml"):
                continue
            yield info.filename, archive.read(info)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument(
        "--allow-foreign",
        action="store_true",
        help="accept XML entries whose root annex A does not declare",
    )
    parser.add_argument(
        "--schema-dir",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "compliance" / "schema" / "annex-a",
    )
    args = parser.parse_args()
    validators = load_validators(args.schema_dir)
    if not validators:
        print(f"no schemas found in {args.schema_dir}", file=sys.stderr)
        return 2

    problems: list[str] = []
    checked = 0
    skipped = 0
    for path in args.files:
        if not path.exists():
            problems.append(f"{path}: no such file")
            continue
        for label, data in entries(path):
            status, found = validate(
                data, f"{path}:{label}", validators, args.allow_foreign
            )
            if status == "ok":
                checked += 1
                print(f"OK   {path}:{label}")
            elif status == "skip":
                skipped += 1
                print(f"SKIP {path}:{label}")
            else:
                problems.extend(found)

    for problem in problems:
        print(f"FAIL {problem}", file=sys.stderr)
    print(f"{checked} entry/entries conform, {skipped} skipped, {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
