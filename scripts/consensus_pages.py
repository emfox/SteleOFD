#!/usr/bin/env python3
"""Report the sample pages ofdrw and easyofd render alike.

Two independent implementations agreeing on a page settles that page's
appearance, so agreement is the evidence a rendering defect is ours rather
than a reading of the spec. This script renders every sample with both and
reports which pages they match on, which is how a fix is shown to bring us in
line with what two other engines already agreed about.

Neither library writes a page at the size its document declares, so each page
is first fitted to A4 by its ink before anything is measured. Agreement is
then judged on where ink falls, allowing a stroke's width of slack so that a
different font or letter spacing does not read as a disagreement, while a
word that is missing cannot be hidden.

Rendering needs both toolchains:

    java -jar ofdrw-png.jar <file.ofd> <outDir> <ppm>
    python3 easyofd_export.py <file.ofd> <outDir> <dpi>

Run with `--compare-only` to re-measure pages already rendered.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

REPO = Path(__file__).resolve().parent.parent
SAMPLES = REPO / "tests" / "fixtures" / "gbt33190-samples"
# The work directory keeps the clause number alone, so a sample of the
# standard and one of ofdrw with the same name cannot collide.
PREFIX = "gb_"

# The comparison grid: A4 at 96 dpi, the size both tools aim for.
GRID = (794, 1123)
# A pixel is ink when it is darker than this; white paper is 255.
INK_LEVEL = 200
# Blurring by this much of the grid leaves structure and drops texture.
BLUR = 3.0
# How far apart two strokes may sit and still be the same stroke.
NEAR = 2
# A page is similar when the ink is in the same places and nearly all of each
# side's ink has the other's near it. Weaker than this is a real difference.
STRUCTURE_MIN = 0.85
COVERAGE_MIN = 0.80

# ofdrw takes pixels per millimetre; 3.78 is 96 dpi to the nearest hundredth.
PPM = "3.78"
DPI = "96"


def render_ofdrw(src: Path, out: Path, jar: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["java", "-Djava.awt.headless=true", "-jar", str(jar), str(src), str(out), PPM],
        check=True, capture_output=True, timeout=1200,
    )
    return len(list(out.glob("*.png")))


def render_easyofd(src: Path, out: Path, script: Path, python: str) -> int:
    out.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [python, str(script), str(src), str(out), DPI],
        check=True, capture_output=True, timeout=1200,
    )
    return len(list(out.glob("*.png")))


def ink_bbox(mask: np.ndarray) -> tuple[int, int, int, int] | None:
    if not mask.any():
        return None
    ys, xs = np.where(mask)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def to_grid(path: Path) -> tuple[np.ndarray, np.ndarray] | None:
    """Fit a page's ink onto the grid, returning its mask and a soft map."""
    rgb = np.asarray(Image.open(path).convert("RGB"), dtype=np.uint8)
    mask = rgb.min(axis=2) < INK_LEVEL
    box = ink_bbox(mask)
    if box is None:
        return None
    x0, y0, x1, y1 = box
    ink = Image.fromarray((mask[y0:y1, x0:x1] * 255).astype(np.uint8))
    ink = ink.resize(GRID, Image.LANCZOS)
    soft = ink.filter(ImageFilter.GaussianBlur(BLUR))
    return (np.asarray(ink, dtype=np.float32) > 127,
            np.asarray(soft, dtype=np.float32) / 255.0)


def grown(mask: np.ndarray, radius: int) -> np.ndarray:
    """The mask dilated by `radius`, so nearby ink counts as the same ink."""
    img = Image.fromarray((mask * 255).astype(np.uint8))
    k = radius * 2 + 1
    return np.asarray(img.filter(ImageFilter.MaxFilter(k)), dtype=np.float32) > 127


