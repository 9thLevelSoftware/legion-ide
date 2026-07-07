# PKT-OPEN Evidence — M11 Opener

Branch: `main`
Date: 2026-07-07
Packet: PKT-OPEN (campaign opener / post-M12 housekeeping)

## Summary

This packet performed the required M11 opener housekeeping only:
- closed the M12 campaign ledger,
- created the M11 campaign ledger with the explicit P6.F4 / ACP deferral,
- pushed local `main` to `origin`,
- cleaned the two approved local M12 packet branches,
- dispatched `legion-smoke` on `main`,
- later required a review-fix because the first hosted smoke dispatch targeted the intermediate ledger commit `e0e36a3`, not the final opener evidence commit `7d17d0e`,
- records both the mis-sequenced run and the corrective post-push dispatch against the then-latest pushed `main` SHA honestly.

No PKT-PLAN or feature implementation work was started.

## Deliverables

### D1: M12 ledger close committed

Commit: `b2ad9e0` — `docs: close M12 campaign ledger`

` .superpowers/sdd/progress-m12-campaign.md` was committed with the final PKT-CRASH closeout detail and the explicit M12 campaign-complete section.

### D2: M11 campaign ledger created

Commit: `e0e36a3` — `docs: open M11 campaign ledger`

Created `.superpowers/sdd/progress-m11-campaign.md` in the ignored `.superpowers/sdd/` tree via force-add, using the existing campaign-ledger format:
- packet checklist,
- completion-log skeleton,
- explicit note that P6.F4 / ACP interop remains deferred by user decision on 2026-07-07.

### D3: Push result

Command:

```powershell
git push origin main
```

Result:

```text
To https://github.com/9thLevelSoftware/legion-ide.git
   8c78db4..e0e36a3  main -> main
```

Post-push branch state immediately after the push:

```text
## main...origin/main
```

### D4: Local M12 branch cleanup

Target branches:
- `m12/updater`
- `m12/crash-capture`

Safe-delete attempt with `git branch -d` was rejected for both branches because the packet branches were squash-merged rather than merged by exact commit ancestry:

```text
error: the branch 'm12/updater' is not fully merged
error: the branch 'm12/crash-capture' is not fully merged
```

Force deletion was justified only after proving the packet diffs were already represented on `main`:
- `git merge-base main m12/updater` = `5ace1ea` and `git diff 5ace1ea..m12/updater | git patch-id --stable` matched `git diff 5ace1ea..62bbb68 | git patch-id --stable`
- `git merge-base main m12/crash-capture` = `62bbb68` and `git diff 62bbb68..m12/crash-capture | git patch-id --stable` matched `git diff 62bbb68..02379f4 | git patch-id --stable`

Deletion commands:

```powershell
git branch -D m12/updater
git branch -D m12/crash-capture
```

Result:

```text
Deleted branch m12/updater (was ab5706c).
Deleted branch m12/crash-capture (was 53966ed).
* main
```

### D5: Initial hosted `legion-smoke` dispatch (intermediate state only)

Dispatch command:

```powershell
gh workflow run .github/workflows/legion-smoke.yml --ref main
```

Auth prerequisite check:

```powershell
gh auth status
```

Observed run:
- Workflow: `Legion Smoke`
- Run id: `28893311632`
- Event: `workflow_dispatch`
- Head SHA: `e0e36a3c26bb072662324ea0101d7e1e9fb3ab43`
- URL: <https://github.com/9thLevelSoftware/legion-ide/actions/runs/28893311632>
- Created at: `2026-07-07T19:35:52Z`

Re-inspected status during the review-fix round:
- Workflow status: `completed`
- Workflow conclusion: `cancelled`
- Confirmed target: this run is pinned to intermediate SHA `e0e36a3`, so it cannot be used as hosted validation for final opener evidence commit `7d17d0e`
- Observed job state at re-inspection:
  - `GP-3 smoke (ubuntu-latest)` — `failure`
  - `GP-3 smoke (windows-latest)` — `cancelled`
  - `GP-3 smoke (macos-latest)` — `cancelled`
  - `Update drill (ubuntu-latest)` — `success`
  - `Update drill (windows-latest)` — `cancelled` (work step itself had already succeeded before cancellation during post-steps)
  - `Update drill (macos-latest)` — `success`

