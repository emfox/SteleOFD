#!/usr/bin/env python3
"""Tests for check_performance.py.

The gate reads two different things: a timing from criterion's estimate, and a
memory reading from what the benchmark printed. The second is the newer one and
the easier to get wrong, so most of this exercises the parser that recovers it,
and one case runs the gate against a log that is over its ceiling.
"""

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

GATE = pathlib.Path(__file__).resolve().parent / "check_performance.py"
sys.path.insert(0, str(GATE.parent))

import check_performance as gate  # noqa: E402


class MemoryReadingTest(unittest.TestCase):
    def readings(self, text: str) -> dict[str, float]:
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "bench.log"
            log.write_text(text, encoding="utf-8")
            return gate.memory_readings([log], pathlib.Path(directory) / "none")[0]

    def test_a_reading_is_recovered_with_its_name(self):
        got = self.readings("memory-held: page_raster/1 18.8 MB (95496 byte png)\n")
        self.assertEqual(got, {"page_raster/1": 18.8})

    def test_the_largest_of_repeated_readings_wins(self):
        text = (
            "memory-held: page_raster/20 17.5 MB (1 byte png)\n"
            "memory-held: page_raster/20 19.2 MB (1 byte png)\n"
        )
        self.assertEqual(self.readings(text)["page_raster/20"], 19.2)

    def test_a_bare_number_is_not_a_reading(self):
        # Criterion treats any printed number as one more benchmark value, so a
        # reading without its prefix and unit must not be mistaken for one.
        self.assertEqual(self.readings("page_raster/1: 19676178\n"), {})

    def test_text_that_mentions_a_reading_mid_line_is_ignored(self):
        self.assertEqual(self.readings("  memory-held: page_raster/1 18.8 MB\n"), {})


class DirectoryTest(unittest.TestCase):
    """The gate with no `--logs`, which is how a local run invokes it.

    A reading only exists in a benchmark's stdout, and criterion discards that
    after the run. The default path must still report rather than trace back.
    """

    def run_gate(self, directory: pathlib.Path) -> tuple[int, str]:
        result = subprocess.run(
            [sys.executable, str(GATE), "--criterion-dir", str(directory)],
            capture_output=True,
            text=True,
            cwd=GATE.parent.parent,
        )
        return result.returncode, result.stdout + result.stderr

    def test_a_directory_with_no_readings_fails_instead_of_raising(self):
        # A directory is one path, not a list of them, which is what used to
        # raise `TypeError: 'PosixPath' object is not iterable`.
        with tempfile.TemporaryDirectory() as directory:
            code, output = self.run_gate(pathlib.Path(directory))
        self.assertEqual(code, 1, output)
        self.assertNotIn("Traceback", output)
        self.assertIn("NO DATA", output)

    def test_a_reading_saved_in_a_directory_is_found(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "memory").mkdir()
            (root / "memory" / "run.log").write_text(
                "memory-held: page_raster/1 18.8 MB (1 byte png)\n", encoding="utf-8"
            )
            got = gate.memory_readings([root], root)[0]
        self.assertEqual(got["page_raster/1"], 18.8)

    def test_a_run_without_saved_output_names_the_command_that_would_help(self):
        # Criterion keeps an estimate but not the stdout the reading was
        # printed to, so the gate has to say what would supply it.
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "memory" / "page_raster" / "1" / "new").mkdir(parents=True)
            code, output = self.run_gate(root)
        self.assertEqual(code, 1, output)
        self.assertNotIn("Traceback", output)
        self.assertIn("--bench memory", output)

    def test_the_readings_the_benchmark_kept_are_read_with_no_arguments(self):
        # The benchmark appends each reading to this file precisely because
        # criterion discards the stdout it printed them to.
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "memory" / "page_raster" / "1" / "new").mkdir(parents=True)
            (root / gate.READINGS).write_text(
                "memory-held: page_raster/1 18.8 MB (1 byte png)\n", encoding="utf-8"
            )
            code, output = self.run_gate(root)
        self.assertIn("page_raster/1", output)
        self.assertIn("18.8 MB", output)


class StalenessTest(unittest.TestCase):
    """A reading describes the code that produced it, and nothing else."""

    def test_a_log_written_after_the_source_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "benchmarks.log"
            log.write_text("", encoding="utf-8")
            self.assertEqual(gate.stale(log), "")

    def test_a_log_written_before_the_source_is_named_with_its_fix(self):
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "benchmarks.log"
            log.write_text("", encoding="utf-8")
            # Backdating the log is what an older `cargo bench` left behind.
            os.utime(log, (0, 0))
            reason = gate.stale(log)
        self.assertIn("older than the last source change", reason)
        self.assertIn("run_benchmarks.py", reason)

    def test_the_source_date_is_the_newest_file_that_could_move_a_benchmark(self):
        self.assertGreater(gate.newest_source(), 0.0)


class BudgetTest(unittest.TestCase):
    def test_every_memory_ceiling_names_a_reading_the_benchmark_prints(self):
        names = {name for name, _ in gate.MEMORY_BUDGETS}
        self.assertEqual(names, {"page_raster/1", "page_raster/20"})


if __name__ == "__main__":
    unittest.main()