def stats(ref: tuple[np.ndarray, np.ndarray], oth: tuple[np.ndarray, np.ndarray]) -> dict:
    mref, sref = ref
    moth, soth = oth
    a = sref.ravel() - sref.mean()
    b = soth.ravel() - soth.mean()
    denom = float(np.sqrt((a * a).sum() * (b * b).sum()))
    corr = float((a * b).sum() / denom) if denom else 1.0
    hit = float((mref & grown(moth, NEAR)).sum()) / float(mref.sum()) if mref.sum() else 1.0
    back = float((moth & grown(mref, NEAR)).sum()) / float(moth.sum()) if moth.sum() else 1.0
    return {
        "structure": round(corr, 4),
        "coverage": round(min(hit, back), 4),
        "ink_ref": round(float(mref.mean()), 4),
        "ink_other": round(float(moth.mean()), 4),
    }


def judge(s: dict) -> str:
    if s["structure"] >= STRUCTURE_MIN and s["coverage"] >= COVERAGE_MIN:
        return "same"
    if s["structure"] >= 0.85 and s["coverage"] >= 0.70:
        return "close"
    return "different"


def code_of(stem: str) -> str:
    """The clause a sample exercises: 6.2.004_1 plus its description is 6.2.004-1."""
    return re.sub(r"^gb_", "", stem).split(" ")[0].replace("_", "-")


def compare(work: Path) -> list[dict]:
    rows = []
    for src in sorted(SAMPLES.glob("*.ofd")):
        stem = PREFIX + src.stem
        pa = sorted((work / "ofdrw" / stem).glob("*.png"),
                    key=lambda p: int(p.stem) if p.stem.isdigit() else 0)
        pb = sorted((work / "easyofd" / stem).glob("*.png"),
                    key=lambda p: int(p.stem) if p.stem.isdigit() else 0)
        row = {"sample": src.name, "code": code_of(stem),
               "pages_ofdrw": len(pa), "pages_easyofd": len(pb)}
        if not pa or not pb:
            rows.append({**row, "status": "no-output", "per_page": []})
            continue
        per = []
        for i in range(min(len(pa), len(pb))):
            ra, rb = to_grid(pa[i]), to_grid(pb[i])
            if ra is None or rb is None:
                per.append({"page": i, "verdict": "one-blank"})
                continue
            s = stats(ra, rb)
            per.append({**s, "page": i, "verdict": judge(s)})
        verdicts = [p["verdict"] for p in per]
        if len(pa) != len(pb):
            status = "page-count"
        elif all(v in ("same", "close") for v in verdicts):
            status = "keep"
        elif sum(v in ("same", "close") for v in verdicts) * 2 >= len(verdicts):
            status = "partial"
        else:
            status = "drop"
        rows.append({**row, "status": status, "per_page": per})
    return rows


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--work", type=Path, default=Path("/tmp/consensus"))
    ap.add_argument("--ofdrw-jar", type=Path)
    ap.add_argument("--easyofd", type=Path, help="the exporter script to call")
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--compare-only", action="store_true")
    args = ap.parse_args()

    if not args.compare_only:
        if not args.ofdrw_jar or not args.easyofd:
            ap.error("rendering needs --ofdrw-jar and --easyofd")
        for src in sorted(SAMPLES.glob("*.ofd")):
            out = args.work
            for who, fn in (("ofdrw", lambda: render_ofdrw(src, out / "ofdrw" / (PREFIX + src.stem), args.ofdrw_jar)),
                            ("easyofd", lambda: render_easyofd(src, out / "easyofd" / (PREFIX + src.stem), args.easyofd, args.python))):
                try:
                    n = fn()
                except (subprocess.SubprocessError, OSError) as e:
                    n = f"failed: {type(e).__name__}"
                print(f"{who:8} {src.name}: {n}", flush=True)

    rows = compare(args.work)
    counts: dict[str, int] = {}
    for r in rows:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
        print(f"{r['status']:10} {r['sample']}", flush=True)
    print("\n" + ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
