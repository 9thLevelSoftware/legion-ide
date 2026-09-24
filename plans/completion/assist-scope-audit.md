# Assist scope audit

Inventory slice: `COMP-SCOPE-FAMILY-12` replacement, baseline `9fc9bdb758072d578a2de1d49d068556743f1838`.

The former single placeholder titled `Assist` was replaced by 9 Assist-specific integration and workflow requirements (`COMP-SCOPE-FAMILY-12-04` through `-13`, excluding the canonical setup/runtime/context/usage owners). The slice covers the approved feature-family promise (completion/ghost text, explanations, inline and multi-file changes, progressive feedback, review/apply/reject, and cancellation), with canonical provider/context requirements referenced as dependencies rather than duplicated.

## Counts and invariants

| Check | Result |
|---|---:|
| Baseline requirements | 316 |
| Retained non-family rows | 315 |
| Assist rows after replacement | 9 |
| Final requirements | 324 |
| Assist acceptance values | 9 `unassessed` |
| Duplicate requirement IDs | 0 |
| New duplicate legacy IDs | 0 |
| Retained baseline duplicate pairs | 2 |
| Assist legacy IDs | 0 (the placeholder had none; existing P3/P4 IDs remain on their original rows) |

All retained rows are field-equal to baseline after excluding the replaced family row. The JSON was written as UTF-8 and the replacement was spliced into the baseline object span so unrelated compact source formatting was preserved.

## Assist atomic inventory

| ID | Package | Implementation classification | Scope |
|---|---|---|---|
| `COMP-SCOPE-FAMILY-12-04` | S3-05 | partial | Real-route completion/ghost text and proposal mediation |
| `COMP-SCOPE-FAMILY-12-05` | S3-05 | partial | Explanations and assistant-rail commands |
| `COMP-SCOPE-FAMILY-12-06` | S3-05 | partial | Anchored inline edits and stale/ambiguous failure |
| `COMP-SCOPE-FAMILY-12-07` | S3-05 | partial | Multi-file proposal sets without pre-approval writes |
| `COMP-SCOPE-FAMILY-12-08` | S3-05 | partial | Progressive feedback, cancellation, retry |
| `COMP-SCOPE-FAMILY-12-09` | S3-05 | partial | Whole-file/per-hunk review, apply, reject, edit, undo |
| `COMP-SCOPE-FAMILY-12-10` | S3-05 | partial | Checkpoints, conflict/restart/provider-loss recovery |
| `COMP-SCOPE-FAMILY-12-11` | S3-05 | absent | Durable session create/resume/rename/delete/export |
| `COMP-SCOPE-FAMILY-12-13` | S3-07 | absent | Packaged native local/hosted Rust, TS/JS, Python acceptance and recovery |

`partial` means current source contains a substrate or narrower path that is relevant to the outcome, while the approved full workflow remains incomplete. `absent` means the approved workflow or acceptance proof is not present in the inspected current source. Neither classification promotes implementation to acceptance; all acceptance remains `unassessed`.

## Source and legacy-ID handling

Every row cites literal paths that exist in the workspace, anchored to the approved specification, `docs/superpowers/plans/2026-09-04-ai-team-completion.md`, and `docs/superpowers/plans/2026-09-04-completion-traceability.md`, with current source/evidence paths for the relevant substrate. Existing legacy IDs for P3 mutation/proposal/checkpoint/risk and P4 assistant-rail rows were retained on their original requirements; they were not copied into this family expansion. The baseline already contains two duplicate legacy-ID pairs, reproduced exactly and untouched:

| Legacy ID | Existing rows |
|---|---|
| `P1.F3.T2` | `COMP-P1-F3-T2-1`, `COMP-P1-F3-T2-1-02` |
| `P6.F5.T1` | `COMP-P6-F5-T1-1`, `COMP-P6-F5-T1-1-02` |

No new family row claims either legacy ID or any other retained legacy ID; the new family has no fabricated legacy identity.

## Canonical owner and dependency map

The removed setup/runtime/context/usage placeholders are already owned canonically by the following retained rows and are represented as dependencies where Assist consumes them:

| Assist row | Canonical owner rows | Exact Assist dependency rows |
|---|---|---|
| `12-04` completion | `COMP-PROV-001` (`S3-02`, `luna_worker`), `COMP-PROV-004` (`S3-01`, `luna_worker`), `COMP-CTX-001` (`S3-04`, `luna_worker`), `COMP-P4-F3-T1-1` (`S3-05`, `luna_worker`) | `COMP-PROV-001`, `COMP-PROV-004`, `COMP-CTX-001`, `COMP-P4-F3-T1-1` |
| `12-05` explanations/rail | `COMP-PROV-001`, `COMP-PROV-004`, `COMP-CTX-008` (`S3-04`, `luna_worker`), `COMP-P4-F3-T2-1` (`S3-05`, `luna_worker`) | same four rows |
| `12-06` inline edits | `COMP-P3-F2-T1-1..T2-1` (`S3-05`, `luna_worker`), `COMP-CTX-006` (`S3-04`, `luna_worker`) | `COMP-P3-F2-T1-1`, `COMP-P3-F2-T2-1`, `COMP-CTX-006` |
| `12-07` multi-file proposals | `COMP-P3-F2-T1-1..T2-1` (`S3-05`, `luna_worker`) | those two rows |
| `12-08` streaming/cancel | `COMP-PROV-004`, `COMP-PROV-006` (`S3-02`, `luna_worker`) and `COMP-P4-F3-T3-1` (`S3-05`, `luna_worker`) | `COMP-PROV-004`, `COMP-PROV-006` |
| `12-09` review | `COMP-P3-F2-T1-1..T4-1` (`S3-05`, `luna_worker`) | all four rows |
| `12-10` recovery | `COMP-P3-F3-T1-1..T4-1` (`S3-05`, `luna_worker`) | all four rows |
| `12-11` sessions/retention | `COMP-P4-F3-T4-1` (`S3-05`, `luna_worker`) | that row |
| `12-13` native Assist acceptance | `COMP-PROV-011` and `COMP-CTX-010` (`S3-07`, `luna_worker`) | both rows |

No implementation files were changed. This audit is inventory evidence only; it does not mark any acceptance or completion gate green.
