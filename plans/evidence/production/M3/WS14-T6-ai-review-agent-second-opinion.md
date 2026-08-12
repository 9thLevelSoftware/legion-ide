# WS14.T6 AI Review Agent Second-Opinion Evidence

Date: 2026-06-13
Kanban card: `t_fda21263`
Scope: reviewer-specialist fixture and offline eval harness for seeded bug detection

## Verdict
Implemented a deterministic reviewer eval path that flags a seeded bad patch in a fixture dataset.

## Changes made in this run
- `evals/run_eval.py`
  - Added `--reviewer-fixture` mode.
  - Added reviewer-specific metrics for seeded-bug detection, high-risk labeling, and needs-human escalation.
  - Added shared rate helpers to keep the eval summaries consistent.
- `evals/fixtures/reviewer_seeded_bug.jsonl`
  - Added a three-example reviewer fixture, including one seeded bad patch that must be flagged as high-risk.
- `evals/test_run_eval.py`
  - Added unittest coverage for the reviewer fixture and CLI output path.
- `evals/README.md`
  - Documented the reviewer fixture command and its expected behavior.

## Verification
- `python3 -m unittest evals.test_run_eval` ✅
- `python3 evals/run_eval.py --reviewer-fixture --dataset evals/fixtures/reviewer_seeded_bug.jsonl --output /tmp/legion-reviewer-eval.json` ✅
- `python3 evals/run_eval.py --dry-run` ✅
- `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json` ✅
- `python3 -m compileall evals` ✅

## Evidence notes
- The seeded bug example is intentionally labeled `high-risk`; the evaluator reports a `seeded_bug_detection_rate` of 1.0 for the fixture.
- The reviewer path remains deterministic and offline-safe, so it can serve as the future scoring harness for a live reviewer model without requiring heavy dependencies.
