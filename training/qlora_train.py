"""QLoRA training entrypoint for Legion specialist models.

Three modes, in increasing order of what they touch:

* ``--dry-run`` prints the planned job. No files are read.
* ``--fixture-smoke`` validates a dataset and writes a manifest. No heavy deps.
* real mode (heavy deps present) validates consent, then trains for
  ``--max-steps`` optimizer steps against a 4-bit NF4 base model and saves a
  LoRA adapter.

Real mode requires ``--consent-manifest``. That is not paperwork: the dataset
this trains on must come from ``cargo run -p xtask -- training-corpus``, which
routes every trace through the consent-gated pipeline landed in P9.F4.T1/T2 and
re-derives consent for every emitted line. A JSONL file on disk carries no
evidence of where it came from, so without the manifest check this script would
happily train on anything shaped like ``{"instruction": ..., "output": ...}`` --
including a hand-written file whose traces were never consented. The manifest is
the only thing tying the bytes to the gate, so real mode refuses without it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import sys
import time
from pathlib import Path
from typing import Any

#: Consent states the training corpus is permitted to retain. Mirrors
#: ``export_permits_consent`` in ``xtask/src/training_corpus.rs``; a manifest
#: naming any other state is refused rather than trained on.
CONSENTED_STATES = frozenset({"Granted", "NotRequired"})

#: Manifest schema this script knows how to validate.
SUPPORTED_MANIFEST_SCHEMA = 1

#: LoRA target modules for Qwen2/Llama-style attention + MLP blocks.
DEFAULT_TARGET_MODULES = (
    "q_proj",
    "k_proj",
    "v_proj",
    "o_proj",
    "gate_proj",
    "up_proj",
    "down_proj",
)


class ConsentRefusal(Exception):
    """Raised when a dataset cannot be shown to have come from the consent gate."""


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


def sha256_file(path: Path) -> str:
    """Content digest of a file, recorded so an archived run names its inputs."""
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def assert_dataset_is_consented(
    manifest: dict[str, Any],
    examples: list[dict[str, Any]],
    split: str = "train",
) -> dict[str, Any]:
    """Refuse a dataset that the export manifest does not vouch for.

    Checks, in the order a tampered artifact is most likely to fail them:

    1. the manifest is a schema this script understands;
    2. every consent state the corpus retained is one the gate permits -- an
       unrecognised state fails closed, so widening the gate without widening
       this list refuses rather than trains;
    3. the retained-state counts add up to the corpus candidate count, so a
       state cannot be hidden by deleting its row;
    4. the manifest's count for the split matches the number of lines actually
       present, so lines cannot be appended to the JSONL after export;
    5. every line carries a unique, non-empty ``example_id``, so an appended
       line cannot borrow an exported line's identity.

    Returns the provenance record to stamp on the training manifest.
    """
    schema = manifest.get("schema_version")
    if schema != SUPPORTED_MANIFEST_SCHEMA:
        raise ConsentRefusal(
            f"export manifest schema_version {schema!r} is not the supported "
            f"schema {SUPPORTED_MANIFEST_SCHEMA}"
        )

    retained = manifest.get("retained_consent_states")
    if not isinstance(retained, dict) or not retained:
        raise ConsentRefusal(
            "export manifest carries no retained_consent_states; the corpus "
            "cannot be shown to be consented"
        )
    unpermitted = sorted(set(retained) - CONSENTED_STATES)
    if unpermitted:
        raise ConsentRefusal(
            f"export manifest retained non-consented state(s): {unpermitted}"
        )

    candidate_count = manifest.get("candidate_count")
    retained_total = sum(retained.values())
    if retained_total != candidate_count:
        raise ConsentRefusal(
            f"export manifest retained_consent_states sum to {retained_total} "
            f"but the corpus retained {candidate_count} candidates; a consent "
            "state is unaccounted for"
        )

    expected = manifest.get(f"{split}_count")
    if expected != len(examples):
        raise ConsentRefusal(
            f"export manifest declares {expected!r} {split} example(s) but the "
            f"dataset holds {len(examples)}; the dataset was changed after export"
        )

    seen: set[str] = set()
    for index, example in enumerate(examples):
        example_id = example.get("example_id")
        if not isinstance(example_id, str) or not example_id.strip():
            raise ConsentRefusal(
                f"{split} example {index} has no example_id; it cannot be traced "
                "back to a consented candidate"
            )
        if example_id in seen:
            raise ConsentRefusal(
                f"{split} example {index} repeats example_id {example_id!r}; a "
                "line was duplicated after export"
            )
        seen.add(example_id)

    return {
        "corpus_id": manifest.get("corpus_id"),
        "corpus_fingerprint": manifest.get("corpus_fingerprint"),
        "dataset_fingerprint": manifest.get("dataset_fingerprint"),
        "prompt_template_version": manifest.get("prompt_template_version"),
        "source_trace_count": manifest.get("source_trace_count"),
        "candidate_count": candidate_count,
        "skipped_unconsented_count": manifest.get("skipped_unconsented_count"),
        "skipped_non_terminal_count": manifest.get("skipped_non_terminal_count"),
        "retained_consent_states": dict(retained),
        "legion_bench_comparison": manifest.get("comparison"),
    }


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


def render_prompt(instruction: str) -> str:
    """The exact string the model is conditioned on, trailing space included.

    Both the trainer and ``eval_adapter.py`` call this, and they have to. The
    first version of this harness let the eval build its own string: it scored
    ``instruction`` followed by ``" Accepted"``, while training had shown the
    model ``instruction + " "`` followed by ``"Accepted"``. On Qwen's tokenizer
    those are different token sequences -- ``[':', ' ', 'Accepted']`` versus
    ``[':', ' Accepted']`` -- so the eval scored two continuations the adapter
    had never been trained on, and the trained arm collapsed to a single label.
    Training loss looked fine throughout. A tokenizer boundary is not a detail
    the two sides can each decide for themselves.
    """
    return f"{instruction} "


def label_token_ids(tokenizer, label: str) -> list[int]:
    """Token ids for a decision, tokenized as the trainer tokenizes it."""
    return tokenizer(label, add_special_tokens=False)["input_ids"]


def build_supervised_batch(tokenizer, examples, sequence_length):
    """Tokenize a batch, masking the prompt so only the decision is trained.

    Loss on the prompt tokens would teach the adapter to reproduce the metadata
    it is being shown, which is both useless and the larger share of every
    sequence. Only the completion (and its EOS) carries a label.
    """
    input_ids_batch = []
    labels_batch = []
    for example in examples:
        prompt = render_prompt(example["instruction"])
        completion = f"{example['output']}{tokenizer.eos_token}"
        prompt_ids = tokenizer(prompt, add_special_tokens=False)["input_ids"]
        completion_ids = tokenizer(completion, add_special_tokens=False)["input_ids"]
        ids = (prompt_ids + completion_ids)[:sequence_length]
        labels = ([-100] * len(prompt_ids) + completion_ids)[:sequence_length]
        input_ids_batch.append(ids)
        labels_batch.append(labels)

    width = max(len(ids) for ids in input_ids_batch)
    pad_id = tokenizer.pad_token_id
    attention = []
    for index, ids in enumerate(input_ids_batch):
        padding = width - len(ids)
        attention.append([1] * len(ids) + [0] * padding)
        input_ids_batch[index] = ids + [pad_id] * padding
        labels_batch[index] = labels_batch[index] + [-100] * padding
    return input_ids_batch, attention, labels_batch


def _run_real_training(
    dataset_path: Path,
    manifest_path: Path,
    output_dir: Path,
    base_model: str,
    max_steps: int,
    learning_rate: float,
    lora_rank: int,
    sequence_length: int,
    device: str,
    batch_size: int,
    seed: int,
    load_in_4bit: bool,
) -> dict[str, Any]:
    """Train a LoRA adapter on a consented corpus and save it."""
    deps = _import_training_deps()
    torch = deps["torch"]
    transformers = deps["transformers"]
    peft = deps["peft"]

    examples = _load_jsonl(dataset_path)
    errors = _validate_dataset(examples)
    if errors:
        for err in errors:
            print(f"dataset validation error: {err}", file=sys.stderr)
        return {"valid": False, "errors": errors}

    manifest_text = manifest_path.read_text(encoding="utf-8")
    provenance = assert_dataset_is_consented(json.loads(manifest_text), examples, "train")
    print(
        "consent gate: "
        f"corpus={provenance['corpus_id']} "
        f"consented={provenance['candidate_count']} "
        f"dropped_unconsented={provenance['skipped_unconsented_count']} "
        f"dropped_non_terminal={provenance['skipped_non_terminal_count']} "
        f"states={provenance['retained_consent_states']}",
        flush=True,
    )

    random.seed(seed)
    torch.manual_seed(seed)
    if torch.cuda.is_available():
        torch.cuda.manual_seed_all(seed)

    tokenizer = transformers.AutoTokenizer.from_pretrained(base_model)
    if tokenizer.pad_token_id is None:
        tokenizer.pad_token = tokenizer.eos_token

    load_kwargs: dict[str, Any] = {"torch_dtype": torch.bfloat16}
    quantization = None
    if load_in_4bit:
        quantization = transformers.BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_use_double_quant=True,
            bnb_4bit_compute_dtype=torch.bfloat16,
        )
        load_kwargs["quantization_config"] = quantization
    if device == "cuda":
        load_kwargs["device_map"] = {"": 0}

    model = transformers.AutoModelForCausalLM.from_pretrained(base_model, **load_kwargs)
    model.config.use_cache = False
    if load_in_4bit:
        model = peft.prepare_model_for_kbit_training(model)

    lora_config = peft.LoraConfig(
        r=lora_rank,
        lora_alpha=lora_rank * 2,
        lora_dropout=0.05,
        bias="none",
        task_type="CAUSAL_LM",
        target_modules=list(DEFAULT_TARGET_MODULES),
    )
    model = peft.get_peft_model(model, lora_config)
    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total = sum(p.numel() for p in model.parameters())
    print(f"trainable params: {trainable} / {total}", flush=True)

    optimizer = torch.optim.AdamW(
        [p for p in model.parameters() if p.requires_grad], lr=learning_rate
    )
    scheduler = transformers.get_linear_schedule_with_warmup(
        optimizer,
        num_warmup_steps=max(1, max_steps // 10),
        num_training_steps=max_steps,
    )

    order = list(range(len(examples)))
    random.shuffle(order)
    cursor = 0
    loss_curve: list[dict[str, float]] = []
    model.train()
    started = time.time()

    for step in range(max_steps):
        batch_indices = []
        while len(batch_indices) < batch_size:
            if cursor >= len(order):
                random.shuffle(order)
                cursor = 0
            batch_indices.append(order[cursor])
            cursor += 1
        batch = [examples[i] for i in batch_indices]
        input_ids, attention, labels = build_supervised_batch(
            tokenizer, batch, sequence_length
        )
        device_obj = next(model.parameters()).device
        outputs = model(
            input_ids=torch.tensor(input_ids, device=device_obj),
            attention_mask=torch.tensor(attention, device=device_obj),
            labels=torch.tensor(labels, device=device_obj),
        )
        outputs.loss.backward()
        torch.nn.utils.clip_grad_norm_(
            [p for p in model.parameters() if p.requires_grad], 1.0
        )
        optimizer.step()
        scheduler.step()
        optimizer.zero_grad(set_to_none=True)

        loss_value = float(outputs.loss.detach().item())
        loss_curve.append({"step": step + 1, "loss": round(loss_value, 6)})
        if (step + 1) % 10 == 0 or step == 0:
            print(f"step {step + 1}/{max_steps} loss={loss_value:.4f}", flush=True)

    elapsed = time.time() - started
    output_dir.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(output_dir))
    tokenizer.save_pretrained(str(output_dir))

    peak_vram_mib = None
    if torch.cuda.is_available():
        peak_vram_mib = int(torch.cuda.max_memory_allocated() // (1024 * 1024))

    window = max(1, max_steps // 10)
    plan = {
        "mode": "real",
        "valid": True,
        "trained": True,
        "dataset": str(dataset_path),
        "dataset_sha256": sha256_file(dataset_path),
        "consent_manifest": str(manifest_path),
        "consent_manifest_sha256": sha256_file(manifest_path),
        "consent_provenance": provenance,
        "example_count": len(examples),
        "base_model": base_model,
        "load_in_4bit": load_in_4bit,
        "quantization": "nf4-double-bf16-compute" if load_in_4bit else "bf16",
        "max_steps": max_steps,
        "batch_size": batch_size,
        "learning_rate": learning_rate,
        "lora_rank": lora_rank,
        "lora_alpha": lora_rank * 2,
        "lora_target_modules": list(DEFAULT_TARGET_MODULES),
        "sequence_length": sequence_length,
        "seed": seed,
        "device": device,
        "output_dir": str(output_dir),
        "trainable_parameters": trainable,
        "total_parameters": total,
        "first_loss": loss_curve[0]["loss"] if loss_curve else None,
        "final_loss": loss_curve[-1]["loss"] if loss_curve else None,
        "mean_loss_first_decile": round(
            sum(row["loss"] for row in loss_curve[:window]) / window, 6
        )
        if loss_curve
        else None,
        "mean_loss_last_decile": round(
            sum(row["loss"] for row in loss_curve[-window:]) / window, 6
        )
        if loss_curve
        else None,
        "loss_curve": loss_curve,
        "wall_seconds": round(elapsed, 3),
        "peak_vram_mib": peak_vram_mib,
        "deps_present": {
            "torch": deps["torch"].__version__,
            "transformers": deps["transformers"].__version__,
            "peft": deps["peft"].__version__,
            "datasets": deps["datasets"].__version__,
            "trl": deps["trl"].__version__,
        },
    }
    _write_manifest(output_dir, plan)
    print(json.dumps({k: v for k, v in plan.items() if k != "loss_curve"}, indent=2, sort_keys=True))
    return plan


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
    """Validate deps and the dataset without training. Used when max-steps <= 0."""
    deps = _import_training_deps()
    examples = _load_jsonl(dataset_path)
    errors = _validate_dataset(examples)
    if errors:
        for err in errors:
            print(f"dataset validation error: {err}", file=sys.stderr)
        return {"valid": False, "errors": errors}

    plan = {
        "mode": "real",
        "valid": True,
        "trained": False,
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
            "operator args (--max-steps > 0, --device cuda, and "
            "--consent-manifest pointing at the xtask training-corpus export)."
        ),
    }
    _write_manifest(output_dir, plan)
    print(json.dumps(plan, indent=2, sort_keys=True))
    return plan


def main() -> int:
    """Validate a training request and run or plan the job."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", default="Qwen/Qwen2.5-Coder-1.5B-Instruct")
    parser.add_argument("--base-model", default="")
    parser.add_argument("--dataset", default="datasets/legion-traces.jsonl")
    parser.add_argument(
        "--consent-manifest",
        default="",
        help=(
            "export_manifest.json written by `cargo run -p xtask -- "
            "training-corpus`. Required for a real training run."
        ),
    )
    parser.add_argument("--output-dir", default="training/out/docs-summarizer")
    parser.add_argument("--specialist", default="docs-summarizer")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--max-steps", type=int, default=0)
    parser.add_argument("--learning-rate", type=float, default=2e-4)
    parser.add_argument("--lora-rank", type=int, default=16)
    parser.add_argument("--sequence-length", type=int, default=2048)
    parser.add_argument("--batch-size", type=int, default=4)
    parser.add_argument("--seed", type=int, default=20260819)
    parser.add_argument(
        "--no-4bit",
        action="store_true",
        help="Train in bf16 instead of 4-bit NF4 (drops the Q from QLoRA).",
    )
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

    if args.max_steps <= 0:
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
        print(
            "\nSkipping training run because --max-steps <= 0. "
            "To run training, set a positive --max-steps.",
            file=sys.stderr,
        )
        return 0

    if not args.consent_manifest:
        print(
            "refusing to train: --consent-manifest is required for a real run.\n"
            "Produce a consented dataset with:\n"
            "  cargo run -p xtask -- training-corpus --out target/training-flywheel\n"
            "then pass --consent-manifest target/training-flywheel/export_manifest.json",
            file=sys.stderr,
        )
        return 2
    manifest_path = Path(args.consent_manifest)
    if not manifest_path.exists():
        raise SystemExit(f"consent manifest does not exist: {manifest_path}")

    try:
        plan = _run_real_training(
            dataset,
            manifest_path,
            Path(args.output_dir),
            base_model,
            args.max_steps,
            args.learning_rate,
            args.lora_rank,
            args.sequence_length,
            args.device,
            args.batch_size,
            args.seed,
            not args.no_4bit,
        )
    except ConsentRefusal as exc:
        print(f"refusing to train: {exc}", file=sys.stderr)
        return 2
    return 0 if plan.get("valid") else 2


if __name__ == "__main__":
    raise SystemExit(main())
