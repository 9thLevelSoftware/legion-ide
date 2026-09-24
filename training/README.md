# Legion Training Harness

Training is opt-in and consent-gated. The checked-in Python entrypoints validate
and print operator plans without importing GPU training libraries by default.

A real training run has one supported input: the dataset written by
`cargo run -p xtask -- training-corpus`, which routes every `(audit, proposal)`
trace through the consent-gated pipeline in `legion_observability::training`
(P9.F4.T1/T2) and re-derives consent for every line it emits. `qlora_train.py`
refuses to train without the matching `export_manifest.json`, because a JSONL
file on disk carries no evidence of where it came from.

## Dry-run commands

```sh
python3 training/qlora_train.py --dry-run
python3 training/convert_to_gguf.py --dry-run
```

## Fixture smoke tests (CI-safe, no heavy deps)

```sh
python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train-smoke
python3 training/convert_to_gguf.py --fixture-smoke --model-dir /tmp/legion-train-smoke --output /tmp/legion-model.gguf --metadata-output /tmp/legion-gguf.json
```

## Consent-gate tests (no heavy deps)

```sh
python3 -m unittest training.test_qlora_train
```

## Real training (heavy deps, GPU)

Step 1 — export a consented corpus. Nothing else is a supported training input:

```sh
cargo run -p xtask -- training-corpus --expand 1200 --out target/training-flywheel
```

This writes `train.jsonl`, `holdout.jsonl`, and `export_manifest.json`. The
manifest records the corpus and dataset fingerprints, the consent states of every
retained candidate, how many traces the consent filter dropped, and the
comparison against the archived Legion-Bench baseline.

Step 2 — train a LoRA adapter on 4-bit NF4 base weights:

```sh
python3 training/qlora_train.py \
  --model-id Qwen/Qwen2.5-Coder-1.5B-Instruct \
  --dataset target/training-flywheel/train.jsonl \
  --consent-manifest target/training-flywheel/export_manifest.json \
  --output-dir target/training-flywheel/adapter \
  --max-steps 200 --batch-size 8 --sequence-length 256 \
  --learning-rate 2e-4 --lora-rank 16 --seed 20260819 --device cuda
```

Omitting `--consent-manifest`, or pointing it at a manifest whose corpus retained
a `Denied`, `Missing`, or `RenewalRequired` candidate, exits 2 without loading a
model. So does a `train.jsonl` whose line count no longer matches the manifest.

Step 3 — score the base model and the adapter on the withheld holdout split:

```sh
python3 training/eval_adapter.py \
  --model-id Qwen/Qwen2.5-Coder-1.5B-Instruct \
  --adapter target/training-flywheel/adapter \
  --dataset target/training-flywheel/holdout.jsonl \
  --consent-manifest target/training-flywheel/export_manifest.json \
  --output target/training-flywheel/adapter_vs_base.json
```

Both arms run in one process against the same quantized base weights, so the
adapter is the only difference between them. The report includes the
majority-class accuracy of the split, because on a near-balanced binary task a
raw accuracy number is easy to misread.

Step 4 — pair the run with the Legion-Bench baseline. An unpaired training run
is not a result:

```sh
cargo run -p xtask -- legion-bench --mode recorded
cargo run -p xtask -- verify-legion-bench
```

If `torch`, `transformers`, `peft`, `datasets`, or `trl` are missing, the script
exits with exact install instructions:

```sh
pip install torch transformers peft datasets trl
```

With `--max-steps 0` (the default) real mode validates dependencies and the
dataset, writes a plan manifest, and stops without training.

## Real GGUF conversion (optional)

```sh
python3 training/convert_to_gguf.py --model-dir /tmp/legion-train --output /tmp/legion-model.gguf --llama-cpp-convert-script /path/to/convert_hf_to_gguf.py --quantize-command /path/to/llama-quantize --metadata-output /tmp/legion-gguf.json
```

Real training requires an operator-provisioned environment, consented trace
exports, redaction/secret-scan evidence, and an explicit model run record.
