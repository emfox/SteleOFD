#!/usr/bin/env python3
"""Fetch sample documents from the pinned fixtures repository.

`fetch_ofdrw.py` and `fetch_gb_samples.py` both read sample `.ofd` files that
are too large and too third-party to commit here. This module is the one place
that decides how they arrive, so the two scripts cannot drift apart on the part
that matters: what a fetched file is allowed to be.

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
`ALL_PROXY=socks5h://...` and nothing else will see this module hang on a
direct connection until the timeout expires. Set `https_proxy` when the
environment sets only `ALL_PROXY`.
"""

from __future__ import annotations

import hashlib
import io
import json
import tarfile
import urllib.error
import urllib.request
import zipfile

# The fixtures repository, pinned by commit: see the module docstring.
FIXTURES = "emfox/OFD-fixtures"
COMMIT = "d8ec65e23cef0bbc1645ff0759bdd140cd3f9849"
ARCHIVE = "https://codeload.github.com/" + FIXTURES + "/tar.gz/" + COMMIT
USER_AGENT = "steleofd-corpus"


class FixtureError(Exception):
    """A fixture could not be fetched or did not match its pinned hash."""


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

    def names(self) -> list[str]:
        """Every sample in the set, sorted."""
        return sorted(self._files)

    def pinned(self, name: str) -> str:
        """The hash a sample must have."""
        try:
            return str(self._files[name]["sha256"])
        except KeyError as error:
            raise FixtureError(f"not pinned by the manifest: {name}") from error

    def read(self, name: str) -> bytes:
        """Return a sample's bytes, or refuse if they are not the pinned ones."""
        expected = self.pinned(name)
        blob = _member(self._archive, self._index, f"{self.set_name}/{name}")
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
        try:
            with zipfile.ZipFile(io.BytesIO(blob)) as archive:
                return {
                    info.filename: archive.read(info)
                    for info in archive.infolist()
                    if not info.is_dir()
                }
        except (zipfile.BadZipFile, OSError) as error:
            raise FixtureError(f"{name}: not a readable zip: {error}") from error
