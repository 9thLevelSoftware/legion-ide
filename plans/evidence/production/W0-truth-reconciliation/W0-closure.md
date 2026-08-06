# Wave 0 — Truth reconciliation closure

Date: 2026-08-05
Branch: `wave0/backlog-truth-reconciliation`
Predecessor: WS-A-D campaign closeout (2026-07-22) and the #105-#118 security wave (2026-08-03)

## Why this wave existed

The backlog and the readiness ledger were each validated only against
themselves, so they could disagree indefinitely — and did.

`xtask/src/kanban_backlog.rs` mapped an absent `status` field to `"todo"` via
`#[serde(default)]`, and `"status"` was not in `REQUIRED_TASK_FIELDS`. Sixty-five
of 146 cards had no status key at all. They read as not-started, which is
indistinguishable from a card someone triaged and deliberately marked
not-started. The gate stayed green throughout: it failed **open** in the
not-done direction.

Evidence was required only for `status = "done"`, so `in-progress` was a free
claim. Three cards (`P2.F5.T1`, `P2.F5.T2`, `P2.F5.T4`) sat in-progress with an
empty evidence field.

## Changes

| Item | Change |
| --- | --- |
| W0.1 | `status` is now `Option<String>` with no serde default and is in `REQUIRED_TASK_FIELDS`. An omitted status is a hard validation error. |
| W0.2 | Non-blank `evidence` is required for `in-progress` and `blocked`, not only `done`. `MissingEvidenceForDone` generalised to `MissingEvidenceForStatus`. |
| W0.3 | New `external_unblock` field over the closed vocabulary `EXT-CERT-WIN`, `EXT-CERT-MAC`, `EXT-CERT-LIN`, `EXT-FEED`, `EXT-PENTEST`, `EXT-LIVEKEY`, `EXT-VM`, `EXT-CORPUS`; required when `status = "blocked"`. |
| W0.4 | New gate `cargo run -p xtask -- verify-readiness-consistency` (`xtask/src/readiness_consistency.rs`) cross-reads both files and fails when the ledger describes a task in a way its backlog status contradicts. |
| W0.5 | All 66 un-statused cards triaged with evidence; `P2.F5.T4` demoted from in-progress to todo; `P3.F1.T2` contradiction resolved by running the test. |
| W0.6 | ADR-0044 collision resolved; `docs/adrs/` eliminated; `docs-hygiene` now enforces ADR location and number uniqueness. |
| W0.7 | Wasmtime supply-chain debt filed as P7's entry blocker (`W0-7-wasmtime-adr-debt.md`). |

## Backlog status: before and after

Before, as reported by the gate: 75 done, 6 in-progress, 65 unset (silently todo).

After, all explicit:

| Epic | done | in-progress | todo | blocked |
| --- | --- | --- | --- | --- |
| P0 | 11 | 0 | 0 | 0 |
| P1 | 12 | 2 | 3 | 0 |
| P2 | 14 | 6 | 3 | 0 |
| P3 | 16 | 0 | 0 | 0 |
| P4 | 15 | 0 | 0 | 0 |
| P5 | 12 | 0 | 0 | 0 |
| P6 | 12 | 0 | 2 | 0 |
| P7 | 1 | 3 | 4 | 0 |
| P8 | 12 | 2 | 1 | 1 |
| P9 | 3 | 5 | 6 | 0 |
| **Total** | **108** | **18** | **19** | **1** |

Thirty-three cards that the gate reported as outstanding were already delivered.
Nineteen are genuinely not started. One is blocked on an external gate
(`P8.F1.T3`, `EXT-VM` — fresh-VM Gatekeeper/SmartScreen evidence).

## The P3.F1.T2 contradiction, resolved by test

The PR-LANG-001 ledger row named write-side apply activation (`P3.F1.T2`) a
promotion blocker. The backlog recorded it `done` with M9 evidence. Rather than
pick a side in prose, its verification command was run:

```
cargo test -p legion-app --test apply_activation
```

13 passed, 0 failed — including `approve_and_apply_rename_proposal_applies_and_renames_file`,
which *is* the write-side apply the ledger called blocked. The backlog was
right; the ledger text was stale and has been corrected in place. The row's
status is unchanged: PR-LANG-001 remains **Substrate validated**, and its sole
remaining promotion blocker is 3-OS CI smoke.

## Corrections made to previously-asserted claims

- `P2.F5.T4` ("Keep network/auth operations policy-visible") was marked
  in-progress. No implementation backs it — no git network/auth policy
  projection exists in `legion-protocol` or `legion-desktop`. Demoted to `todo`.
- The PR-LANG-001 evidence cell's apply-activation blocker claim was stale.
  Corrected, with the test output that disproves it.
- `P1.F4.T2` and `P1.F4.T3` were initially reconciled as done, but their
  acceptance criteria require the still-missing 100MB streaming path and an
  explicit streaming projection state. Both remain `todo` until those product
  behaviors and their non-ignored verification exist.

## Verification

```
cargo test -p xtask --lib kanban_backlog          # 8 passed
cargo test -p xtask --lib readiness_consistency   # 11 passed
cargo test -p xtask --test kanban_backlog         # 19 passed
cargo test -p xtask --test docs_hygiene           # 20 passed
cargo run -p xtask -- verify-kanban-backlog       # ok: 10 epics, 38 features, 146 tasks
cargo run -p xtask -- verify-readiness-consistency # ok: 146 tasks cross-checked
cargo run -p xtask -- docs-hygiene                # passed
cargo run -p xtask -- claim-audit                 # passed
cargo run -p xtask -- check-deps                  # passed
```

Negative coverage — each new rule ships with a test proving it goes red:
omitted status, `in-progress` without evidence, `blocked` without an
`external_unblock`, an unknown `external_unblock`, a duplicate ADR number, a
misplaced ADR, a ledger calling a done task a blocker, and a ledger calling a
todo task delivered. Review follow-up adds coverage for an allowlisted archived
ADR, an unknown ledger task id, and predicates shared by coordinated task ids.

## Readiness ledger impact

**None, by design.** No row changes status in this wave. Wave 0 makes the
existing statuses earned rather than better. The standing-gate count rises from
20 to 21 with `verify-readiness-consistency`.

## Known limits

`verify-readiness-consistency` splits on clause separators (`. `, `; `, `|`) and
clamps at neighbouring task ids except coordinated lists that share a predicate;
it does not parse sentences and does not understand negation. A sentence
*reporting* that a past claim was wrong reads as making that claim — corrections
must put the task id in the resolved clause. This limit was hit while writing
the PR-LANG-001 correction above and is recorded in the module docs.
