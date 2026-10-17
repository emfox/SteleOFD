#!/usr/bin/env python3
"""Tests for check_commit_scope.py.

The checker reads git history, so most of this exercises the two functions that
decide what to report -- `area` and `review` -- on the shapes that were seen
while writing it. The end-to-end test builds a throwaway repository and runs the
script against it, because the reporting path (shallow clones, the limit, the
exit code) is where a refactor is most likely to break something quietly.
"""

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import check_commit_scope as scope  # noqa: E402


class AreaTest(unittest.TestCase):
    def test_crate_paths_group_by_crate(self):
        self.assertEqual(scope.area("crates/steleofd-parser/src/lib.rs"),
                         "crates/steleofd-parser")
        self.assertEqual(scope.area("crates/steleofd-parser/tests/a.rs"),
                         "crates/steleofd-parser")

    def test_workspace_manifest_is_its_own_area(self):
        # `crates` is in this repository a directory of crates, not a crate
        # with a manifest at its root, so the path falls through to the
        # last-resort branch. Pinning the behaviour rather than the intent:
        # if the layout gains a crates/Cargo.toml, this is the test to change.
        self.assertEqual(scope.area("crates/Cargo.toml"), "crates/Cargo.toml")

    def test_known_top_level_directories_group_by_directory(self):
        self.assertEqual(scope.area("scripts/check_all.py"), "scripts")
        self.assertEqual(scope.area(".github/workflows/rust.yml"), ".github")

    def test_other_paths_group_by_first_segment(self):
        self.assertEqual(scope.area("README.md"), "README.md")
        self.assertEqual(scope.area("docs/CONVENTIONS.md"), "docs")


class ReviewTest(unittest.TestCase):
    def test_one_area_is_one_change(self):
        lines = {"crates/steleofd-parser": 200}
        self.assertEqual(scope.review("fix(parser): step glyphs", lines), [])

    def test_a_small_second_area_is_a_mention(self):
        lines = {"crates/steleofd-parser": 200, "scripts": 5}
        self.assertEqual(scope.review("fix(parser): step glyphs", lines), [])

    def test_two_heavy_areas_are_reported(self):
        # The second area has to reach half the first, and the areas outside
        # the first have to hold half the lines. Two crates at 200 and 180
        # with nothing else is the borderline the check deliberately lets
        # through: at that ratio the "second change" may be a move.
        lines = {"crates/steleofd-parser": 200,
                 "crates/steleofd-display": 150,
                 "crates/steleofd-font": 150}
        reasons = scope.review("fix(parser): step glyphs", lines)
        self.assertTrue(reasons, lines)

    def test_subject_naming_a_crate_the_bulk_is_not_in_is_reported(self):
        lines = {"crates/steleofd-display": 200,
                 "crates/steleofd-parser": 60}
        reasons = scope.review("fix(parser): step glyphs", lines)
        self.assertTrue(reasons)
        self.assertIn("crates/steleofd-parser", reasons[0])

    def test_subject_scope_matches_primary_area(self):
        # The bulk is where the subject says it is. That is not enough to
        # clear the commit: `review` also asks whether the lines outside the
        # primary area hold half the total, and here they do not, so this
        # passes -- but for the spread rule, not because of the subject.
        lines = {"crates/steleofd-parser": 300, "crates/steleofd-display": 20}
        self.assertEqual(scope.review("fix(parser): step glyphs", lines), [])

    def test_a_concentrated_change_is_not_a_squash(self):
        # A rename moves lines into one area; the others are mentions.
        lines = {"crates/steleofd-parser": 400,
                 "crates/steleofd-display": 39,
                 "scripts": 20}
        self.assertEqual(scope.review("refactor(parser): rename a type", lines), [])


class CommitParseTest(unittest.TestCase):
    def setUp(self):
        # commits() shells out to git, which finds the enclosing repository.
        # A test that does not change into its own temporary repository reads
        # this project's history and passes or fails for reasons of its own.
        self.old = pathlib.Path.cwd()

    def tearDown(self):
        os.chdir(self.old)

    def test_generated_and_binary_files_are_not_areas(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp)
            self.git(repo, "init", "-q")
            self.git(repo, "config", "user.email", "t@example.com")
            self.git(repo, "config", "user.name", "t")
            os.chdir(repo)
            (repo / "a.txt").write_text("one\n")
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-qm", "feat(a): first")
            (repo / "a.txt").write_text("one\ntwo\n")
            (repo / "Cargo.lock").write_text("x\n" * 100)
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-qm", "feat(a): second")
            entries = scope.commits("HEAD")
            sha, subject, lines = entries[0]
            self.assertEqual(subject, "feat(a): second")
            self.assertNotIn("Cargo.lock", lines)
            self.assertEqual(lines, {"a.txt": 1})

    @staticmethod
    def git(repo, *args):
        subprocess.run(["git", "-C", str(repo), *args], check=True,
                       capture_output=True)


class EndToEndTest(unittest.TestCase):
    def test_reports_a_squash_and_passes_a_clean_history(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp)
            for path in ("crates/steleofd-parser", "crates/steleofd-display",
                         "scripts"):
                (repo / path).mkdir(parents=True)
            self.git(repo, "init", "-q")
            self.git(repo, "config", "user.email", "t@example.com")
            self.git(repo, "config", "user.name", "t")
            (repo / "scripts/x.py").write_text("")
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-qm", "chore(ci): start")

            for crate in ("steleofd-parser", "steleofd-display"):
                for name in ("a.rs", "b.rs", "c.rs"):
                    (repo / "crates" / crate / name).write_text("x\n" * 100)
            self.git(repo, "add", ".")
            self.git(repo, "commit", "-qm", "fix(parser): two crates at once")

            done = subprocess.run(
                [sys.executable,
                 str(pathlib.Path(__file__).resolve().parent
                     / "check_commit_scope.py")],
                cwd=repo, capture_output=True, text=True)
            self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
            self.assertIn("two crates at once", done.stdout)

    @staticmethod
    def git(repo, *args):
        subprocess.run(["git", "-C", str(repo), *args], check=True,
                       capture_output=True)


if __name__ == "__main__":
    unittest.main()
