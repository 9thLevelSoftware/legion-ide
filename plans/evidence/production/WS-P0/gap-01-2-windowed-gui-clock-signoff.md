# GAP-01.2 — Windowed GUI promotion clock sign-off

**Date:** 2026-09-02  
**Task:** GAP-01.2 T0-D clock  
**Criteria:** [`T0-D-smoke-promotion-criteria.md`](T0-D-smoke-promotion-criteria.md)  
**Does not promote** any product-readiness ledger row.  
**Does not** add `.github/workflows/legion-windowed-gui.yml` as a required check.

## The four runs

| # | Date (UTC) | Trigger | Branch | SHA | Result | Run |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2026-09-02 02:47 | dispatch | `gap/p0-windowed-gui-hosted-gpu` | `982f6789` | success | [33584539014](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33584539014) |
| 2 | 2026-09-02 03:29 | dispatch | `main` | `3d92bf35` | success | [33587226828](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33587226828) |
| 3 | 2026-09-02 10:46 | dispatch | `main` | `f2a356c3` | success | [33621158550](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33621158550) |
| 4 | 2026-09-02 12:02 | dispatch | `main` | `4e0708a4` | success | [33627790410](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33627790410) |

Each run is three jobs (ubuntu-latest, windows-latest, macos-latest). Every committed `report.toml` has `window_created = true`, `status = "passed"`, `not_beta_smoke = true`, `not_golden_path_5 = true`. GUI step hard-fail (`continue-on-error` absent). Artifacts live under [`windowed-gui-3os/`](windowed-gui-3os/).

## Criterion 1 is met as written, with the same kind of thinness the smoke clock recorded

Criterion 1 asks for four consecutive green scheduled (or fully equivalent dispatch) 3-OS runs. Four consecutive green 3-OS windowed-GUI runs exist. Facts that belong in front of the sign-off:

- **All four were `workflow_dispatch`.** None was the weekly Monday 08:00 UTC schedule.
- **Run 1 was not on `main`.** It ran on `gap/p0-windowed-gui-hosted-gpu` at the #204 merge SHA. Runs 2–4 were on `main`.
- **Four distinct SHAs**, unlike smoke runs 3 and 4 which repeated one commit.
- **All four fall inside about nine hours on 2026-09-02.** Stability-over-time is a weaker signal than four scheduled weeks.

The greens are real: a native window, open/edit/save, on three OSes. The count is a weaker stability claim than the criterion's authors probably intended.

## Criterion 2: rust-analyzer provisioning

Not applicable. `legion-windowed-gui.yml` does not provision rust-analyzer. That criterion belongs to `legion-smoke.yml` GP-1.

## Criteria 3 and 4: met on 2026-09-02 by owner sign-off

3. Maintainer acceptance of PR-path cost.
4. Written owner sign-off with run URLs and SHAs.

The owner was shown the four-run table, the caveats above, that GAP-05.3 still needs VoiceOver, that GAP-05.4 still needs a live AT-SPI dump plus Orca, and that completing this clock is not the same as adding a required check, and signed off:

> "those all look good, consider this my sign-off"

Recorded verbatim because that is what criterion 4 asks for: an owner decision, not an inference from a green tick.

## Status

- Criterion 1 — met as written; see the caveats above.
- Criterion 2 — not applicable to this workflow.
- Criterion 3 — met, owner sign-off 2026-09-02.
- Criterion 4 — met, owner sign-off recorded above.

The GAP-01.2 promotion clock is complete.

This does **not** add windowed-gui to `protect-main` (id `21950476`) and does **not** fold it into `legion-gates.yml`. Completing the clock is not the same as making the job a required check, and nothing here does that. Smoke stayed independent after its 2026-08-19 sign-off for the same reason.

GAP-05.3 and GAP-05.4 stay open. Ledger row statuses are unchanged.
