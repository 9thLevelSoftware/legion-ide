# Legion review task

You are executing a Legion guided workflow step with a human in the loop.
Use the supplied context pack as the authoritative task context.

Context pack: .legion/project/changes/chg_r1-r2-requirement-coverage/reviews/rev_r1-r2-requirement-coverage-review-2/context-pack.md

## Objective

Review the collected build evidence for this task. Do not implement fixes or change files.

Task objective:
Run the escape attempts test suite to verify that all automated and AI-assisted operations are sandboxed behind trust boundaries.

## Scope

Read scope:
- .legion/project/changes/chg_r1-r2-requirement-coverage/change.yaml
- .legion/project/changes/chg_r1-r2-requirement-coverage/oracle/orc_r2-sandbox-containment.yaml

Original task write scope (review only; do not write):
- .

Forbidden scope:
- .git
- node_modules
- .legion/project
- .legion/var/runtime.sqlite

## Harness Rules

- Read before verdict.
- Inspect the context pack, build evidence, task run, executor result, and redacted logs before reporting.
- Treat recorded successful verification commands as evidence; rerun commands only when necessary and permitted by the runtime.
- Do not modify files, apply fixes, publish, release, or perform unrelated cleanup.

## Required JSON Result

Return only JSON with this shape:
```json
{
  "status": "succeeded | failed | blocked",
  "summary": "short factual review summary",
  "reviewVerdicts": {"specification": "pass", "integration": "pass", "evidence": "pass"},
  "findings": [{"id": "finding-id", "title": "Finding title", "body": "Evidence and impact", "severity": "minor | major | blocking"}],
  "filesChanged": [],
  "commandsRun": []
}
```
