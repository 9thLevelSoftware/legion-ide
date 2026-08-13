# Legion-on-Legion Weekly Dogfood Journal

Use this template for weekly dogfood runs where Legion is used to develop itself.

## Instructions

1. Copy this template to `plans/evidence/dogfood/YYYY-MM-DD-dogfood-journal.md`.
2. Fill in every field. If a field does not apply, write "N/A" with a reason.
3. Run the dogfood workflow using the current build on the named branch/commit.
4. Record evidence paths for any failures or blockers.

## Template

```
# Dogfood Journal — YYYY-MM-DD

## Session

- **Branch:**
- **Commit SHA:**
- **OS / Platform:**
- **Build method:** (local cargo build / packaged installer / other)
- **Legion version / channel:**

## Workflow Attempted

Describe the workflow tried during this session (e.g., edit Rust code, use LSP completion, run tests, review git diff, use Assist for a refactor, delegate a task).

## Modes Used

- [ ] Manual
- [ ] Assist
- [ ] Delegate
- [ ] Legion Workflows

## Evidence

| Item | Path / Description |
| --- | --- |
| Screenshots | |
| Terminal output | |
| Test results | |
| Logs / traces | |

## Result

- **Outcome:** (success / partial / blocked)
- **What worked:**
- **What failed:**
- **Blockers encountered:**

## Product-Readiness Impact

Does this session change any product-readiness claim? If so, which row and what evidence?

## Follow-Up

- [ ] Issues filed:
- [ ] Fixes needed:
- [ ] Ledger updates:
```
