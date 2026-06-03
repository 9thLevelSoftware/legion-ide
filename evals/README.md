# Legion Evaluation Harness

`evals/run_eval.py` records the Phase 8 evaluation contract and can run in multiple modes:

## Dry-run (CI-safe)

```sh
python3 evals/run_eval.py --dry-run
```

## Offline fixture mode (no network)

```sh
python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json
```

## Endpoint mode (optional, timeout-bounded)

```sh
python3 evals/run_eval.py --endpoint http://localhost:8000 --model Qwen/Qwen2.5-Coder-1.5B-Instruct --dataset evals/fixtures/minimal.jsonl --max-examples 3 --output /tmp/legion-eval.json
```

Endpoint mode reads an API key from `OPENAI_API_KEY` or `LEGION_API_KEY`.

The first suite covers schema compliance, proposal patch application, compile/test success, regression rate, latency/cost, and refusal rate. Real model execution must use consented and redacted trace exports only.
