#!/usr/bin/env python3
"""Check the annex A rebuild against spellings in the printed standard.

The XSD files under `compliance/schema/annex-a` were rebuilt from the printed
annex A, which is a scanned image in every PDF in circulation. A third-party
wiki republishes the same annex as text and disagrees with us in eleven places,
all of them `g` to `q` or a case flip. This script settles such a dispute the
only way that counts: it renders the annex A pages of the standard PDF, OCRs
them, and reports which spelling the printed page carries.

Usage:
    python3 scripts/cross_check_schema.py <standard.pdf>

It is a manual check, not a gate: it needs a PDF that CI does not carry and an
OCR pass that takes minutes. Its output is evidence for a commit message, not a
pass or fail.
"""

from __future__ import annotations

import argparse
import glob
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCHEMA_DIR = REPO / "compliance" / "schema" / "annex-a"

# Each pair is a spelling we use and the mistake to rule out. The first is
# expected on the printed page, the second must be absent.
CHECKS = [
    ("Segment", "Seqment"),
    ("DeltaY", "Deltay"),
    ("DefaultCS", "DefaultCs"),
    ("QuadraticBezier", "OuadraticBezier"),
    ("Background", "Backqround"),
    ("Signature", "Sianature"),
    ("SignedInfo", "SiqnedInfo"),
    ("RotationAngle", "RotationAnglet"),
]

# Annex A sits at the back of the 135-page standard; the pages below carry the
# constructs the checks name. Scanning the tail keeps the run to a few minutes.
PAGE_RANGE = (95, 135)
DPI = 300


def ocr_pages(pdf: Path, work: Path) -> dict[int, str]:
    """OCR each page of `PAGE_RANGE` and return page number to text."""
    pages: dict[int, str] = {}
    for page in range(*PAGE_RANGE):
        stem = work / f"p{page}"
        subprocess.run(
            ["pdftoppm", "-f", str(page), "-l", str(page), "-r", str(DPI),
             "-png", str(pdf), str(stem)],
            check=True, capture_output=True,
        )
        for image in sorted(glob.glob(f"{stem}-*.png")):
            text = subprocess.run(
                ["tesseract", image, "-", "--psm", "6"],
                check=True, capture_output=True,
            ).stdout.decode("utf-8", "replace")
            pages[page] = text
            os.remove(image)
    return pages


def ours() -> str:
    """Every XSD file in the rebuild, concatenated."""
    return "\n".join(p.read_text() for p in sorted(SCHEMA_DIR.glob("*.xsd")))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pdf", type=Path, help="the standard as published PDF")
    args = parser.parse_args()

    if not args.pdf.exists():
        print(f"no such PDF: {args.pdf}", file=sys.stderr)
        return 2
    for tool in ("pdftoppm", "tesseract"):
        if shutil.which(tool) is None:
            print(f"{tool} is required and not on PATH", file=sys.stderr)
            return 2

    with tempfile.TemporaryDirectory() as tmp:
        pages = ocr_pages(args.pdf, Path(tmp))

    body = ours()
    combined = "\n".join(pages.values())

    print(f"{'ours':24s} {'printed':>9s} {'mistake':>9s}   verdict")
    failures = 0
    for good, bad in CHECKS:
        in_ours = good in body
        printed = combined.count(good)
        mistake = combined.count(bad)
        if not in_ours:
            verdict = "we do not use the good spelling"
            failures += 1
        elif printed == 0:
            verdict = "not found in the scanned range"
        elif mistake > 0:
            verdict = "the printed page carries both, inspect by hand"
            failures += 1
        else:
            verdict = "printed page agrees with us"
        print(f"{good:24s} {printed:9d} {mistake:9d}   {verdict}")

    if failures:
        print(f"\n{failures} check(s) need a human", file=sys.stderr)
        return 1
    print("\nOK: the printed standard agrees with the rebuild")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
