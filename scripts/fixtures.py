#!/usr/bin/env python3
"""Fetch sample documents from the pinned fixtures repository.

`scripts/sample_fetch.py` reads sample `.ofd` files that are too large and too
third-party to commit here. This module is the one place that decides how they
arrive, for every set, so the sets cannot drift apart on the part that matters:
what a fetched file is allowed to be.

Two properties are load-bearing.

**The reference is a commit.** A branch and a tag can both be rewritten; a
commit cannot. The samples a developer fetches are then the samples the tests
were written against, and when they have to change, the change lands as a diff
to `COMMIT` and `manifest.json` together, in one reviewable commit.

**The hash decides.** `manifest.json` records a SHA-256 for every file. A
download that does not match is refused, not tested: a suite that silently runs
against a different sample is worse than one that refuses to run, because the
first reports a failure somewhere else entirely.

The whole repository comes down as one archive rather than a request per file.
GitHub's codeload service serves it in a single response, which is seconds
where a per-file fetch is minutes, and it keeps the `manifest.json` that
governs the files in the same atomic transfer as the files themselves.

Reaching codeload is the network's problem, not this module's: it uses whatever
proxy the environment configures, like every other HTTP client here, so there
is one place to fix when GitHub is slow rather than one per script.

The variable is `https_proxy`, and it has to be that one. `urllib` reads only
`http_proxy`, `https_proxy` and `no_proxy`; `ALL_PROXY` -- which curl and git
honour -- is invisible to it, so a shell that reaches GitHub through a
`ALL_PROXY=socks5h://...` and nothing else would hang on a direct connection
until the timeout expired. `configure_proxy` closes that gap from both ends: it
is called on import, and `scripts/sample_fetch.py` calls it again for a
`--proxy` given on the command line.

**A file already on disk can answer for itself.** `SampleSet` records a SHA-256
per kept name once a set has been unpacked and written out, and on the next run
a file whose name and hash both match is not fetched again. A fresh checkout
has no record and pays the one download; a working copy that already has the
samples pays an archive-sized verification and no network at all.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import tarfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

# The fixtures repository, pinned by commit: see the module docstring.
FIXTURES = "emfox/OFD-fixtures"
COMMIT = "1ad683e"
ARCHIVE = "https://codeload.github.com/" + FIXTURES + "/tar.gz/" + COMMIT
USER_AGENT = "sofd-fixtures"


class FixtureError(Exception):
    """A fixture could not be fetched or did not match its pinned hash."""


def configure_proxy(url: str | None = None) -> tuple[str, ...]:
    """Point `urllib` at a proxy, and say which variables were set.

    `url` names one explicitly; without it the environment is read and an
    `ALL_PROXY` is copied into the two variables `urllib` does read. A sock
    scheme is left out: `urllib` speaks HTTP proxies, and a `socks5h://` value
    handed over unchanged fails at request time instead of here.
    """
    if url is None:
        url = os.environ.get("ALL_PROXY") or os.environ.get("all_proxy") or ""
        if url.lower().startswith("socks"):
            return ()
    if not url:
        return ()
    set_now = []
    for name in ("https_proxy", "http_proxy"):
        os.environ[name] = url
        set_now.append(name)
    return tuple(set_now)


configure_proxy()


def _fetch_archive(timeout: float) -> bytes:
    """Download the pinned archive."""
    request = urllib.request.Request(ARCHIVE, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.read()
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        raise FixtureError(f"cannot reach {ARCHIVE}: {error}") from error


def _members(archive: bytes) -> dict[str, tarfile.TarInfo]:
    """Index the archive by path relative to its single top directory."""
    try:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
            names = tar.getnames()
            if not names:
                raise FixtureError("archive is empty")
            root = names[0].split("/")[0] + "/"
            return {m.name[len(root) :]: m for m in tar.getmembers() if m.isfile()}
    except (tarfile.TarError, OSError) as error:
        raise FixtureError(f"archive is unreadable: {error}") from error


def _member(archive: bytes, index: dict[str, tarfile.TarInfo], path: str) -> bytes:
    member = index.get(path)
    if member is None:
        raise FixtureError(f"not in the fixtures commit: {path}")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
        handle = tar.extractfile(member)
        if handle is None:
            raise FixtureError(f"not a regular file: {path}")
        return handle.read()


class Fixtures:
    """The samples of one set, held to the hashes their manifest records."""

    def __init__(self, set_name: str, timeout: float = 180.0) -> None:
        self.set_name = set_name
        archive = _fetch_archive(timeout)
        self._index = _members(archive)
        self._archive = archive
        # Sliced once: decompressing the whole archive per sample would make a
        # large set cost its own size in work for every file in it.
        self._members: dict[str, bytes] = {}
        try:
            document = json.loads(_member(self._archive, self._index, "manifest.json"))
            self._files = document["sets"][set_name]["files"]
        except (KeyError, TypeError, ValueError) as error:
            raise FixtureError(f"manifest has no set {set_name!r}: {error}") from error
        # A manifest entry with no file is caught here rather than at the first
        # download, so a broken fixtures commit fails before anything is written.
        absent = [name for name in self._files if f"{set_name}/{name}" not in self._index]
        if absent:
            raise FixtureError(f"manifest lists {absent[0]!r}, which the commit does not contain")

    @property
    def files(self) -> dict[str, object]:
        """The manifest's entries for this set, by file name."""
        return dict(self._files)

    def names(self) -> list[str]:
        """Every sample in the set, sorted."""
        return sorted(self._files)

    def pinned(self, name: str) -> str:
        """The hash a sample must have."""
        try:
            return str(self._files[name]["sha256"])
        except KeyError as error:
            raise FixtureError(f"not pinned by the manifest: {name}") from error

    def unpack(self, path: str) -> bytes:
        """The bytes of any file in the commit, by its path inside it.

        `read` answers for a file the manifest pins by hash; this answers for
        one it only lists, and holds nothing to a digest.
        """
        if path not in self._members:
            self._members[path] = _member(self._archive, self._index, path)
        return self._members[path]

    def read(self, name: str) -> bytes:
        """Return a sample's bytes, or refuse if they are not the pinned ones."""
        expected = self.pinned(name)
        blob = self.unpack(f"{self.set_name}/{name}")
        actual = hashlib.sha256(blob).hexdigest()
        if actual != expected:
            size = self._files[name].get("bytes", "unknown")
            raise FixtureError(
                f"{name}: sha256 {actual[:12]} is not the pinned {expected[:12]} "
                f"({len(blob)} of {size} bytes)"
            )
        return blob

    def verify(self, name: str, blob: bytes) -> str | None:
        """Check a file already on disk against the manifest, without fetching."""
        expected = self.pinned(name)
        actual = hashlib.sha256(blob).hexdigest()
        if actual == expected:
            return None
        size = self._files[name].get("bytes", "unknown")
        return f"sha256 {actual[:12]} is not the pinned {expected[:12]} ({len(blob)} of {size} bytes)"

    def unzip(self, name: str) -> dict[str, bytes]:
        """Expected samples held inside a pinned archive, read out of it.

        A set may publish its samples as one archive rather than as loose
        files, because that is how the publisher distributes them; see
        `unwrap` for the second layer some of those archives carry.
        """
        blob = self.read(name)
        return unzip(blob, name)


