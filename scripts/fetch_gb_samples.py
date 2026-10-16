#!/usr/bin/env python3
"""Fetch the GB/T 33190-2016 conformance samples the parser tolerates.

`scripts/fetch_corpus.py` pins whatever the reference Java implementation
happened to accept. That covers the shapes a real writer emits, but it cannot
cover the shapes the standard itself enumerates: those samples exist to probe
one clause of the specification at a time, deliberately including the ones a
producer should not write.

These come from an open-source toolkit mirror of the standard's own test set,
covering GB/T 33190 section 6.1 (the ZIP container) and section 6.2 (the file
structure). They are third-party and not ours, so nothing is committed: the
script downloads the published archive, unpacks the nested container, and
restores the GBK-encoded Chinese names, which the ZIP was written with and
which the toolkit's own listing does not.

The upstream host blocks datacenter clients on its raw path, so the mirror
listed below is used; both serve the same blob.

Usage:
    python3 scripts/fetch_gb_samples.py [--out DIR] [--check] [--parse]

`--check` reports what is missing without downloading, which is what CI runs.
`--parse` opens every sample that is present and compares the set that fails
against `KNOWN_UNPARSED`: a sample that breaks for a new reason is a
regression, and one that starts working means the list is stale.
"""

from __future__ import annotations

import argparse
import io
import sys
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

# The published archive, pinned by blob id so a mirror drift cannot silently
# change what the suite runs against.
MIRROR = "https://raw.atomgit.com"
BLOB = "open-source-toolkit/886653/blobs/5f19f117681ba58457e3969f613580922d3e760d/Desktop.zip"
URL = MIRROR + "/" + BLOB

# The inner archive, and the directory the thirteen documents land in.
INNER = "ofd标准测试文件.zip"

# Why the set is here, one line per document. The names are the standard's own
# clause numbers, so each entry says which requirement it exercises.
SAMPLES: dict[str, str] = {
    # 6.1 The ZIP container: how entries are stored and compressed.
    "6.1.001 Entry全部不压缩.ofd": "every entry stored, none deflated",
    "6.1.002 Entry全部压缩.ofd": "every entry deflated",
    "6.1.003 Entry合理压缩.ofd": "XML deflated, media stored, as recommended",
    "6.1.004 Entry不合理压缩.ofd": "XML stored, media deflated, against the advice",
    "6.1.005 OFD和Document在ZIP流后部.ofd": "the entry point sits late in the central directory",
    # 6.2 The file structure: where files live and how they are named.
    "6.2.001 正常文件结构.ofd": "the recommended layout, the control case",
    "6.2.002 平行结构文件.ofd": "documents stored as siblings rather than nested",
    "6.2.003 使用相对路径.ofd": "DocRoot written with an explicit ./ segment",
    "6.2.004_1 文件目录结构与推荐的不符.ofd": "a layout off the recommended shape",
    "6.2.004_2 路径中含有空格.ofd": "entry names containing spaces",
    "6.2.004_3 路径中含有转义字符.ofd": "entry names containing escaped characters",
    "6.2.005 文件目录结构乱序.ofd": "entries written out of directory order",
    "6.2.006 各级目录下有无用内容.ofd": "stray files that no table refers to",
}

# Samples that do not open yet, and why. Empty on purpose: all thirteen open,
# and CI fails the moment that stops being true. A sample that never opened
# belongs here with a cause, and one that starts opening must leave.
KNOWN_UNPARSED: dict[str, str] = {}

# How `--parse` opens one sample. Built by the caller before this runs.
PARSE_COMMAND = "cargo run --quiet -p steleofd-cli -- info".split()


