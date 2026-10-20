#!/usr/bin/env python3
"""Tests for the fixture fetch: the record, the archive, the proxy.

The part worth testing is not the download but what decides against it. A
sample already on disk under its own name with the hash the last run recorded
must not be fetched again, and one whose bytes changed must be. Both are
decided from a file this module writes, so most of this builds a directory and
a fake `Fixtures` and never opens a socket.

`sample_fetch.check` is end to end against that fake as well: the branch that
touches the network is the branch being pinned, and a test that reached GitHub
would be a test that fails on a plane.
"""

import os
import pathlib
import sys
import tarfile
import tempfile
import unittest
import unittest.mock
import zipfile
import io

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import fixtures  # noqa: E402
import sample_fetch  # noqa: E402


def archive_of(members: dict[str, bytes]) -> bytes:
    """A tar.gz shaped like a codeload archive: one top directory."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as tar:
        for path, blob in members.items():
            info = tarfile.TarInfo(f"repo-1ad683e/{path}")
            info.size = len(blob)
            tar.addfile(info, io.BytesIO(blob))
    return buffer.getvalue()


class FakeFixtures:
    """A `Fixtures` stand-in that serves bytes without a network."""

    def __init__(self, set_name: str, members: dict[str, bytes],
                 pinned: dict[str, bytes]) -> None:
        self.blob = archive_of(members)
        self._index = fixtures._members(self.blob)
        self._members = {}
        self._pinned = pinned
        self.reads: list[str] = []
        self.set_name = set_name

    @property
    def files(self):
        return dict(self._pinned)

    def unpack(self, path):
        self.reads.append(path)
        return fixtures._member(self.blob, self._index, path)

    def read(self, name):
        """A sample by name, the way the manifest pins it: set directory first."""
        self.reads.append(name)
        return self.unpack(f"{self.set_name}/{name}")

    def unzip(self, name):
        return fixtures.unzip(self.read(name), name)

    def names(self):
        return sorted(self._pinned)


def small_ofd(payload: bytes = b"") -> bytes:
    """Bytes that pass the sanity check: a zip with an OFD.xml entry."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        archive.writestr("OFD.xml", b"<OFD/>")
        archive.writestr("body.bin", payload)
    return buffer.getvalue()


class ProxyTest(unittest.TestCase):
    def setUp(self):
        self.saved = {k: os.environ.get(k) for k in
                      ("ALL_PROXY", "all_proxy", "http_proxy", "https_proxy")}
        for key in self.saved:
            os.environ.pop(key, None)

    def tearDown(self):
        for key, value in self.saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def test_an_explicit_url_sets_both_variables_urllib_reads(self):
        # `utllib` reads `http_proxy` and `https_proxy` and nothing else, so
        # naming one has to reach both or a half of the traffic goes direct.
        set_now = fixtures.configure_proxy("http://127.0.0.1:7890")
        self.assertEqual(set(set_now), {"http_proxy", "https_proxy"})
        self.assertEqual(os.environ["https_proxy"], "http://127.0.0.1:7890")

    def test_all_proxy_is_copied_over_when_nothing_else_names_one(self):
        os.environ["ALL_PROXY"] = "http://127.0.0.1:7890"
        self.assertEqual(set(fixtures.configure_proxy()), {"http_proxy", "https_proxy"})
        self.assertEqual(os.environ["http_proxy"], "http://127.0.0.1:7890")

    def test_a_socks_url_is_left_alone(self):
        # `urllib` speaks HTTP proxies; handing it a socks5h URL fails at
        # request time with a message about the wrong thing.
        os.environ["ALL_PROXY"] = "socks5h://127.0.0.1:1080"
        self.assertEqual(fixtures.configure_proxy(), ())
        self.assertNotIn("https_proxy", os.environ)

    def test_nothing_configured_changes_nothing(self):
        self.assertEqual(fixtures.configure_proxy(), ())
        self.assertNotIn("http_proxy", os.environ)


class RecordTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.out = pathlib.Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def test_a_fresh_directory_records_nothing(self):
        disk = fixtures.SampleSet("set", self.out)
        self.assertEqual(disk.recorded(), {})
        self.assertEqual(disk.local(), {})

    def test_written_hashes_read_back(self):
        disk = fixtures.SampleSet("set", self.out)
        disk.write({"a.ofd": b"one", "b.ofd": b"two"})
        self.assertEqual(
            disk.recorded(),
            {"a.ofd": fixtures.sha256(b"one"), "b.ofd": fixtures.sha256(b"two")},
        )
        self.assertEqual(disk.local(), disk.recorded())

    def test_a_changed_file_is_no_longer_local(self):
        disk = fixtures.SampleSet("set", self.out)
        disk.write({"a.ofd": b"one", "b.ofd": b"two"})
        (self.out / "a.ofd").write_bytes(b"edited")
        self.assertEqual(list(disk.local()), ["b.ofd"])

    def test_a_missing_file_is_no_longer_local(self):
        disk = fixtures.SampleSet("set", self.out)
        disk.write({"a.ofd": b"one"})
        (self.out / "a.ofd").unlink()
        self.assertEqual(disk.local(), {})

    def test_two_sets_share_one_record(self):
        # The record lives in the parent of both sample directories only when
        # they share one; each set normally has its own, and neither may drop
        # the other's entry on the way past.
        first = fixtures.SampleSet("first", self.out)
        second = fixtures.SampleSet("second", self.out)
        first.write({"a.ofd": b"one"})
        second.write({"b.ofd": b"two"})
        self.assertEqual(list(first.recorded()), ["a.ofd"])
        self.assertEqual(list(second.recorded()), ["b.ofd"])

    def test_a_corrupt_record_reads_as_empty_rather_than_raising(self):
        (self.out / fixtures.RECORD).write_text("not json", encoding="utf-8")
        self.assertEqual(fixtures.SampleSet("set", self.out).recorded(), {})


class ExpectedTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.out = pathlib.Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def disk(self, spec, members, pinned):
        return sample_fetch.DiskSet(
            spec, self.out, pinned=FakeFixtures(spec.name, members, pinned))

    def test_loose_files_come_from_the_manifest_hashes(self):
        blob = small_ofd()
        spec = sample_fetch.Set(
            name="loose", about="", header="", samples={"a.ofd": "why"},
            unparsed={}, sections=(sample_fetch.Section(path="loose", archive=False),),
        )
        disk = self.disk(spec, {"loose/a.ofd": blob}, {"a.ofd": blob})
        self.assertEqual(disk.expected(), {"a.ofd": blob})

    def test_a_nested_archive_is_peeled(self):
        inner = io.BytesIO()
        with zipfile.ZipFile(inner, "w") as layer:
            layer.writestr("doc.ofd", small_ofd())
        spec = sample_fetch.Set(
            name="nested", about="", header="", samples={"doc.ofd": "why"},
            unparsed={},
            sections=(sample_fetch.Section(path="nested/outer.zip", pinned=False,
                                           inner="inner.zip"),),
        )
        outer = io.BytesIO()
        with zipfile.ZipFile(outer, "w") as layer:
            layer.writestr("inner.zip", inner.getvalue())
        disk = self.disk(spec, {"nested/outer.zip": outer.getvalue()}, {"outer.zip": b""})
        self.assertEqual(list(disk.expected()), ["doc.ofd"])

    def test_a_nested_archive_that_is_absent_is_named(self):
        spec = sample_fetch.Set(
            name="nested", about="", header="", samples={"doc.ofd": "why"},
            unparsed={},
            sections=(sample_fetch.Section(path="nested/outer.zip", pinned=False,
                                           inner="missing.zip"),),
        )
        outer = io.BytesIO()
        with zipfile.ZipFile(outer, "w") as layer:
            layer.writestr("other.zip", b"")
        disk = self.disk(spec, {"nested/outer.zip": outer.getvalue()}, {"outer.zip": b""})
        with self.assertRaises(fixtures.FixtureError):
            disk.expected()


class CheckTest(unittest.TestCase):
    """`check` decides from the record before it touches the archive."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.out = pathlib.Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
        self.blob = small_ofd()

    def spec(self):
        return sample_fetch.Set(
            name="loose", about="", header="", samples={"a.ofd": "why"},
            unparsed={}, sections=(sample_fetch.Section(path="loose", archive=False),),
        )

    def patch_disk(self, spec, pinned):
        """Make `check` open this directory against a stand-in archive."""
        real = sample_fetch.DiskSet
        return unittest.mock.patch.object(
            sample_fetch, "DiskSet",
            side_effect=lambda *a, **k: real(spec, self.out, pinned=pinned),
        )

    def test_a_recorded_sample_is_not_fetched_again(self):
        spec = self.spec()
        real = sample_fetch.DiskSet
        real(spec, self.out, pinned=FakeFixtures(
            "loose", {"loose/a.ofd": self.blob}, {"a.ofd": self.blob})).write({"a.ofd": self.blob})
        # The archive would now be a network fetch, and it must not be reached:
        # a stand-in that raises says so louder than a counter.
        with self.patch_disk(spec, Exploding()):
            self.assertEqual(sample_fetch.check(spec, self.out, 1.0), 0)

    def test_an_absent_sample_is_fetched(self):
        spec = self.spec()
        pinned = FakeFixtures("loose", {"loose/a.ofd": self.blob}, {"a.ofd": self.blob})
        with self.patch_disk(spec, pinned):
            self.assertEqual(sample_fetch.check(spec, self.out, 1.0), 0)
        self.assertEqual((self.out / "a.ofd").read_bytes(), self.blob)

    def test_a_changed_sample_is_replaced(self):
        spec = self.spec()
        (self.out / "a.ofd").write_bytes(b"old")
        pinned = FakeFixtures("loose", {"loose/a.ofd": self.blob}, {"a.ofd": self.blob})
        with self.patch_disk(spec, pinned):
            self.assertEqual(sample_fetch.check(spec, self.out, 1.0), 0)
        self.assertEqual((self.out / "a.ofd").read_bytes(), self.blob)

    def test_an_archive_that_cannot_be_reached_reports_each_unresolved_name(self):
        spec = self.spec()
        with self.patch_disk(spec, Exploding()):
            self.assertEqual(sample_fetch.check(spec, self.out, 1.0), 1)


class Exploding:
    """A `Fixtures` that fails the test if the network is reached."""

    files = {}

    def __getattr__(self, name):
        raise AssertionError(f"the pinned archive was opened for {name}")


if __name__ == "__main__":
    unittest.main()
