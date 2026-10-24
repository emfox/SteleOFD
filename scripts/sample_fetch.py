#!/usr/bin/env python3
"""Fetch, verify and open the third-party samples the parser is tested against.

Two sets of `.ofd` documents are too large and too third-party to commit, and
both are read by the suite: the reference Java implementation's own test
material, and the GB/T 33190 standard's conformance set. What differs between
them is where the documents sit inside the published archive and which clauses
they probe. What does not differ is everything else -- the pinned commit, the
hash that decides whether a download is acceptable, the record of what was
already written, the zip sanity check, and opening every sample and comparing
failures against a known list. That shared part lives here, once.

    python3 scripts/sample_fetch.py [SET ...] [--out DIR] [--check] [--parse]
    python3 scripts/sample_fetch.py --list

With no `SET`, every set is handled. `--check` downloads what is absent and
changes nothing else; `--parse` downloads what is absent, then opens every
sample. A sample already on disk under its own name with the hash the last run
recorded is not fetched again -- see `scripts/fixtures.py` for the record and
for why the archive is pinned to a commit.
"""

from __future__ import annotations

import argparse
import io
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path

from fixtures import (
    COMMIT,
    RECORD,
    FixtureError,
    Fixtures,
    SampleSet,
    configure_proxy,
    decode_zip_name,
    sha256,
    unzip,
)

SCRIPTS = Path(__file__).resolve().parent
SAMPLES = SCRIPTS.parent / "tests" / "fixtures"

# How each set is opened out of its pinned files. Sections are the published
# archive and, for a publisher who ships one, the archive inside it.
OFDRW_HEADER = """\
Files published by the reference Java implementation's test suite.

`ofdrw` is the reference implementation of the format and is Apache-2.0, which
makes its test material worth taking rather than worth asking permission for
twice. A sample set written by one implementation says what that implementation
happened to accept -- malformed paths, odd namespaces, signature envelopes and
scanned images included -- which is the shape a regression suite wants and the
opposite of what a hand-written fixture gives.

Upstream serves these from a `master` branch, and a branch name does not pin
bytes, so they are copied into `emfox/OFD-fixtures` at a commit with a SHA-256
per file. The map below says where each one lives there and why it is here: a
sample nobody can explain is a sample nobody maintains.
"""

GBT_HEADER = """\
The GB/T 33190-2016 conformance samples, one document per clause.

The other set pins whatever the reference implementation accepted. That covers
the shapes a real writer emits, but not the shapes the standard itself
enumerates: these exist to probe one clause at a time, deliberately including
the ones a producer should not write. They are the standard's own
section 6.1 (the ZIP container) and 6.2 (the file structure), published by an
open-source toolkit mirror.

They used to arrive as a nested archive from that mirror, served by an id its
owner is free to rewrite -- the same defect as a branch name. `emfox/OFD-fixtures`
stores the published archive itself, at a commit, with a SHA-256 in its
`manifest.json`. Only that archive is stored, not the documents unpacked out of
it, so every byte can be checked against the publisher; the peeling happens
here. Both layers write their Chinese names as GBK without the UTF-8 flag, which
is what `decode_zip_name` restores.
"""

# Paragraphs that describe any set, kept in one place so they cannot come to
# disagree about the parts that are the same everywhere.
COMMON = """\
Nothing is committed here. The samples are large, they are not ours, and a
vendored copy would rot against upstream, so they stay under `tests/fixtures/`,
which git ignores. What is committed is this file: the pin list is the source
of truth for what a fresh checkout has, and `.fetched.json` in the sample
directory records the hash of what a working copy already holds. A file that is
present under the same name with the same hash is not downloaded again.
"""

OFDRW_COUNT = 22


@dataclass(frozen=True)
class Section:
    """One archive to open, and how to reach it."""

    # Path inside the fixtures commit.
    path: str
    # The manifest records this file, so its SHA-256 is checked before its bytes
    # are used. False for a set that ships a directory or an unlisted archive,
    # where presence in the commit is the only claim it makes.
    pinned: bool = True
    # One file per sample. False when the section is an archive of samples,
    # which `unzip` opens; a pinned directory needs neither.
    archive: bool = True
    # The archive inside this one, when the publisher nested them.
    inner: str | None = None


@dataclass(frozen=True)
class Set:
    """A pinned set of samples: where it comes from, how it opens."""

    name: str
    # One line saying what the set is, for `--list`.
    about: str
    # The docstring this script carries for the set, spelled out above.
    header: str
    # Archive path -> why the document is here. The value is where the file
    # lives inside the fixtures repository, for a loose file, or the clause a
    # document exercises, for a set that ships one archive of documents.
    samples: dict[str, str]
    # Samples that do not open yet, and the cause. CI fails when one starts
    # opening: that is the signal to shorten the list.
    unparsed: dict[str, str]
    sections: tuple[Section, ...]

    def dir(self, root: Path) -> Path:
        return root / self.name


