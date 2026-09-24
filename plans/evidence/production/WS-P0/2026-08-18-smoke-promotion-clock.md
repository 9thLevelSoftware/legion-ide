# Smoke promotion clock — all four criteria met, with a caveat worth reading

**Date:** 2026-08-18
**Task:** P0.F4.T5 — activate hosted 3-OS `legion-smoke.yml` runs and start the promotion clock
**Criteria:** `plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md`

## The four runs

| # | Date (UTC) | Trigger | Branch | SHA | Result | Run |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-08-17 05:19 | dispatch | `main` | `4c69fd22` | success | [31997478394](https://github.com/9thLevelSoftware/legion-ide/actions/runs/31997478394) |
| 2 | 2026-08-17 07:07 | **schedule** | `main` | `f58a949c` | success | [32004443484](https://github.com/9thLevelSoftware/legion-ide/actions/runs/32004443484) |
| 3 | 2026-08-18 15:11 | dispatch | `main` | `f02dc569` | success | [32152897451](https://github.com/9thLevelSoftware/legion-ide/actions/runs/32152897451) |
| 4 | 2026-08-18 18:26 | dispatch | `main` | `f02dc569` | success | [32171015225](https://github.com/9thLevelSoftware/legion-ide/actions/runs/32171015225) |

Run 4 was green on all 15 jobs: GP-1 through GP-4 and the update drill, on each
of windows-latest, macos-latest and ubuntu-latest.

The last failing scheduled run was 2026-08-10; before that the record was four
consecutive *failing* scheduled runs on `main` (2026-07-13 through 2026-08-10).

## Criterion 1 is literally met and thinner than it looks

Criterion 1 asks for "four consecutive green scheduled (or fully equivalent)
3-OS smoke runs". Four consecutive green 3-OS runs on `main` exist. Three facts
about them belong in front of whoever signs this off, because a table of four
green rows overstates what was actually demonstrated:

- **Runs 3 and 4 are the same commit.** `f02dc569` was tested twice. The four
  runs cover three distinct commits, not four.
- **Three of the four were dispatched by hand**, one of them (run 4) explicitly
  to advance this clock. Only run 2 was scheduled.
- **All four fall inside about 37 hours.** The criterion's purpose is confidence
  that the smoke suite is stable enough to block merges; stability is a claim
  about time, and 37 hours of a mostly-static tree is weak evidence for it.

None of that makes the runs invalid — they are real 3-OS runs on `main`, and the
green is genuine. It makes the *count* a weaker signal than the criterion's
authors probably intended, and saying so is cheaper than discovering it after
smoke becomes a merge blocker.

**Recommendation:** treat the next scheduled run (2026-08-24) as the fourth
data point rather than run 4. That yields four greens over eight days across
four distinct commits, two of them scheduled, which is what the criterion is
trying to buy. The cost of waiting is one week; the cost of promoting a flaky
gate is every PR after it.

## Criterion 2: rust-analyzer provisioning

Met, and verified rather than assumed. GP-1 on run 4 provisioned the real
component and initialised a live session:

```
Provision rust-analyzer: info: downloading component rust-analyzer
[s2] rust-analyzer: /home/runner/.cargo/bin/rust-analyzer version=Some("rust-analyzer 1.97.1 (8bab26f 2026-07-14)")
[s2] passed — rust-analyzer session live (63ms)
golden-path-1: smoke passed
```

This was checked against the log rather than inferred from the job's green tick,
because a soft-skip branch reporting `ok` is the failure mode that held
P2.F3.T2 open for two days. No skip branch was taken here on any platform.

## Criteria 3 and 4: met on 2026-08-19 by owner sign-off

3. Maintainer acceptance of PR-path cost.
4. Written owner sign-off with run URLs and SHAs.

Both are owner decisions, and this file supplies the run URLs and SHAs that
criterion 4 requires. As of the 2026-08-18 revision neither was met; that
changed the following day.

The owner was shown the state above in full — including the caveat that the
four green runs are thinner than the count suggests, and the standing
recommendation to treat the 2026-08-24 scheduled run as the genuine fourth
data point — and signed off anyway:

> "That's enough for me. Consider it signed off."

Recorded verbatim because that is what criterion 4 asks for: an owner decision,
not an inference from a green tick. The caveat was not withdrawn and is not
retracted here; the owner accepted the evidence as it stands rather than
disputing its weight. A reader who later wants the stronger version of this
record should look for a scheduled run on a distinct SHA after 2026-08-24.

One further data point exists that the sections above predate: a hand-dispatched
3-OS smoke run against `main` at the merge of #166 and #167, dispatched while
recording this sign-off. It is a fifth observation on a distinct SHA, not a
substitute for a scheduled one.

## Status

- Criterion 1 — met as written; see the caveat above before relying on it.
- Criterion 2 — met and verified from logs.
- Criterion 3 — met, owner accepted PR-path cost (2026-08-19).
- Criterion 4 — met, owner sign-off recorded above (2026-08-19).

All four criteria are met, so the smoke promotion clock is complete and
P0.F4.T5 is done.

Two things this does **not** do, both worth stating because the tempting reading
is wrong. It does not move `PR-UI-001` to *Product workflow validated*: that
row carries its own bar in the readiness ledger — current native platform
accessibility and focus evidence across supported OSes — and that evidence does
not exist. And it does not add the smoke to branch protection; promoting the
clock is not the same as making the smoke a required check, and nothing here
does that.

ADR-0046 Amendment 1 (2026-08-19) lifts the freeze for two named tasks on the
owner's direct instruction rather than on this sign-off. The distinction is
recorded there.
