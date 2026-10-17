#!/usr/bin/env python3
"""Fetch the third-party OFD samples the parser regression suite runs against.

The corpus under `compliance/corpus/` is written by hand, because it exists to
exercise one clause each and its sources are meant to be read. That is the wrong
shape for a regression suite: a hand-written corpus only ever covers what its
author already thought of. These samples come the other way round, from another
implementation's test suite, so they carry whatever that implementation happened
to accept -- malformed paths, odd namespaces, signature envelopes and scanned
images included.

`ofdrw` is the reference Java implementation of the format and is Apache-2.0,
which makes its test material the one corpus worth taking rather than a
corpus worth asking permission for twice. Nothing is committed here: the
samples are large, they are not ours, and a vendored copy would rot against
upstream.

They are not fetched from `ofdrw` either. Upstream serves them from a `master`
branch, and a branch name does not pin bytes: the samples would change under
the suite whenever someone re-captured one, and the failure would look like a
parser regression. They are copied into `emfox/OFD-fixtures` instead, at a
commit, with a SHA-256 per file in that repository's `manifest.json`. See
`scripts/fixtures.py` for how fetching keeps to both; a sample that differs
from the one the suite was written against is refused rather than silently
tested.

Usage:
    python3 scripts/fetch_corpus.py [--out DIR] [--only NAME] [--check] [--parse]

`--check` reports which samples are missing or no longer match their pinned
hash, without downloading anything; that is what CI runs to tell a developer
what to fetch before a local run.

`--parse` opens every sample that is present and compares the set that fails
against `KNOWN_UNPARSED` below. It fails on either side of a mismatch: a sample
that breaks for a new reason is a regression, and one that starts working means
the list is stale and should be shortened.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from fixtures import FixtureError, Fixtures

SET = "ofdrw"

# Why each sample is here. A sample nobody can explain is a sample nobody
# maintains, and the reason a regression appears is almost never "the parser
# broke" but "the parser never promised this and now must". The value is where
# the file lives inside `emfox/OFD-fixtures`; the hash it must have lives in
# that repository's manifest.json, so there is one copy of the truth.
SAMPLES: dict[str, str] = {
    # Container and structure.
    "test-ofda.ofd": "ofdrw/test-ofda.ofd",
    "helloworld.ofd": "ofdrw/helloworld.ofd",
    "1.ofd": "ofdrw/1.ofd",
    "999.ofd": "ofdrw/999.ofd",
    "h.ofd": "ofdrw/h.ofd",
    "n.ofd": "ofdrw/n.ofd",
    "y.ofd": "ofdrw/y.ofd",
    "z.ofd": "ofdrw/z.ofd",
    "20240531141733.ofd": "ofdrw/20240531141733.ofd",
    "draw_param_ref.ofd": "ofdrw/draw_param_ref.ofd",
    "V4RideRight.ofd": "ofdrw/V4RideRight.ofd",
    # Images, clipping and transparency.
    "containsJPEG.ofd": "ofdrw/containsJPEG.ofd",
    "testPathClip.ofd": "ofdrw/testPathClip.ofd",
    "testPathFillOpacity.ofd": "ofdrw/testPathFillOpacity.ofd",
    "testImageNotFound.ofd": "ofdrw/testImageNotFound.ofd",
    "testImageOverridePage.ofd": "ofdrw/testImageOverridePage.ofd",
    "zsbk.ofd": "ofdrw/zsbk.ofd",
    # Things the specification does not quite cover.
    "ano.ofd": "ofdrw/ano.ofd",
    "SignScaleError.ofd": "ofdrw/SignScaleError.ofd",
    "signout.ofd": "ofdrw/signout.ofd",
    "pattern类型.ofd": "ofdrw/pattern类型.ofd",
    # A real scanned invoice: the shape a government office actually receives,
    # and the reason JBIG2 is not optional.
    # Names are the producer's, CJK included: a sample renamed on the way in
    # loses the only link back to the document it came from, and fetched
    # samples are exempt from the language policy for exactly that reason.
    "intro-数科.ofd": "ofdrw/intro-数科.ofd",
}


# Samples that do not open yet, and why. A corpus is only useful if a failure in
# it means something, so the list is short, each entry names a cause, and CI
# fails when a sample here starts working: that is the signal to remove it.
# Empty for now: the two that used to sit here both open, one after its clip
# areas were read as a union and one after the drawing parameters a `Res` table
# declares stopped being ignored.
KNOWN_UNPARSED: dict[str, str] = {}

# How `--parse` opens one sample. Built by the caller before this runs.
PARSE_COMMAND = "cargo run --quiet -p steleofd-cli -- info".split()


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
        # One line of the error, so the report names a cause rather than a code.
        reason = (done.stderr.strip() or done.stdout.strip()).splitlines()
        failed[path.name] = reason[-1] if reason else f"exit {done.returncode}"
    known = set(KNOWN_UNPARSED)
    actual = set(failed)
    ok = len(present) - len(failed)
    print(f"third-party corpus: {ok}/{len(present)} opened")
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


def _check(wanted: list[str], out: Path, timeout: float) -> int:
    """Report what is absent or changed, without writing anything."""
    try:
        pinned = Fixtures(SET, timeout)
    except FixtureError as error:
        print(f"FAIL fixtures: {error}", file=sys.stderr)
        return 1
    missing: list[str] = []
    changed: list[str] = []
    for name in wanted:
        path = out / name
        if not path.is_file():
            missing.append(name)
            continue
        problem = pinned.verify(name, path.read_bytes())
        if problem is not None:
            changed.append(f"{name}: {problem}")
    present = len(wanted) - len(missing) - len(changed)
    print(f"third-party corpus: {present}/{len(wanted)} present and pinned in {out}")
    for name in missing:
        print(f"  missing {name}")
    for line in changed:
        print(f"  changed {line}")
    if missing or changed:
        print("run: python3 scripts/fetch_corpus.py")
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "ofdrw",
        help="where the samples land",
    )
    parser.add_argument("--only", action="append", help="fetch just these samples")
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--check", action="store_true", help="report what is missing and stop")
    parser.add_argument("--parse", action="store_true", help="open every sample and compare failures")
    args = parser.parse_args()

    wanted = args.only or sorted(SAMPLES)

    if args.parse:
        return _parse(args.out)

    if args.check:
        return _check(wanted, args.out, args.timeout)

    try:
        pinned = Fixtures(SET, args.timeout)
    except FixtureError as error:
        print(f"FAIL fixtures: {error}", file=sys.stderr)
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    failures = 0
    for name in wanted:
        if name not in SAMPLES:
            print(f"unknown sample {name}", file=sys.stderr)
            failures += 1
            continue
        try:
            blob = pinned.read(name)
        except FixtureError as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            failures += 1
            continue
        problem = _sanity(name, blob)
        if problem is not None:
            print(f"FAIL {name}: {problem}", file=sys.stderr)
            failures += 1
            continue
        (args.out / name).write_bytes(blob)
        print(f"ok   {name:<28} {len(blob):>9,} bytes")

    print(f"\n{len(wanted) - failures}/{len(wanted)} fetched into {args.out}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())