"""Dry-run QLoRA training entrypoint for Legion specialist models."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> int:
    """Validate a training request and print the planned job."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", default="Qwen/Qwen2.5-Coder-1.5B-Instruct")
    parser.add_argument("--dataset", default="datasets/legion-traces.jsonl")
    parser.add_argument("--output-dir", default="training/out/docs-summarizer")
    parser.add_argument("--specialist", default="docs-summarizer")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    plan = {
        "dry_run": args.dry_run,
        "model_id": args.model_id,
        "dataset": args.dataset,
        "output_dir": args.output_dir,
        "specialist": args.specialist,
        "method": "qlora",
        "consent_required": True,
        "redaction_required": True,
        "raw_trace_default": "disabled",
    }
    print(json.dumps(plan, indent=2, sort_keys=True))
    if args.dry_run:
        return 0

    dataset = Path(args.dataset)
    if not dataset.exists():
        raise SystemExit(f"dataset does not exist: {dataset}")
    raise SystemExit(
        "real training requires installing the operator-selected training stack"
    )


if __name__ == "__main__":
    raise SystemExit(main())
