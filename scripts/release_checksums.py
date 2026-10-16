#!/usr/bin/env python3
"""
SteleOFD release checksum tool.

The release workflow ships unsigned, portable builds for three desktop systems.
There are no signing keys, so the trust anchor a downloader has is the checksum:
this tool writes the `SHA256SUMS` manifest the release publishes and verifies a
directory of downloads against one.

The manifest is written in the format `sha256sum` reads, so the check is
`sha256sum -c SHA256SUMS` on any machine, with this script or without it. Files
are listed in sorted path order so two runs over the same bytes produce the same
manifest — a release asset that changes for no reason is a release asset nobody
can diff.

Usage:
    python3 scripts/release_checksums.py generate dist/
    python3 scripts/release_checksums.py verify   dist/
    python3 scripts/release_checksums.py verify   dist/ --sums SHA256SUMS
    python3 scripts/release_checksums.py selftest

Exit code: 0 on success, 1 on failure.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
import tempfile
from pathlib import Path

MANIFEST_NAME = "SHA256SUMS"
CHUNK = 1 << 20


def digest(path: Path) -> str:
    """The SHA-256 of one file, read in chunks so a large binary is fine."""
    sha = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            sha.update(chunk)
    return sha.hexdigest()


def payloads(directory: Path, manifest: str) -> list[Path]:
    """Every file in the directory except the manifest itself, path-sorted."""
    return sorted(
        (p for p in directory.iterdir() if p.is_file() and p.name != manifest),
        key=lambda p: p.name,
    )


def generate(directory: Path, manifest: str) -> int:
    if not directory.is_dir():
        print(f"error: {directory} is not a directory", file=sys.stderr)
        return 1
    files = payloads(directory, manifest)
    if not files:
        print(f"error: {directory} holds no files to checksum", file=sys.stderr)
        return 1
    lines = [f"{digest(path)}  {path.name}" for path in files]
    body = "\n".join(lines) + "\n"
    (directory / manifest).write_text(body, encoding="utf-8")
    print(f"wrote {directory / manifest}")
    for path in files:
        print(f"  {digest(path)}  {path.name}")
    return 0


def parse_manifest(path: Path) -> list[tuple[str, str]]:
    """The `(sha256, name)` pairs a manifest lists, ignoring blank lines."""
    pairs: list[tuple[str, str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            raise ValueError(f"malformed line: {line!r}")
        pairs.append((parts[0], parts[1].lstrip("*")))
    return pairs


def verify(directory: Path, manifest: str) -> int:
    sums = directory / manifest
    if not sums.is_file():
        print(f"error: {sums} does not exist", file=sys.stderr)
        return 1
    pairs = parse_manifest(sums)
    if not pairs:
        print(f"error: {sums} lists no files", file=sys.stderr)
        return 1
    failures = 0
    for expected, name in pairs:
        target = directory / name
        if not target.is_file():
            print(f"MISSING  {name}")
            failures += 1
            continue
        actual = digest(target)
        if actual == expected:
            print(f"OK       {name}")
        else:
            print(f"MISMATCH {name}\n  expected {expected}\n  actual   {actual}")
            failures += 1
    # A file the manifest does not mention is as much a surprise as a bad hash,
    # so it is reported rather than passed over.
    listed = {name for _, name in pairs}
    for path in payloads(directory, manifest):
        if path.name not in listed:
            print(f"UNLISTED {path.name}")
            failures += 1
    if failures:
        print(f"FAILED: {failures} problem(s)")
        return 1
    print(f"OK: {len(pairs)} file(s) match {manifest}")
    return 0


def selftest() -> int:
    """Exercise generate and verify on a throwaway directory. What CI runs."""
    checks: list[tuple[str, bool]] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "a.bin").write_bytes(b"alpha")
        (root / "b.bin").write_bytes(b"beta")

        ok = generate(root, MANIFEST_NAME) == 0
        checks.append(("generate writes a manifest", ok and (root / MANIFEST_NAME).is_file()))
        checks.append(("verify accepts intact files", verify(root, MANIFEST_NAME) == 0))

        # The manifest must be stable: same bytes, same lines.
        first = (root / MANIFEST_NAME).read_text(encoding="utf-8")
        generate(root, MANIFEST_NAME)
        checks.append(
            ("the manifest is deterministic", (root / MANIFEST_NAME).read_text(encoding="utf-8") == first)
        )

        (root / "a.bin").write_bytes(b"tampered")
        checks.append(("verify rejects a tampered file", verify(root, MANIFEST_NAME) == 1))
        (root / "a.bin").write_bytes(b"alpha")

        (root / "extra.bin").write_bytes(b"surprise")
        checks.append(("verify rejects an unlisted file", verify(root, MANIFEST_NAME) == 1))
        (root / "extra.bin").unlink()

        (root / "b.bin").unlink()
        checks.append(("verify reports a missing file", verify(root, MANIFEST_NAME) == 1))
    failed = [name for name, ok in checks if not ok]
    for name, ok in checks:
        print(f"{'PASS' if ok else 'FAIL'}  {name}")
    if failed:
        print(f"FAILED: {len(failed)} check(s)")
        return 1
    print(f"OK: {len(checks)} check(s) passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["generate", "verify", "selftest"])
    parser.add_argument("directory", type=Path, nargs="?")
    parser.add_argument("--sums", default=MANIFEST_NAME)
    args = parser.parse_args()
    if args.mode == "selftest":
        return selftest()
    if args.directory is None:
        print("error: a directory is required", file=sys.stderr)
        return 1
    if args.mode == "generate":
        return generate(args.directory, args.sums)
    return verify(args.directory, args.sums)


if __name__ == "__main__":
    sys.exit(main())