# The sets, keyed by the directory each lands in. Names and reasons are the
# producers' own, CJK included -- a sample renamed on the way in loses the only
# link back to the document it came from.
OFDRW = Set(
    name="ofdrw",
    about="the reference Java implementation's own test material",
    header=OFDRW_HEADER,
    samples={
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
        # A real scanned invoice: the shape a government office actually
        # receives, and the reason JBIG2 is not optional.
        "intro-数科.ofd": "ofdrw/intro-数科.ofd",
    },
    # Empty for now: the two that used to sit here both open, one after its
    # clip areas were read as a union and one after the drawing parameters a
    # `Res` table declares stopped being ignored.
    unparsed={},
    # Loose files in the set's directory, each with its own hash in the manifest.
    sections=(Section(path="ofdrw", pinned=False, archive=False),),
)

# The published archive, and the archive it contains. Both are ZIPs, and the
# outer one sits in the set's directory in the fixtures commit.
GBT_DIR = "gbt33190-samples"
GBT_ARCHIVE = "Desktop.zip"
GBT_INNER = "ofd标准测试文件.zip"

GBT33190 = Set(
    name="gbt33190-samples",
    about="the GB/T 33190 conformance set, one document per clause",
    header=GBT_HEADER,
    samples={
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
    },
    # Empty on purpose: all thirteen open, and CI fails the moment that stops
    # being true. A sample that never opened belongs here with a cause, and one
    # that starts opening must leave.
    unparsed={},
    sections=(
        Section(path=f"{GBT_DIR}/{GBT_ARCHIVE}", pinned=False, inner=GBT_INNER),
    ),
)

SETS: dict[str, Set] = {one.name: one for one in (OFDRW, GBT33190)}

# How `--parse` opens one sample. Built by the caller before this runs.
PARSE_COMMAND = "cargo run --quiet -p sofd-cli -- info".split()


def sanity(name: str, blob: bytes) -> str | None:
    """Reject anything that is not an OFD package before it reaches a test."""
    if len(blob) < 22:
        return "too short to be a zip"
    if blob[:2] != b"PK":
        return "does not start with a zip signature"
    if b"OFD.xml" not in blob and b"ofd.xml" not in blob:
        return "no OFD.xml entry point in the central directory"
    return None


class DiskSet(SampleSet):
    """A set as it lands on disk: which names, and how they are unpacked."""

    def __init__(self, spec: Set, out: Path, timeout: float = 180.0,
                 pinned: Fixtures | None = None) -> None:
        super().__init__(spec.name, out, timeout, pinned)
        self.spec = spec

    def landing(self) -> list[str]:
        return sorted(self.spec.samples)

    def documents(self) -> dict[str, bytes]:
        """Every document, peeled out of the pinned archives.

        A directory of loose files yields all of them at once; an archive is
        sliced once per file asked for, which is why a caller that only needs
        one sample does not go through here.
        """
        by_name: dict[str, bytes] = {}
        archive = self.pinned()
        for section in self.spec.sections:
            if not section.archive:
                # A directory of loose files, one manifest hash each.
                by_name.update({
                    name: archive.read(name)
                    for name in self.spec.samples if name in archive.files
                })
                continue
            if section.pinned:
                by_name.update(archive.unzip(section.path))
                continue
            # Nothing in the manifest describes this one, so its presence in the
            # commit is the only claim it makes. One nested layer is what a
            # publisher ships; a third would need a section of its own.
            if section.inner is None:
                by_name.update(unzip(archive.unpack(section.path), section.path))
                continue
            outer = unzip(archive.unpack(section.path), section.path)
            if section.inner not in outer:
                raise FixtureError(f"{section.path} holds no {section.inner}")
            by_name.update(unzip(outer[section.inner], section.inner))
        return {name: by_name[name] for name in self.spec.samples if name in by_name}

    def expected(self) -> dict[str, bytes]:
        """The kept documents this set's pinned archives can supply."""
        return self.documents()


