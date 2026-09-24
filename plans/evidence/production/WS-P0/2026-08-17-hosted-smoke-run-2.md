# Hosted 3-OS smoke, run 2 — all 15 jobs green (P0.F4.T5)

**Date:** 2026-08-17
**Run:** https://github.com/9thLevelSoftware/legion-ide/actions/runs/31997478394
**Ref:** `main` (workflow_dispatch)
**Head:** post-#137 / post-#139

## Result

Every job succeeded. No skips, no soft passes.

| Job | ubuntu | windows | macos |
| --- | --- | --- | --- |
| GP-1 smoke | success | success | success |
| GP-2 smoke | success | success | success |
| GP-3 smoke | success | success | success |
| GP-4 smoke | success | success | success |
| Update drill | success | success | success |

Verified per-job rather than from the run's overall conclusion, because a run
can report `success` while jobs skip.

## What this is, and what it is not

**It is the second green.** `T0-D-smoke-promotion-criteria.md` requires *four
consecutive* green 3-OS runs, plus rust-analyzer provisioning success, plus
maintainer acceptance of PR-path cost, plus written owner sign-off with run
URLs. One green is one green. The preceding run
(`2026-08-15-hosted-smoke-first-run.md`) was on branch `phase-0-truth-repair`
rather than `main`, so whether the clock counts it as consecutive is an owner
call, not an author call — the criteria say "scheduled (or fully equivalent)"
and a dispatch on `main` is arguably equivalent while a dispatch on a branch is
arguably not.

**It is not a promotion.** Criteria 3 and 4 are owner actions and neither has
happened. Nothing in the ledger moves on the strength of this file.

## Why it matters anyway

`PR-LANG-001`'s evidence names 3-OS CI smoke as its **sole** remaining
promotion blocker, and Phase 1 exit depends on that row. Before today the only
data was four consecutive *failing* scheduled runs on `main` (2026-07-27,
2026-08-03, 2026-08-10, 2026-08-15) — every GP-1 and every update-drill job
failing on all three platforms — and one green run on a branch. That pattern
left two readings open: main is broken on every platform, or those runs are
stale against a tree that has since moved a long way.

This run answers it. On current `main`, all four golden paths and the update
drill pass on all three operating systems.

## Not claimed

The four earlier failures were **not** diagnosed. This run shows the current
state; it does not explain the previous one, and nobody should read "it passes
now" as "we fixed it" — no change was made to fix it. If the cause was
environmental it may recur, and the next scheduled run is the honest test of
that. Diagnosing those four is worth doing precisely because an unexplained
green is a fragile foundation for a promotion clock.

The rust-analyzer provisioning criterion (2) is not assessed here; GP-1's
success implies a working language server on all three runners, but the
criterion asks for an explicit statement and this is not one.