def _fetch(timeout: float) -> bytes:
    request = urllib.request.Request(URL, headers={"User-Agent": "steleofd-corpus"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def _decode_name(info: zipfile.ZipInfo) -> str:
    """Restore a GBK file name the archive wrote without the UTF-8 flag.

    The producer stored the names as GBK bytes and left the UTF-8 bit clear, so
    the reader decodes them as code page 437. Re-encoding that and decoding as
    GBK recovers the Chinese the clause numbers are written in.
    """
    if info.flag_bits & 0x800:
        return info.filename
    try:
        return info.filename.encode("cp437").decode("gbk")
    except (UnicodeEncodeError, UnicodeDecodeError):
        # A name that is already plain ASCII round-trips as itself.
        return info.filename


def _unpack(outer: bytes, out: Path) -> list[str]:
    """Peel the nested archive and write the OFD documents into `out`."""
    with zipfile.ZipFile(io.BytesIO(outer)) as top:
        inner_bytes = None
        for name in top.namelist():
            if _decode_name(top.getinfo(name)) == INNER or name.endswith(".zip"):
                inner_bytes = top.read(name)
                break
        if inner_bytes is None:
            raise ValueError(f"no inner archive in the downloaded blob (has {top.namelist()})")

    written: list[str] = []
    with zipfile.ZipFile(io.BytesIO(inner_bytes)) as inner:
        for info in inner.infolist():
            if info.is_dir():
                continue
            name = _decode_name(info)
            if not name.lower().endswith(".ofd"):
                continue
            target = out / Path(name).name
            target.write_bytes(inner.read(info))
            written.append(target.name)
    return written


def _sanity(name: str, blob: bytes) -> str | None:
    """Reject anything that is not an OFD package before it reaches a test."""
    if len(blob) < 22:
        return "too short to be a zip"
    if blob[:2] != b"PK":
        return "does not start with a zip signature"
    if b"OFD.xml" not in blob and b"ofd.xml" not in blob:
        return "no OFD.xml entry point in the central directory"
    return None


def _parse(out: Path) -> int:
    """Open each sample that is present, and hold the failures to the list."""
    import subprocess

    present = sorted(p for p in out.glob("*.ofd") if p.is_file())
    if not present:
        print(f"no samples in {out}", file=sys.stderr)
        return 1
    failed: dict[str, str] = {}
    for path in present:
        done = subprocess.run(
            PARSE_COMMAND + [str(path)], capture_output=True, text=True, check=False
        )
        if done.returncode == 0:
            continue
        reason = (done.stderr.strip() or done.stdout.strip()).splitlines()
        failed[path.name] = reason[-1] if reason else f"exit {done.returncode}"
    known = set(KNOWN_UNPARSED)
    actual = set(failed)
    ok = len(present) - len(failed)
    print(f"GB/T 33190 samples: {ok}/{len(present)} opened")
    for name in sorted(actual - known):
        print(f"  REGRESSION {name}: {failed[name]}")
    for name in sorted(known - actual):
        print(f"  STALE {name}: listed as broken but now opens")
    for name in sorted(actual & known):
        print(f"  known {name}: {KNOWN_UNPARSED[name]}")
    if actual != known:
        print("\nthe set of unopened samples changed; update KNOWN_UNPARSED")
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "gb-t-33190",
        help="where the samples land",
    )
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--check", action="store_true", help="report what is missing and stop")
    parser.add_argument("--parse", action="store_true", help="open every sample and compare failures")
    args = parser.parse_args()

    if args.parse:
        return _parse(args.out)

    if args.check:
        missing = [n for n in sorted(SAMPLES) if not (args.out / n).is_file()]
        present = len(SAMPLES) - len(missing)
        print(f"GB/T 33190 samples: {present}/{len(SAMPLES)} present in {args.out}")
        for name in missing:
            print(f"  missing {name}")
        if missing:
            print("run: python3 scripts/fetch_gb_samples.py")
        return 1 if missing else 0

    try:
        outer = _fetch(args.timeout)
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        print(f"FAIL download: {error}", file=sys.stderr)
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    try:
        written = _unpack(outer, args.out)
    except (zipfile.BadZipFile, ValueError) as error:
        print(f"FAIL unpack: {error}", file=sys.stderr)
        return 1

    failures = 0
    for name in sorted(SAMPLES):
        target = args.out / name
        if not target.is_file():
            print(f"FAIL {name}: not present in the downloaded archive", file=sys.stderr)
            failures += 1
            continue
        problem = _sanity(name, target.read_bytes())
        if problem is not None:
            print(f"FAIL {name}: {problem}", file=sys.stderr)
            failures += 1
            continue
        print(f"ok   {name:<40} {target.stat().st_size:>9,} bytes")

    print(f"\n{len(SAMPLES) - failures}/{len(SAMPLES)} unpacked into {args.out} ({len(written)} found)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
