"""Standalone verification script for the TextKit line-deduplication feature.

Run from anywhere with: python checks/verify_dedup.py
Exits 0 when the feature behaves as specified, 1 otherwise.
"""

import io
import os
import sys
from contextlib import redirect_stdout

# Make the fixture root importable regardless of the caller's cwd.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

failures = []


def check(name, actual, expected):
    if actual != expected:
        failures.append(f"{name}: expected {expected!r}, got {actual!r}")


def main():
    try:
        from textkit.dedup import dedupe_lines
    except Exception as exc:  # noqa: BLE001 - report any import problem
        print(f"FAIL: cannot import dedupe_lines from textkit.dedup: {exc}")
        return 1

    check("keeps first occurrence, preserves order",
          dedupe_lines(["b", "a", "b", "a", "c"]), ["b", "a", "c"])
    check("case sensitive", dedupe_lines(["A", "a", "A"]), ["A", "a"])
    check("empty list", dedupe_lines([]), [])
    check("no duplicates unchanged", dedupe_lines(["x", "y"]), ["x", "y"])

    original = ["dup", "dup", "solo"]
    result = dedupe_lines(original)
    check("returns a new list", result is original, False)
    check("does not mutate input", original, ["dup", "dup", "solo"])

    try:
        from textkit import cli
    except Exception as exc:  # noqa: BLE001
        print(f"FAIL: cannot import textkit.cli: {exc}")
        return 1

    buffer = io.StringIO()
    with redirect_stdout(buffer):
        exit_code = cli.main(["dedup", "x\ny\nx"])
    check("cli dedup exit code", exit_code, 0)
    check("cli dedup output", buffer.getvalue(), "x\ny\n")

    buffer = io.StringIO()
    with redirect_stdout(buffer):
        exit_code = cli.main(["dedup", ""])
    check("cli dedup empty text exit code", exit_code, 0)
    check("cli dedup empty text output", buffer.getvalue(), "")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
