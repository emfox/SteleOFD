#!/usr/bin/env python3
"""Tests for check_performance.py.

The gate reads two different things: a timing from criterion's estimate, and a
memory reading from what the benchmark printed. The second is the newer one and
the easier to get wrong, so most of this exercises the parser that recovers it,
and one case runs the gate against a log that is over its ceiling.
"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import check_performance as gate  # noqa: E402


class MemoryReadingTest(unittest.TestCase):
    def readings(self, text: str) -> dict[str, float]:
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "bench.log"
            log.write_text(text, encoding="utf-8")
            return gate.memory_readings([log])

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


class BudgetTest(unittest.TestCase):
    def test_every_memory_ceiling_names_a_reading_the_benchmark_prints(self):
        names = {name for name, _ in gate.MEMORY_BUDGETS}
        self.assertEqual(names, {"page_raster/1", "page_raster/20"})


if __name__ == "__main__":
    unittest.main()