def unzip(blob: bytes, name: str) -> dict[str, bytes]:
    """The members of a zip `name`, or a `FixtureError` naming it."""
    try:
        with zipfile.ZipFile(io.BytesIO(blob)) as archive:
            return {
                decode_zip_name(info): archive.read(info)
                for info in archive.infolist()
                if not info.is_dir()
            }
    except (zipfile.BadZipFile, OSError) as error:
        raise FixtureError(f"{name}: not a readable zip: {error}") from error


def decode_zip_name(info: zipfile.ZipInfo) -> str:
    """Restore a GBK file name an archive wrote without the UTF-8 flag.

    A publisher that stored GBK bytes and left the UTF-8 bit clear is read as
    code page 437; re-encoding that and decoding as GBK recovers the name.
    """
    if info.flag_bits & 0x800:
        return info.filename
    try:
        return info.filename.encode("cp437").decode("gbk")
    except (UnicodeEncodeError, UnicodeDecodeError):
        return info.filename


def sha256(blob: bytes) -> str:
    """The digest a record and a manifest both spell."""
    return hashlib.sha256(blob).hexdigest()


class SampleSet:
    """One set's samples, as named files on disk, held to a recorded hash.

    The record is what makes a second run cheap: a file that is present under
    the same name with the same digest does not have to be fetched again. It is
    also what makes the check meaningful without a network, because a tampered
    file no longer matches what was written.

    A kept name may be a member of an archive in the set -- a zip of zips is
    how one publisher ships -- so `unpack` says how to open the set's contents
    and `kept` says which of their names land on disk.
    """

    def __init__(self, set_name: str, out: Path, timeout: float = 180.0,
                 pinned: Fixtures | None = None) -> None:
        self.set_name = set_name
        self.out = out
        self.timeout = timeout
        self.record_file = out / RECORD
        # A caller that already opened the archive passes it in, so a set is
        # never downloaded twice for one command.
        self._pinned = pinned

    # -- the recorded side -------------------------------------------------

    def recorded(self) -> dict[str, str]:
        """The hashes of what was written here, without touching the network."""
        try:
            document = json.loads(self.record_file.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return {}
        files = document.get("sets", {}).get(self.set_name, {}).get("files", {})
        if not isinstance(files, dict):
            return {}
        return {str(name): str(digest) for name, digest in files.items()}

    def remember(self, files: dict[str, str]) -> None:
        """Record the hashes just written, merging into whatever is there."""
        try:
            document = json.loads(self.record_file.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            document = {"commit": COMMIT, "sets": {}}
        sets = document.setdefault("sets", {})
        sets[self.set_name] = {"files": dict(sorted(files.items()))}
        # The record is not committed, so its formatting is not a review surface.
        self.out.mkdir(parents=True, exist_ok=True)
        self.record_file.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    def local(self) -> dict[str, str]:
        """Names present here whose bytes match the record.

        A name that is missing, or whose bytes changed, is absent from the
        answer rather than reported: the caller decides whether that is a file
        to fetch, a file to replace, or a failure.
        """
        have: dict[str, str] = {}
        for name, digest in self.recorded().items():
            path = self.out / name
            if path.is_file() and sha256(path.read_bytes()) == digest:
                have[name] = digest
        return have

    # -- the remote side ---------------------------------------------------

    def pinned(self) -> Fixtures:
        """The pinned archive, fetched on first use."""
        if self._pinned is None:
            self._pinned = Fixtures(self.set_name, self.timeout)
        return self._pinned

    def expected(self) -> dict[str, bytes]:
        """Every sample this set keeps, by the name it lands under."""
        return {name: self.pinned().read(name) for name in self.kept()}

    def landing(self) -> list[str]:
        """The names this set should leave on disk."""
        return self.kept()

    def kept(self) -> list[str]:
        """The names the set writes to disk."""
        raise NotImplementedError

    # -- the difference ----------------------------------------------------

    def missing(self) -> dict[str, bytes]:
        """The kept samples that are not already here with the recorded bytes.

        A name the manifest pins but the record does not know about is fetched
        like any other: the record says what was written, not what was expected.
        """
        have = self.local()
        wanted = self.expected()
        return {name: blob for name, blob in wanted.items() if name not in have}

    def write(self, files: dict[str, bytes]) -> None:
        """Write samples out and record their hashes."""
        self.out.mkdir(parents=True, exist_ok=True)
        for name, blob in files.items():
            (self.out / name).write_bytes(blob)
        merged = self.recorded()
        merged.update({name: sha256(blob) for name, blob in files.items()})
        self.remember(merged)


# The record's name, and why it is what it is: it lives inside the sample
# directory, which is ignored by git, so a fresh checkout has none and pays the
# download it was always going to pay.
RECORD = ".fetched.json"
