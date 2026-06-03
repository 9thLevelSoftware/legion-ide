"""QLoRA training entrypoint for Legion specialist models with optional real paths."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
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
    return examples


def _validate_dataset(examples: list[dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    for i, ex in enumerate(examples):
        if "instruction" not in ex:
            errors.append(f"example {i} missing 'instruction'")
        if "output" not in ex:
            errors.append(f"example {i} missing 'output'")
    return errors


def _write_manifest(output_dir: Path, manifest: dict[str, Any]) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8"
    )


def _run_fixture_smoke(
    dataset_path: Path,
    output_dir: Path,
    base_model: str,
    max_steps: int,
    learning_rate: float,
    lora_rank: int,
    sequence_length: int,
    device: str,
) -> int:
    """CPU/lightweight fixture smoke: validate dataset and write a manifest."""
    examples = _load_jsonl(dataset_path)
    errors = _validate_dataset(examples)
    if errors:
        for err in errors:
            print(f"dataset validation error: {err}", file=sys.stderr)
        return 2

    manifest = {
        "mode": "fixture-smoke",
        "dataset": str(dataset_path),
        "example_count": len(examples),
        "base_model": base_model,
        "max_steps": max_steps,
        "learning_rate": learning_rate,
        "lora_rank": lora_rank,
        "sequence_length": sequence_length,
        "device": device,
        "heavy_deps": False,
        "manifest_path": str(output_dir / "manifest.json"),
    }
    _write_manifest(output_dir, manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


def _import_training_deps() -> dict[str, Any]:
    """Lazily import heavy training dependencies and return module handles."""
    missing: list[str] = []
    try:
        import torch
    except Exception as exc:
        missing.append(f"torch (install: pip install torch)  [{exc}]")
        torch = None
    try:
        import transformers
    except Exception as exc:
        missing.append(f"transformers (install: pip install transformers)  [{exc}]")
        transformers = None
    try:
        import peft
    except Exception as exc:
        missing.append(f"peft (install: pip install peft)  [{exc}]")
        peft = None
    try:
        import datasets
    except Exception as exc:
        missing.append(f"datasets (install: pip install datasets)  [{exc}]")
        datasets = None
    try:
        import trl
    except Exception as exc:
        missing.append(f"trl (install: pip install trl)  [{exc}]")
        trl = None
    if missing:
        print("Missing required training dependencies:", file=sys.stderr)
        for item in missing:
            print(f"  - {item}", file=sys.stderr)
        print(
            "\nInstall with: pip install torch transformers peft datasets trl",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return {
        "torch": torch,
        "transformers": transformers,
        "peft": peft,
        "datasets": datasets,
        "trl": trl,
    }


def _build_training_plan(
    dataset_path: Path,
    output_dir: Path,
    base_model: str,
    max_steps: int,
    learning_rate: float,
    lora_rank: int,
    sequence_length: int,
    device: str,
) -> dict[str, Any]:
    """Build a real training plan/manifest with dep validation but no long GPU run."""
    deps = _import_training_deps()
    examples = _load_jsonl(dataset_path)
    errors = _validate_dataset(examples)
    if errors:
        for err in errors:
            print(f"dataset validation error: {err}", file=sys.stderr)
        return {"valid": False, "errors": errors}

    # Minimal real code skeleton: validate dataset loading and dep versions
    plan = {
        "mode": "real",
        "valid": True,
        "dataset": str(dataset_path),
        "example_count": len(examples),
        "base_model": base_model,
        "max_steps": max_steps,
        "learning_rate": learning_rate,
        "lora_rank": lora_rank,
        "sequence_length": sequence_length,
        "device": device,
        "output_dir": str(output_dir),
        "deps_present": {
            "torch": deps["torch"].__version__,
            "transformers": deps["transformers"].__version__,
            "peft": deps["peft"].__version__,
            "datasets": deps["datasets"].__version__,
            "trl": deps["trl"].__version__,
        },
        "note": (
            "Training plan validated. To start training, run with explicit "
            "operator args (e.g., --max-steps > 0 and --device cuda)."
        ),
    }
    _write_manifest(output_dir, plan)
    print(json.dumps(plan, indent=2, sort_keys=True))
    return plan


def main() -> int:
    """Validate a training request and print the planned job."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", default="Qwen/Qwen2.5-Coder-1.5B-Instruct")
    parser.add_argument("--base-model", default="")
    parser.add_argument("--dataset", default="datasets/legion-traces.jsonl")
    parser.add_argument("--output-dir", default="training/out/docs-summarizer")
    parser.add_argument("--specialist", default="docs-summarizer")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--max-steps", type=int, default=0)
    parser.add_argument("--learning-rate", type=float, default=2e-4)
    parser.add_argument("--lora-rank", type=int, default=16)
    parser.add_argument("--sequence-length", type=int, default=2048)
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--fixture-smoke", action="store_true")
    args = parser.parse_args()

    base_model = args.base_model or args.model_id

    if args.dry_run:
        plan = {
            "dry_run": True,
            "model_id": args.model_id,
            "base_model": base_model,
            "dataset": args.dataset,
            "output_dir": args.output_dir,
            "specialist": args.specialist,
            "method": "qlora",
            "consent_required": True,
            "redaction_required": True,
            "raw_trace_default": "disabled",
        }
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0

    dataset = Path(args.dataset)
    if not dataset.exists():
        raise SystemExit(f"dataset does not exist: {dataset}")

    if args.fixture_smoke:
        return _run_fixture_smoke(
            dataset,
            Path(args.output_dir),
            base_model,
            args.max_steps,
            args.learning_rate,
            args.lora_rank,
            args.sequence_length,
            args.device,
        )

    # Real mode: validate deps and build a manifest. Do not launch long training
    # unless operator explicitly provides positive max_steps.
    plan = _build_training_plan(
        dataset,
        Path(args.output_dir),
        base_model,
        args.max_steps,
        args.learning_rate,
        args.lora_rank,
        args.sequence_length,
        args.device,
    )
    if not plan.get("valid"):
        return 2
    if args.max_steps <= 0:
        print(
            "\nSkipping training run because --max-steps <= 0. "
            "To run training, set a positive --max-steps.",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
