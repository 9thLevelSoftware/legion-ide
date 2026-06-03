"""Dry-run conversion plan for trained Legion adapters."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> int:
    """Validate conversion arguments and print an operator plan."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="training/out/docs-summarizer")
    parser.add_argument("--output", default="models/docs-summarizer.Q4_K_M.gguf")
    parser.add_argument("--quantization", default="Q4_K_M")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    plan = {
        "dry_run": args.dry_run,
        "input": args.input,
        "output": args.output,
        "quantization": args.quantization,
        "tool": "llama.cpp convert + quantize",
    }
    print(json.dumps(plan, indent=2, sort_keys=True))
    if args.dry_run:
        return 0

    if not Path(args.input).exists():
        raise SystemExit(f"input does not exist: {args.input}")
    raise SystemExit("real conversion requires operator-installed llama.cpp tooling")


if __name__ == "__main__":
    raise SystemExit(main())
