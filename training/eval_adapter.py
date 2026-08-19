"""Score a base model and a LoRA adapter on the same consented holdout split.

The two arms share one process and one set of quantized base weights: the
adapter is attached after the base arm has been scored and detached is never
needed, because ``PeftModel`` wraps the same tensors. That matters more than it
sounds. A comparison that reloads the model between arms is comparing two
quantizations as well as two adapters, and NF4 quantization is not bit-identical
across loads on every backend. Sharing the weights makes the adapter the only
difference between the arms.

Scoring is a forced choice, not free generation: for each holdout prompt the
model's total log-probability of `` Accepted`` and of `` Rejected`` is computed
and the larger wins. Generation would add sampling variance and a parser, and
neither is part of what is being measured.

The holdout split is produced by ``cargo run -p xtask -- training-corpus``, which
withholds it from ``train.jsonl``. This script re-checks the export manifest with
the same consent gate the trainer uses, because an eval that reads unconsented
data is the same leak as a training run that does.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from training.qlora_train import (  # noqa: E402
    ConsentRefusal,
    _load_jsonl,
    assert_dataset_is_consented,
    label_token_ids,
    render_prompt,
    sha256_file,
)

#: The two decisions the forced choice is between.
LABELS = ("Accepted", "Rejected")


def score_examples(model, tokenizer, examples, torch, sequence_length):
    """Forced-choice accuracy over the holdout split.

    Returns predictions plus the per-label confusion counts. Accuracy alone
    cannot tell a model that learned the task from one that learned to always
    answer the majority label, and on this corpus the majority label is a real
    52.9% -- so the confusion counts are the part worth reading.
    """
    device = next(model.parameters()).device
    label_ids = {label: label_token_ids(tokenizer, label) for label in LABELS}

    correct = 0
    predictions: list[dict[str, Any]] = []
    confusion = {truth: {pred: 0 for pred in LABELS} for truth in LABELS}
    predicted_counts = {label: 0 for label in LABELS}

    model.eval()
    with torch.no_grad():
        for example in examples:
            prompt_ids = tokenizer(
                render_prompt(example["instruction"]), add_special_tokens=False
            )["input_ids"]
            scores: dict[str, float] = {}
            for label, continuation in label_ids.items():
                ids = (prompt_ids + continuation)[:sequence_length]
                input_ids = torch.tensor([ids], device=device)
                logits = model(input_ids=input_ids).logits.float()
                log_probs = torch.log_softmax(logits, dim=-1)[0]
                start = len(ids) - len(continuation)
                total = 0.0
                for offset, token in enumerate(continuation):
                    position = start + offset - 1
                    total += float(log_probs[position, token].item())
                scores[label] = total

            predicted = max(scores, key=scores.get)
            truth = example["output"]
            predicted_counts[predicted] += 1
            if truth in confusion:
                confusion[truth][predicted] += 1
            if predicted == truth:
                correct += 1
            predictions.append(
                {
                    "example_id": example.get("example_id"),
                    "truth": truth,
                    "predicted": predicted,
                    "logprob_accepted": round(scores["Accepted"], 6),
                    "logprob_rejected": round(scores["Rejected"], 6),
                }
            )

    total_count = len(examples)
    return {
        "example_count": total_count,
        "correct": correct,
        "accuracy": round(correct / total_count, 6) if total_count else 0.0,
        "predicted_counts": predicted_counts,
        "confusion": confusion,
        "predictions": predictions,
    }


def majority_class_baseline(examples) -> dict[str, Any]:
    """The score a model gets by ignoring the prompt and always saying one word.

    Archived next to the model numbers because a 53% accuracy on this split is
    not a model result, it is the majority label -- and the comparison should
    make that impossible to misread.
    """
    counts = {label: 0 for label in LABELS}
    for example in examples:
        if example["output"] in counts:
            counts[example["output"]] += 1
    total = sum(counts.values())
    if total == 0:
        return {"label": None, "accuracy": 0.0, "counts": counts}
    label = max(counts, key=counts.get)
    return {
        "label": label,
        "accuracy": round(counts[label] / total, 6),
        "counts": counts,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", default="Qwen/Qwen2.5-Coder-1.5B-Instruct")
    parser.add_argument("--adapter", required=True)
    parser.add_argument("--dataset", required=True)
    parser.add_argument("--consent-manifest", required=True)
    parser.add_argument("--split", default="holdout")
    parser.add_argument("--output", required=True)
    parser.add_argument("--sequence-length", type=int, default=2048)
    parser.add_argument("--device", default="cuda")
    parser.add_argument("--no-4bit", action="store_true")
    args = parser.parse_args()

    import torch
    import transformers
    from peft import PeftModel

    dataset_path = Path(args.dataset)
    manifest_path = Path(args.consent_manifest)
    examples = _load_jsonl(dataset_path)
    try:
        provenance = assert_dataset_is_consented(
            json.loads(manifest_path.read_text(encoding="utf-8")), examples, args.split
        )
    except ConsentRefusal as exc:
        print(f"refusing to evaluate: {exc}", file=sys.stderr)
        return 2

    tokenizer = transformers.AutoTokenizer.from_pretrained(args.model_id)
    if tokenizer.pad_token_id is None:
        tokenizer.pad_token = tokenizer.eos_token

    load_kwargs: dict[str, Any] = {"torch_dtype": torch.bfloat16}
    if not args.no_4bit:
        load_kwargs["quantization_config"] = transformers.BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_use_double_quant=True,
            bnb_4bit_compute_dtype=torch.bfloat16,
        )
    if args.device == "cuda":
        load_kwargs["device_map"] = {"": 0}

    started = time.time()
    model = transformers.AutoModelForCausalLM.from_pretrained(args.model_id, **load_kwargs)
    model.config.use_cache = True

    print("scoring base arm...", flush=True)
    base = score_examples(model, tokenizer, examples, torch, args.sequence_length)

    print("attaching adapter...", flush=True)
    model = PeftModel.from_pretrained(model, args.adapter)
    print("scoring adapter arm...", flush=True)
    adapter = score_examples(model, tokenizer, examples, torch, args.sequence_length)

    baseline = majority_class_baseline(examples)
    report = {
        "schema_version": 1,
        "base_model": args.model_id,
        "adapter": args.adapter,
        "quantization": "bf16" if args.no_4bit else "nf4-double-bf16-compute",
        "split": args.split,
        "dataset": str(dataset_path),
        "dataset_sha256": sha256_file(dataset_path),
        "consent_manifest": str(manifest_path),
        "consent_manifest_sha256": sha256_file(manifest_path),
        "consent_provenance": provenance,
        "majority_class_baseline": baseline,
        "base": base,
        "adapter_scored": adapter,
        "delta_accuracy": round(adapter["accuracy"] - base["accuracy"], 6),
        "wall_seconds": round(time.time() - started, 3),
    }

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    summary = {
        key: value
        for key, value in report.items()
        if key not in {"base", "adapter_scored", "consent_provenance"}
    }
    summary["base"] = {
        k: v for k, v in base.items() if k != "predictions"
    }
    summary["adapter_scored"] = {
        k: v for k, v in adapter.items() if k != "predictions"
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
