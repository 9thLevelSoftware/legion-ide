# PR-4 Git dispatch perf-harness rows

Status: implemented as strict dispatch rows plus explicit report-only follow-ups.

## Strict rows

- `git.ui_dispatch_refresh` measures `CommandDispatchIntent::RefreshGit` from intent entry to return. Its p50 and p95 ceiling is 4 ms. It does not wait for a worker, drain, paint, renderer submission, or next frame.
- `git.remote_push_does_not_block_dispatch` measures a policy-denied `PushGitRemote` intent-to-return path. The fixture remote is intentionally denied, so the row proves the cheap no-egress path and does not claim an allowed remote operation or network measurement.

The product measurement binary writes both rows in the existing `product-perf.toml` workload shape. `xtask` applies the strict policies without the skeleton report-only environment override.

## Report-only follow-ups

- `git.jobs_per_refresh_burst` remains report-only in the harness. PR-2's deterministic 50-refresh test is the authoritative proof of at most two worker jobs; the product perf subprocess has no worker counter and does not infer one.
- `git.spawn_count_per_snapshot` remains deferred until the post-PR-3 typed-gix/process instrumentation exists. No process count is fabricated.
- `git.status_legion_repo` remains deferred until post-PR-3 typed-gix parity evidence. The existing product search row is not relabeled as Git status.

No renderer or process measurements are claimed by this packet. The strict rows gate dispatch return latency only; worker completion and visual paint remain separate evidence surfaces.
