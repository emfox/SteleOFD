#!/usr/bin/env python3
"""Fetch the GB/T 33190-2016 conformance samples the parser tolerates.

`scripts/fetch_ofdrw.py` pins whatever the reference Java implementation
happened to accept. That covers the shapes a real writer emits, but it cannot
cover the shapes the standard itself enumerates: those samples exist to probe
one clause of the specification at a time, deliberately including the ones a
producer should not write.

These come from an open-source toolkit mirror of the standard's own test set,
covering GB/T 33190 section 6.1 (the ZIP container) and section 6.2 (the file
structure). They are third-party and not ours, so nothing is committed.

They used to be pulled as a nested archive from that mirror, which served the
blob by an id its owner is free to rewrite -- the same defect as a branch name.
`emfox/OFD-fixtures` stores the published archive itself instead, at a commit,
with a SHA-256 in its `manifest.json`; `scripts/fixtures.py` says how both are
held to. Only the archive is stored, not the documents unpacked out of it, so
every byte can be checked against the publisher and nothing has to be trusted
to a reconstruction. That puts the peeling here: `Desktop.zip` holds
`ofd标准测试文件.zip`, which holds the thirteen documents, and both archives
write their Chinese names as GBK without the UTF-8 flag.

Usage:
    python3 scripts/fetch_gb_samples.py [--out DIR] [--check] [--parse]

`--check` reports what is missing or no longer matches its pinned hash, without
downloading anything; that is what CI runs. `--parse` opens every sample that
is present and compares the set that fails against `KNOWN_UNPARSED`: a sample
that breaks for a new reason is a regression, and one that starts working means
the list is stale.
"""

from __future__ import annotations

import argparse
import io
import sys
import zipfile
from pathlib import Path

from fixtures import FixtureError, Fixtures

SET = "gbt33190-samples"

# The published archive, and the archive it contains. Both are ZIPs.
ARCHIVE = "Desktop.zip"
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


def _decode_name(info: zipfile.ZipInfo) -> str:
    """Restore a GBK file name the archive wrote without the UTF-8 flag.

    The publisher stored the names as GBK bytes and left the UTF-8 bit clear,
    so a reader decodes them as code page 437. Re-encoding that and decoding as
    GBK recovers the Chinese the clause numbers are written in.
    """
    if info.flag_bits & 0x800:
        return info.filename
    try:
        return info.filename.encode("cp437").decode("gbk")
    except (UnicodeEncodeError, UnicodeDecodeError):
        # A name that is already plain ASCII round-trips as itself.
        return info.filename


def _documents(archive: bytes) -> dict[str, bytes]:
    """Peel both layers of the published archive down to the OFD documents."""
    try:
        with zipfile.ZipFile(io.BytesIO(archive)) as outer:
            inner = None
            for info in outer.infolist():
                if _decode_name(info) == INNER:
                    inner = outer.read(info)
                    break
            if inner is None:
                raise FixtureError(f"{ARCHIVE} holds no {INNER}")
            with zipfile.ZipFile(io.BytesIO(inner)) as layer:
                return {
                    _decode_name(info): layer.read(info)
                    for info in layer.infolist()
                    if not info.is_dir() and _decode_name(info).lower().endswith(".ofd")
                }
    except (zipfile.BadZipFile, OSError) as error:
        raise FixtureError(f"cannot unpack {ARCHIVE}: {error}") from error


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


def _check(out: Path, timeout: float) -> int:
    """Report what is absent or changed, without writing anything."""
    try:
        pinned = Fixtures(SET, timeout)
    except FixtureError as error:
        print(f"FAIL fixtures: {error}", file=sys.stderr)
        return 1
    try:
        documents = _documents(pinned.read(ARCHIVE))
    except FixtureError as error:
        print(f"FAIL archive: {error}", file=sys.stderr)
        return 1
    missing: list[str] = []
    stale: list[str] = []
    for name in sorted(SAMPLES):
        path = out / name
        if not path.is_file():
            missing.append(name)
        elif path.read_bytes() != documents.get(name):
            stale.append(name)
    present = len(SAMPLES) - len(missing) - len(stale)
    print(f"GB/T 33190 samples: {present}/{len(SAMPLES)} present in {out}")
    for name in missing:
        print(f"  missing {name}")
    for name in stale:
        # The hash lives on the archive, so a document is compared with the
        # archive's copy: same answer, without a second manifest entry to keep.
        print(f"  changed {name}: differs from the pinned archive")
    if missing or stale:
        print("run: python3 scripts/fetch_gb_samples.py")
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "gbt33190-samples",
        help="where the samples land",
    )
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--check", action="store_true", help="report what is missing and stop")
    parser.add_argument("--parse", action="store_true", help="open every sample and compare failures")
    args = parser.parse_args()

    if args.parse:
        return _parse(args.out)

    if args.check:
        return _check(args.out, args.timeout)

    try:
        pinned = Fixtures(SET, args.timeout)
    except FixtureError as error:
        print(f"FAIL fixtures: {error}", file=sys.stderr)
        return 1

    try:
        documents = _documents(pinned.read(ARCHIVE))
    except FixtureError as error:
        print(f"FAIL archive: {error}", file=sys.stderr)
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    failures = 0
    for name in sorted(SAMPLES):
        blob = documents.get(name)
        if blob is None:
            print(f"FAIL {name}: not present in the published archive", file=sys.stderr)
            failures += 1
            continue
        problem = _sanity(name, blob)
        if problem is not None:
            print(f"FAIL {name}: {problem}", file=sys.stderr)
            failures += 1
            continue
        (args.out / name).write_bytes(blob)
        print(f"ok   {name:<40} {len(blob):>9,} bytes")

    print(f"\n{len(SAMPLES) - failures}/{len(SAMPLES)} fetched into {args.out}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
