#!/usr/bin/env python3
"""Pack the fixture source directories into `.ofd` containers.

`compliance/fixtures/<name>/` holds the XML of an OFD package side by side, flat,
because that is the shape a person reads and edits. The archive nests them the
way the document's own `BaseLoc` references expect, and which entry goes where
is declared by a `layout.txt` next to the sources: one `archive/path.xml  source.xml`
pair per line, `#` for comments.

The archive is a build artefact but it is committed, because the reader tests
read it straight from the tree. `--check` regenerates it in memory and compares,
which is what CI runs so that an edit to the sources cannot leave the committed
archive stale.

Usage:
    python3 scripts/pack_fixtures.py [--check] [name...]
"""

from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path

FIXTURE_DIR = Path(__file__).resolve().parent.parent / "compliance" / "fixtures"

# A fixed timestamp keeps the bytes stable across runs, so `--check` compares
# content rather than build time and the archive does not churn in git.
STAMP = (1980, 1, 1, 0, 0, 0)


class _BytesWriter:
    """A minimal writable stream so packing needs no temp file."""

    def __init__(self, buffer: bytearray) -> None:
        self._buffer = buffer

    def write(self, data: bytes) -> int:
        self._buffer.extend(data)
        return len(data)

    def flush(self) -> None:
        return None

    def seekable(self) -> bool:
        return False

    def tell(self) -> int:
        return len(self._buffer)


def read_layout(source: Path) -> list[tuple[str, str]]:
    """Parse `<source>/layout.txt` into (archive path, source file) pairs."""
    layout_file = source / "layout.txt"
    if not layout_file.is_file():
        raise SystemExit(f"{layout_file} is missing; it names the archive layout")
    pairs: list[tuple[str, str]] = []
    for number, raw in enumerate(layout_file.read_text().splitlines(), start=1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) != 2:
            raise SystemExit(f"{layout_file}:{number}: expected '<archive> <source>'")
        archive, origin = parts
        if not (source / origin).is_file():
            raise SystemExit(f"{layout_file}:{number}: no such file: {origin}")
        pairs.append((archive, origin))
    if not pairs:
        raise SystemExit(f"{layout_file} names no entries")
    return pairs


def pack(source: Path) -> bytes:
    """Build the archive bytes for one fixture directory."""
    buffer = bytearray()
    layout = read_layout(source)
    with zipfile.ZipFile(_BytesWriter(buffer), "w", zipfile.ZIP_DEFLATED) as archive:
        for archive_path, origin in sorted(layout):
            info = zipfile.ZipInfo(archive_path, date_time=STAMP)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            # ZipInfo defaults this to the host, so packing on Windows writes a
            # different central directory than packing on Unix. Pin it to the
            # value the committed archives carry and `--check` holds anywhere.
            info.create_system = 3
            archive.writestr(info, (source / origin).read_bytes())
    return bytes(buffer)


def sources(names: list[str]) -> list[Path]:
    """Fixture directories to pack, in a stable order."""
    if names:
        chosen = [FIXTURE_DIR / name for name in names]
    else:
        chosen = sorted(p for p in FIXTURE_DIR.iterdir() if p.is_dir())
    missing = [p for p in chosen if not p.is_dir()]
    if missing:
        raise SystemExit("not a fixture directory: " + ", ".join(str(p) for p in missing))
    return chosen


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("names", nargs="*", help="fixture directories; every one when omitted")
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when a committed archive is not what the sources produce",
    )
    args = parser.parse_args()

    failures = 0
    for source in sources(args.names):
        target = source.with_suffix(".ofd")
        built = pack(source)
        if args.check:
            if not target.exists():
                print(f"MISSING {target}: run scripts/pack_fixtures.py", file=sys.stderr)
                failures += 1
            elif target.read_bytes() != built:
                print(
                    f"STALE   {target}: sources changed; run scripts/pack_fixtures.py",
                    file=sys.stderr,
                )
                failures += 1
            else:
                print(f"OK      {target}")
        else:
            target.write_bytes(built)
            print(f"wrote   {target} ({len(built)} bytes)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