This run remains part of the historical record, but only for the intermediate two-commit opener state ending at `e0e36a3`.

### D6: Corrective hosted `legion-smoke` dispatch against the latest pushed opener SHA

Review finding: the initial evidence implied run `28893311632` validated the final opener state, but the final committed opener evidence actually landed later at `7d17d0e`.

Corrective dispatch command:

```powershell
gh workflow run .github/workflows/legion-smoke.yml --ref main
```

Observed corrective run:
- Workflow: `Legion Smoke`
- Run id: `28893658693`
- Event: `workflow_dispatch`
- Head SHA: `7d17d0e2ed3ad0eaaa90c4357c73d98e9b924dd4`
- URL: <https://github.com/9thLevelSoftware/legion-ide/actions/runs/28893658693>
- Created at: `2026-07-07T19:41:45Z`

Status snapshot at corrective-capture time:
- Workflow status: `in_progress`
- Partial observed results:
  - `GP-1 smoke (ubuntu-latest)` — `success`
  - `Update drill (ubuntu-latest)` — `success`
  - `Update drill (windows-latest)` — `in_progress`
  - `Update drill (macos-latest)` — `in_progress`
  - remaining `GP-1` / `GP-2` / `GP-3` jobs — `in_progress`

This corrective run is the hosted smoke record that actually targets the latest pushed opener state before this review-fix commit.

### D7: PKT-OPEN ledger completion update

`.superpowers/sdd/progress-m11-campaign.md` was updated to mark `PKT-OPEN` complete and record the housekeeping outcomes plus the pending hosted-smoke status honestly.

## Verification

Commands run:

```powershell
rg -n "<<<<<<<|=======|>>>>>>>" --glob '!target/**' --glob '!.git/**'
git status --short --branch
gh auth status
git push origin main
gh workflow run .github/workflows/legion-smoke.yml --ref main
gh run list --workflow legion-smoke.yml --branch main --limit 5 --json databaseId,workflowName,displayTitle,status,conclusion,url,createdAt,headSha,event
gh run view 28893311632 --json status,conclusion,url,workflowName,jobs,createdAt,updatedAt,headSha,event,displayTitle
git rev-parse HEAD
gh workflow run .github/workflows/legion-smoke.yml --ref main
gh run list --workflow legion-smoke.yml --branch main --limit 3 --json databaseId,workflowName,displayTitle,status,conclusion,url,createdAt,updatedAt,headSha,event,headBranch
```

Notes:
- The conflict-marker sweep returned only expected test/assertion fixtures and code that intentionally references conflict markers; no live merge artifact was found.
- The full 19-gate local chain was intentionally skipped because this packet is documentation/housekeeping only, introduces no product-code changes, and the brief explicitly treats `legion-smoke` as an independent, non-blocking validation surface.

## Current opener state and self-reference limitation

- Working branch: `main`
- Latest pushed opener SHA before this review-fix commit: `7d17d0e` (`docs: record PKT-OPEN M11 evidence`)
- Corrective hosted smoke dispatch now recorded against that pushed SHA via run `28893658693`
- This committed evidence repair cannot also include a hosted run for its own future commit SHA without another follow-up commit, because the run ID and target SHA are only knowable after push. No such impossible certainty is claimed here.

## Files changed by this packet

- `.superpowers/sdd/progress-m12-campaign.md`
- `.superpowers/sdd/progress-m11-campaign.md`
- `plans/evidence/production/M11/PKT-OPEN-evidence.md`

## Concerns

- Initial hosted smoke run `28893311632` is real but only validates intermediate opener SHA `e0e36a3`; it later finished `cancelled` after the corrective dispatch superseded it.
- Corrective hosted smoke run `28893658693` targets the latest pushed opener SHA `7d17d0e`, but it was still `in_progress` at capture time.
- The repair commit that carries this corrected evidence necessarily post-dates corrective run target `7d17d0e`; a committed file cannot cite a future run against its own as-yet-unpushed SHA without another follow-up commit.