def check(spec: Set, out: Path, timeout: float) -> int:
    """Report each sample's standing, fetching only what cannot be resolved here.

    A sample on disk whose hash is the one recorded is accepted without a
    download, so the usual case -- a working copy that already has the samples
    -- costs no network at all and works offline. Anything else needs the pinned
    archive: to prove a name the record does not know, or to replace a file
    whose bytes changed.
    """
    disk = DiskSet(spec, out, timeout)
    names = disk.landing()
    have = disk.local()
    absent = [name for name in names if name not in have]

    if absent:
        try:
            fetched = disk.missing()
            if fetched:
                disk.write(fetched)
                print(f"  fetched {len(fetched)} sample(s) that were absent or had changed")
            have = disk.local()
            absent = [name for name in names if name not in have]
            # A name the set keeps that no archive supplies is a broken pin,
            # and saying so is the point of this check rather than a download.
            supplied = disk.documents()
            if len(supplied) < len(names):
                for name in names:
                    if name not in supplied:
                        print(f"  FAIL {name}: not in the pinned archive", file=sys.stderr)
                return 1
        except FixtureError as error:
            print(f"FAIL fixtures: {error}", file=sys.stderr)
            for name in absent:
                print(f"  unresolved {name}")
            return 1

    print(f"{spec.name}: {len(names) - len(absent)}/{len(names)} present in {out}")
    for name in absent:
        print(f"  missing {name}")
    if absent:
        print(f"run: python3 scripts/sample_fetch.py {spec.name}")
        return 1
    return 0


def parse(spec: Set, out: Path) -> int:
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
    known = set(spec.unparsed)
    actual = set(failed)
    ok = len(present) - len(failed)
    print(f"{spec.name}: {ok}/{len(present)} opened")
    for name in sorted(actual - known):
        print(f"  REGRESSION {name}: {failed[name]}")
    for name in sorted(known - actual):
        print(f"  STALE {name}: listed as broken but now opens")
    for name in sorted(actual & known):
        print(f"  known {name}: {spec.unparsed[name]}")
    if actual != known:
        print(f"\nthe set of unopened samples changed in {spec.name}; update its `unparsed`")
        return 1
    return 0


def fetch(spec: Set, out: Path, timeout: float) -> int:
    """Write every kept sample that is not already here, and record the hashes."""
    disk = DiskSet(spec, out, timeout)
    try:
        wanted = disk.expected()
    except FixtureError as error:
        print(f"FAIL fixtures: {error}", file=sys.stderr)
        return 1
    failures = 0
    written: dict[str, bytes] = {}
    for name in disk.landing():
        blob = wanted.get(name)
        if blob is None:
            print(f"FAIL {name}: not in the pinned archive", file=sys.stderr)
            failures += 1
            continue
        problem = sanity(name, blob)
        if problem is not None:
            print(f"FAIL {name}: {problem}", file=sys.stderr)
            failures += 1
            continue
        written[name] = blob
    disk.write(written)
    present = disk.local()
    for name in disk.landing():
        if name not in written:
            continue
        if name in present:
            print(f"ok   {name:<44} {len(written[name]):>9,} bytes")
        else:
            print(f"??   {name:<44} written but does not read back", file=sys.stderr)
            failures += 1
    kept = len(disk.landing())
    print(f"\n{kept - failures}/{kept} present in {out}")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("sets", nargs="*", metavar="SET",
                        help="which sets to handle, one of " + ", ".join(SETS))
    parser.add_argument("--out", type=Path, help="the directory the samples land in")
    parser.add_argument("--timeout", type=float, default=180.0,
                        help="seconds to wait on the pinned archive")
    parser.add_argument("--proxy", metavar="URL",
                        help="HTTP proxy for the download, overriding the environment")
    parser.add_argument("--check", action="store_true",
                        help="fetch what is absent, then report each sample's standing")
    parser.add_argument("--parse", action="store_true",
                        help="fetch what is absent, then open every sample")
    parser.add_argument("--list", action="store_true", help="the sets and the pin commit")
    args = parser.parse_args()

    if args.list:
        print(f"pinned to {COMMIT}, fetching {RECORD} into each set's directory")
        for spec in SETS.values():
            print(f"  {spec.name:<20} {len(spec.samples):>3} samples  {spec.about}")
        return 0

    wanted = args.sets or list(SETS)
    unknown = [name for name in wanted if name not in SETS]
    if unknown:
        # Checked here rather than by `choices`, which an empty positional list
        # trips over before the default can be applied.
        parser.error(f"unknown set {unknown[0]}; --list names the sets")

    if configure_proxy(args.proxy or None):
        print(f"fetching through {args.proxy or 'the proxy the environment names'}")

    if args.parse and args.check:
        parser.error("choose --check or --parse, not both")

    failures = 0
    for name in wanted:
        spec = SETS[name]
        out = args.out or spec.dir(SAMPLES)
        if args.parse:
            # Open what is here, but a missing sample would silently shrink the
            # set the comparison is about, so it is fetched first.
            failures += check(spec, out, args.timeout)
            failures += parse(spec, out)
        elif args.check:
            failures += check(spec, out, args.timeout)
        else:
            failures += fetch(spec, out, args.timeout)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
