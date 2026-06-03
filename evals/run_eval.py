"""Dry-run Legion specialist evaluation harness."""

from __future__ import annotations

import argparse
import json


EVALS = [
    {
        "id": "schema-compliance",
        "metric": "valid_worker_result_json",
        "requires_network": False,
    },
    {
        "id": "patch-apply",
        "metric": "proposal_patch_applies",
        "requires_network": False,
    },
    {
        "id": "compile-test-pass",
        "metric": "verification_command_success",
        "requires_network": False,
    },
    {
        "id": "regression-rate",
        "metric": "baseline_behavior_preserved",
        "requires_network": False,
    },
    {
        "id": "latency-cost-refusal",
        "metric": "bounded_latency_cost_and_refusal_rate",
        "requires_network": False,
    },
]


def main() -> int:
    """Run or dry-run the evaluation harness."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", default="phase8-specialists")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    result = {
        "suite": args.suite,
        "dry_run": args.dry_run,
        "eval_count": len(EVALS),
        "evals": EVALS,
        "proposal_only_required": True,
        "metadata_only_default": True,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
