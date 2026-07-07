# Task 1 Report — PKT-OPEN m11/opener

## Scope executed

Executed the M11 campaign opener exactly as a housekeeping packet:
- committed the pending M12 ledger close,
- created the M11 ledger with the required packet list and explicit P6.F4 / ACP deferral,
- pushed `main`,
- removed the approved local M12 packet branches after proving squash-parity,
- dispatched `legion-smoke`,
- wrote committed evidence plus this detailed report.

No PKT-PLAN work or feature implementation was started.

## Commits created

1. `b2ad9e0` — `docs: close M12 campaign ledger`
2. `e0e36a3` — `docs: open M11 campaign ledger`
3. `7d17d0e` — `docs: record PKT-OPEN M11 evidence`
4. `5b0579a` — `docs: repair PKT-OPEN evidence sequencing`
5. `4b9721f` — `docs: finalize PKT-OPEN repair ledger`
6. Final evidence self-reference repair commit — this commit

## Operational results

### Push

`git push origin main` succeeded:

```text
To https://github.com/9thLevelSoftware/legion-ide.git
   8c78db4..e0e36a3  main -> main
```

After writing the committed PKT-OPEN evidence, a second push published the final opener state:

```text
To https://github.com/9thLevelSoftware/legion-ide.git
   e0e36a3..7d17d0e  main -> main
```

### Branch cleanup

`git branch -d` refused both local packet branches because the work landed by squash merge, not by merged branch ancestry. I verified content parity before force deletion:

- Updater parity proof:
  - merge-base: `5ace1ea`
  - patch-id of `git diff 5ace1ea..m12/updater` matched patch-id of `git diff 5ace1ea..62bbb68`
- Crash parity proof:
  - merge-base: `62bbb68`
  - patch-id of `git diff 62bbb68..m12/crash-capture` matched patch-id of `git diff 62bbb68..02379f4`

After that proof:

```text
Deleted branch m12/updater (was ab5706c).
Deleted branch m12/crash-capture (was 53966ed).
```

### Hosted smoke dispatch

`gh workflow run .github/workflows/legion-smoke.yml --ref main` succeeded.

Observed run:
- id: `28893311632`
- URL: <https://github.com/9thLevelSoftware/legion-ide/actions/runs/28893311632>
- head SHA: `e0e36a3c26bb072662324ea0101d7e1e9fb3ab43`

At capture time the workflow was still `in_progress`, including all three `GP-3 smoke` jobs and all three `Update drill` jobs. I did not claim hosted success without completed results.

## Verification performed

- conflict-marker sweep across tracked text-relevant areas excluding `target` and `.git`
- git status before commits, after push, and before evidence write
- `gh auth status`
- workflow dispatch plus live run inspection via `gh run list` and `gh run view`

The full 19-gate local chain was intentionally skipped because this packet only changed ledgers/evidence and the brief explicitly says not to run the full chain unless necessary.

## Files written

- committed: `plans/evidence/production/M11/PKT-OPEN-evidence.md`
- committed: `.superpowers/sdd/progress-m11-campaign.md`
- committed earlier closeout: `.superpowers/sdd/progress-m12-campaign.md`
- report only: `.superpowers/sdd/task-PKT-OPEN-M11-report.md`

## Concerns

The only open concern is timing, not correctness: hosted `legion-smoke` run `28893311632` was still in progress when this report was written, so the evidence records the pending status and live run URL instead of inventing per-OS pass results.

## Fix round — review finding repair

Reviewer finding accepted: the committed PKT-OPEN evidence incorrectly let hosted smoke run `28893311632` read like validation for the final opener evidence state, but that run actually targeted intermediate SHA `e0e36a3` while the final opener evidence commit was `7d17d0e`.

### Additional commands run

```powershell
git status --short --branch
git rev-parse HEAD
gh auth status
gh run list --workflow legion-smoke.yml --branch main --limit 10 --json databaseId,workflowName,displayTitle,status,conclusion,url,createdAt,updatedAt,headSha,event,headBranch
gh run view 28893311632 --json status,conclusion,url,workflowName,jobs,createdAt,updatedAt,headSha,event,displayTitle
gh workflow run .github/workflows/legion-smoke.yml --ref main
gh run list --workflow legion-smoke.yml --branch main --limit 3 --json databaseId,workflowName,displayTitle,status,conclusion,url,createdAt,updatedAt,headSha,event,headBranch
```

### Additional observed results

- Review-fix commit created before the final ledger cleanup pass: `5b0579a` (`docs: repair PKT-OPEN evidence sequencing`).
- Second evidence-only repair commit created after that pass: `4b9721f` (`docs: finalize PKT-OPEN repair ledger`).
- `git rev-parse HEAD` confirmed the latest pushed opener SHA before the repair was `7d17d0e2ed3ad0eaaa90c4357c73d98e9b924dd4`.
- Re-inspection of run `28893311632` confirmed it was dispatched on `e0e36a3c26bb072662324ea0101d7e1e9fb3ab43`, so it only speaks to the intermediate two-commit opener state.
- That initial run was later superseded and ended `cancelled`; before cancellation it had already shown mixed intermediate-state results, including `GP-3 smoke (ubuntu-latest) = failure`, `GP-2 smoke (ubuntu-latest) = failure`, `Update drill (ubuntu-latest) = success`, and `Update drill (macos-latest) = success`.
- Corrective dispatch succeeded and created run `28893658693` at <https://github.com/9thLevelSoftware/legion-ide/actions/runs/28893658693>, targeting `7d17d0e2ed3ad0eaaa90c4357c73d98e9b924dd4`.
- At capture time, corrective run `28893658693` was `in_progress`; observed partial results already included `GP-1 smoke (ubuntu-latest) = success` and `Update drill (ubuntu-latest) = success`, with the remaining jobs still running. The repair records the exact target SHA and live status without claiming hosted success.
- This final repair round is evidence-only: it corrects audit wording, ledger chronology, and commit listings so the packet stops implying that a future evidence-only commit can be validated by a hosted run dispatched earlier.

### Files updated in the fix round

- `plans/evidence/production/M11/PKT-OPEN-evidence.md`
- `.superpowers/sdd/progress-m11-campaign.md`
- `.superpowers/sdd/task-PKT-OPEN-M11-report.md`

### Remaining limitation

The repaired evidence now distinguishes the intermediate run from the corrective post-push run against `7d17d0e`, includes the later evidence-only repair commits `5b0579a` and `4b9721f`, and states the real rule explicitly: the final packet head must be read from `git log` or `origin/main`, not from a self-referential SHA inside the evidence file. Claiming hosted validation for the final evidence-only repair commit would still require another follow-up commit and would recreate the same loop.
