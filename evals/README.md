# Legion Evaluation Dry-Run Scaffold

`evals/run_eval.py --dry-run` records the Phase 8 evaluation contract without requiring local model weights.

The first suite covers schema compliance, proposal patch application, compile/test success, regression rate, latency/cost, and refusal rate. Real model execution must use consented and redacted trace exports only.
