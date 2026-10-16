#!/usr/bin/env python3
"""Performance gate: compare a benchmark run against the stated budget.

The budget lives in `docs/PERFORMANCE.md` and is what makes a benchmark
actionable; this script is what turns it into a pass/fail line. It reads the
criterion output a `cargo bench` run leaves in `target/criterion/` and checks
each budgeted benchmark's point estimate against its ceiling.

Only the ceiling is enforced. Ordinary noise, a dependency bump and different
CI hardware all move a benchmark by tens of percent, so a gate on the *change*
would be red on a bad afternoon and ignored within a week. The ceiling is set
well above the baseline on purpose: what it catches is the change that makes a
page cost several times what it did, which is the one worth stopping.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CRITERION = REPO / "target" / "criterion"

# One entry per budget line in docs/PERFORMANCE.md. `path` is the directory
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=CRITERION,
        help="where cargo bench left its output",
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

    if missing:
        print(f"\nFAILED: {len(missing)} budgeted benchmark(s) produced no output")
        for name in missing:
            print(f"  {name}: no target/criterion/{name}/new/estimates.json")
        return 1
    if over:
        print(f"\nFAILED: {len(over)} benchmark(s) over budget")
        for name, measured, budget_us in over:
            factor = measured / budget_us
            print(
                f"  {name}: {measured:.1f}µs against a {budget_us:.1f}µs budget "
                f"({factor:.1f}x)"
            )
        print("\nA cross here is a deliberate argument, not a merge: either the")
        print("change is worth it and docs/PERFORMANCE.md moves, or it is not.")
        return 1

    print("\nOK: every budgeted benchmark is within budget")
    return 0


if __name__ == "__main__":
    sys.exit(main())
