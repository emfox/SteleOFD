#!/usr/bin/env python3
"""Performance gate: compare a benchmark run against the stated budget.

The budget lives in `docs/Performance.md` and is what makes a benchmark
actionable; this script is what turns it into a pass/fail line. It reads the
criterion output a `cargo bench` run leaves in `target/criterion/` and checks
each budgeted benchmark's point estimate against its ceiling.

Two kinds of line are gated. A *timing* is read from criterion's estimate; a
*memory* reading is a number the benchmark prints, because criterion measures
time and a count of live bytes is not a duration. Both are ceilings, and only the
ceiling is enforced. Ordinary noise, a dependency bump and different CI hardware
all move a benchmark by tens of percent, so a gate on the *change* would be red
on a bad afternoon and ignored within a week. The ceiling is set well above the
baseline on purpose: what it catches is the change that makes a page cost several
times what it did, which is the one worth stopping.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CRITERION = REPO / "target" / "criterion"

# One entry per budget line in docs/Performance.md. `path` is the directory
# criterion writes under target/criterion, `budget_us` the ceiling in
# microseconds for the one-page case. Keep both in step with the document: the
# script is the enforcement, the document is the explanation.
BUDGETS = [
    ("parse/open_document/1", 150.0),
    ("flatten/page_display/1", 2000.0),
    ("convert/png/1", 20000.0),
    ("convert/svg/1", 2500.0),
    ("convert/pdf/1", 10000.0),
]

# The memory ceilings, in mebibytes. One page is what a raster holds once it is
# drawn; the document figure is what a twenty-page packet may hold at once,
# which is what bounds the reader's page cache.
MEMORY_BUDGETS = [
    ("page_raster/1", 40.0),
    ("page_raster/20", 512.0),
]

# What the memory benchmark prints for one reading.
MEMORY_LINE = re.compile(r"^memory-held: (\S+) ([0-9.]+) MB\b", re.MULTILINE)


def estimate_us(path: Path) -> float | None:
    """The point estimate of a benchmark in microseconds, or None if absent.

    Criterion writes `new/estimates.json` on every run; a missing file means
    the benchmark did not run, which is reported rather than silently passed.
    """
    estimates = path / "new" / "estimates.json"
    if not estimates.exists():
        return None
    data = json.loads(estimates.read_text(encoding="utf-8"))
    # The point estimate is in nanoseconds.
    return data["mean"]["point_estimate"] / 1000.0


def memory_readings(paths: list[Path]) -> dict[str, float]:
    """Every memory reading a benchmark printed, in mebibytes.

    A reading is a printout rather than an estimate, so it is captured from the
    benchmark's own output: each path is read in full if it is a file, and
    searched for `*.log` if it is a directory.
    """
    readings: dict[str, float] = {}
    logs = []
    for path in paths:
        if path.is_file():
            logs.append(path)
        elif path.is_dir():
            logs.extend(path.rglob("*.log"))
    for log in logs:
        text = log.read_text(encoding="utf-8", errors="replace")
        for name, value in MEMORY_LINE.findall(text):
            # A reading repeats across samples; the largest is the one that
            # matters, because a ceiling is about the worst case.
            readings[name] = max(readings.get(name, 0.0), float(value))
    return readings


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=CRITERION,
        help="where cargo bench left its output",
    )
    parser.add_argument(
        "--logs",
        type=Path,
        nargs="*",
        default=None,
        help="benchmark output saved to a file, instead of target/criterion",
    )
    args = parser.parse_args()

    if not args.criterion_dir.exists():
        print(f"no benchmark output at {args.criterion_dir}")
        print("run `cargo bench` first")
        return 1

    missing = []
    over = []
    print(f"{'benchmark':<28} {'measured':>12} {'budget':>12}  result")
    print("-" * 66)
    for name, budget_us in BUDGETS:
        measured = estimate_us(args.criterion_dir / name)
        if measured is None:
            missing.append(name)
            print(f"{name:<28} {'-':>12} {budget_us:>10.1f}µs  NO DATA")
            continue
        if measured > budget_us:
            over.append((name, measured, budget_us))
            verdict = "OVER"
        else:
            verdict = "ok"
        print(
            f"{name:<28} {measured:>10.1f}µs {budget_us:>10.1f}µs  {verdict}"
        )

    readings = memory_readings(args.logs if args.logs else args.criterion_dir)

    print()
    print(f"{'memory':<28} {'held':>12} {'budget':>12}  result")
    print("-" * 66)
    for name, budget_mb in MEMORY_BUDGETS:
        held = readings.get(name)
        if held is None:
            missing.append(f"memory/{name}")
            print(f"{name:<28} {'-':>12} {budget_mb:>9.1f} MB  NO DATA")
            continue
        if held > budget_mb:
            over.append((name, held, budget_mb))
            verdict = "OVER"
        else:
            verdict = "ok"
        print(f"{name:<28} {held:>9.1f} MB {budget_mb:>9.1f} MB  {verdict}")

    if missing:
        print(f"\nFAILED: {len(missing)} budgeted benchmark(s) produced no output")
        for name in missing:
            print(f"  {name}: no target/criterion/{name}/new/estimates.json")
        return 1
    if over:
        print(f"\nFAILED: {len(over)} benchmark(s) over budget")
        for name, measured, budget in over:
            factor = measured / budget
            unit = "MB" if "/" in name and name.split("/")[0] in dict(MEMORY_BUDGETS) else "µs"
            print(
                f"  {name}: {measured:.1f}{unit} against a {budget:.1f}{unit} budget "
                f"({factor:.1f}x)"
            )
        print("\nA cross here is a deliberate argument, not a merge: either the")
        print("change is worth it and docs/Performance.md moves, or it is not.")
        return 1

    print("\nOK: every budgeted benchmark is within budget")
    return 0


if __name__ == "__main__":
    sys.exit(main())
