# Legion Training Harness

Phase 8 keeps training opt-in and consent-gated. The checked-in Python entrypoints validate and print operator plans without importing GPU training libraries by default.

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

## Real training (optional, heavy deps)

```sh
python3 training/qlora_train.py --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train --max-steps 10 --device cuda
```

If `torch`, `transformers`, `peft`, `datasets`, or `trl` are missing, the script exits with exact install instructions:

```sh
pip install torch transformers peft datasets trl
```

Real mode validates dependencies and builds a training plan/manifest. It does **not** start a long GPU run unless you explicitly pass a positive `--max-steps`.

## Real GGUF conversion (optional)

```sh
python3 training/convert_to_gguf.py --model-dir /tmp/legion-train --output /tmp/legion-model.gguf --llama-cpp-convert-script /path/to/convert_hf_to_gguf.py --quantize-command /path/to/llama-quantize --metadata-output /tmp/legion-gguf.json
```

Real training requires an operator-provisioned environment, consented trace exports, redaction/secret-scan evidence, and an explicit model run record.
