"""Legion specialist evaluation harness with optional offline and endpoint modes."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


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


def _load_jsonl(path: Path, max_examples: int | None) -> list[dict[str, Any]]:
    examples: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                examples.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid JSON in {path}: {exc}") from exc
            if max_examples is not None and len(examples) >= max_examples:
                break
    return examples


def _redact_endpoint(endpoint: str) -> str:
    """Return an endpoint string safe for logging: strip any query credentials."""
    if "?" in endpoint:
        endpoint = endpoint.split("?", 1)[0]
    return endpoint


def _call_endpoint(
    endpoint: str,
    model: str,
    messages: list[dict[str, str]],
    api_key: str | None,
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Call an OpenAI-compatible /v1/chat/completions endpoint using stdlib urllib."""
    url = endpoint.rstrip("/") + "/v1/chat/completions"
    payload = json.dumps(
        {
            "model": model,
            "messages": messages,
            "temperature": 0.0,
            "max_tokens": 1024,
        }
    ).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"
    req = urllib.request.Request(url, data=payload, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        raise SystemExit(
            f"endpoint request failed: {exc.code} {exc.reason}"
        ) from exc
    except Exception as exc:
        raise SystemExit(f"endpoint request failed: {exc}") from exc


def _run_offline(examples: list[dict[str, Any]]) -> dict[str, Any]:
    """Compute metrics from fixture expectations without network access."""
    results = []
    for ex in examples:
        result = {
            "example_id": ex.get("id", "unknown"),
            "schema_compliance": ex.get("expected_schema", False),
            "proposal_patch_applies": ex.get("expected_patch_applies", False),
            "verification_pass": ex.get("expected_verification_pass", False),
            "regression": ex.get("expected_regression", False),
            "refusal": ex.get("expected_refusal", False),
            "latency_ms": None,
            "cost_usd": None,
            "model": "fixture",
        }
        results.append(result)

    total = len(results)
    return {
        "mode": "offline-fixture",
        "eval_count": total,
        "summary": {
            "schema_compliance_rate": (
                sum(1 for r in results if r["schema_compliance"]) / total if total else 0.0
            ),
            "patch_apply_rate": (
                sum(1 for r in results if r["proposal_patch_applies"]) / total if total else 0.0
            ),
            "verification_pass_rate": (
                sum(1 for r in results if r["verification_pass"]) / total if total else 0.0
            ),
            "regression_rate": (
                sum(1 for r in results if r["regression"]) / total if total else 0.0
            ),
            "refusal_rate": (
                sum(1 for r in results if r["refusal"]) / total if total else 0.0
            ),
            "latency_ms_placeholder": True,
            "cost_usd_placeholder": True,
        },
        "results": results,
    }


def _run_endpoint(
    examples: list[dict[str, Any]],
    endpoint: str,
    model: str,
    max_examples: int | None,
) -> dict[str, Any]:
    """Evaluate examples against a live endpoint."""
    api_key = os.environ.get("OPENAI_API_KEY") or os.environ.get("LEGION_API_KEY")
    examples = examples[:max_examples] if max_examples else examples
    results = []
    for ex in examples:
        messages = ex.get("messages", [])
        start = time.perf_counter()
        try:
            resp = _call_endpoint(endpoint, model, messages, api_key)
            content = resp["choices"][0]["message"].get("content", "")
        except Exception as exc:
            content = f"[error: {exc}]"
        latency_ms = int((time.perf_counter() - start) * 1000)

        # Lightweight heuristics for real endpoint mode
        schema_compliance = content.strip().startswith("{")
        refusal = "refuse" in content.lower() or "cannot" in content.lower()
        result = {
            "example_id": ex.get("id", "unknown"),
            "schema_compliance": schema_compliance,
            "proposal_patch_applies": None,
            "verification_pass": None,
            "regression": None,
            "refusal": refusal,
            "latency_ms": latency_ms,
            "cost_usd": None,
            "model": model,
            "endpoint": _redact_endpoint(endpoint),
        }
        results.append(result)

    total = len(results)
    return {
        "mode": "endpoint",
        "eval_count": total,
        "summary": {
            "schema_compliance_rate": (
                sum(1 for r in results if r["schema_compliance"]) / total if total else 0.0
            ),
            "patch_apply_rate": None,
            "verification_pass_rate": None,
            "regression_rate": None,
            "refusal_rate": (
                sum(1 for r in results if r["refusal"]) / total if total else 0.0
            ),
            "latency_ms_placeholder": False,
            "cost_usd_placeholder": True,
        },
        "results": results,
    }


def main() -> int:
    """Run or dry-run the evaluation harness."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", default="phase8-specialists")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--dataset", default="")
    parser.add_argument("--endpoint", default="")
    parser.add_argument("--model", default="")
    parser.add_argument("--output", default="")
    parser.add_argument("--max-examples", type=int, default=None)
    parser.add_argument("--offline-fixture", action="store_true")
    args = parser.parse_args()

    if args.dry_run:
        result = {
            "suite": args.suite,
            "dry_run": True,
            "eval_count": len(EVALS),
            "evals": EVALS,
            "proposal_only_required": True,
            "metadata_only_default": True,
        }
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0

    if args.offline_fixture:
        if not args.dataset:
            raise SystemExit("--dataset is required for --offline-fixture mode")
        dataset_path = Path(args.dataset)
        if not dataset_path.exists():
            raise SystemExit(f"dataset does not exist: {dataset_path}")
        examples = _load_jsonl(dataset_path, args.max_examples)
        result = _run_offline(examples)
        result["suite"] = args.suite
        result["dataset"] = str(dataset_path)
        result["max_examples"] = args.max_examples
        if args.output:
            Path(args.output).write_text(
                json.dumps(result, indent=2, sort_keys=True), encoding="utf-8"
            )
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0

    if args.endpoint:
        if not args.dataset:
            raise SystemExit("--dataset is required for endpoint mode")
        if not args.model:
            raise SystemExit("--model is required for endpoint mode")
        dataset_path = Path(args.dataset)
        if not dataset_path.exists():
            raise SystemExit(f"dataset does not exist: {dataset_path}")
        examples = _load_jsonl(dataset_path, args.max_examples)
        result = _run_endpoint(examples, args.endpoint, args.model, args.max_examples)
        result["suite"] = args.suite
        result["dataset"] = str(dataset_path)
        if args.output:
            Path(args.output).write_text(
                json.dumps(result, indent=2, sort_keys=True), encoding="utf-8"
            )
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0

    raise SystemExit(
        "Specify one mode: --dry-run, --offline-fixture, or --endpoint"
    )


if __name__ == "__main__":
    raise SystemExit(main())
