# QUAL.11 — Release-blocker taxonomy

**Status:** in-repo definition (GAP-07.2)  
**Queue:** GitHub issues filed with `.github/ISSUE_TEMPLATE/release-blocker.yml`  
**Labels:** `qual-11`, `release-blocker`, plus one `severity-*` label  
**Does not:** promote a product-readiness ledger row, or replace the standing gates

QUAL.11 is the release-blocker taxonomy named in `plans/legion-production-master-plan-v0.2.md` § WS-QUALITY-01. A release blocker is a gap that forbids a named **release claim**. It is not a generic bug. Do not file these on `.github/ISSUE_TEMPLATE/bug_report.md`.

## Severity

| Label | Blocks this claim | Sequence wave |
| --- | --- | --- |
| `severity-p0` | Any credible public or invitation-only preview claim | Waves 0–3 in `plans/p0-installed-product-sequence-v0.1.md` |
| `severity-p1` | Focused Rust-first IDE (after P0) | P1 rows in the 2026-08-31 full pass |
| `severity-p2` | Universal / extension-platform claim | P2 rows |
| `severity-p3` | Autonomous or enterprise claim | P3 rows |

Closing a `severity-p0` issue does not make Legion generally available. It only removes that row as a reason to refuse the claim named in the issue.

## Required fields

Every QUAL.11 issue must name:

| Field | Meaning |
| --- | --- |
| GAP / P0 id | `P0-01`…`P0-10` or a `GAP-*` task id from the sequence |
| Ledger row | `PR-UI-001`, `PR-REL-001`, … or `none` with a reason |
| Owner | GitHub login or team responsible for the next evidence file |
| Intended claim blocked | dogfood, preview, public-alpha, rust-ga, universal-ga, autonomous-ga, or enterprise |
| Current evidence level | 0–5 from the installed-product ladder |
| Remaining gap | one sentence, present tense |
| Close-out evidence | the file or command that will exist when this is no longer a blocker |

## P0 register (current queue)

These ids are always valid on a QUAL.11 issue. File one issue per id unless a single evidence packet honestly closes more than one.

| Id | Gap | Ledger row (typical) |
| --- | --- | --- |
| P0-01 | Installed-product truth | PR-REL-001, PR-UI-001 |
| P0-02 | Release signing | PR-REL-001 |
| P0-03 | Update safety | PR-REL-001 |
| P0-04 | Data safety | PR-UI-001, PR-REL-001 |
| P0-05 | Accessibility | PR-UI-001 |
| P0-06 | Manual / no-AI proof | PR-AI-001, PR-REL-001 |
| P0-07 | Governance | all rows (process) |
| P0-08 | Documentation truth | all rows (process) |
| P0-09 | Performance | PR-UI-001, PR-UI-002 |
| P0-10 | Support / legal | PR-REL-001 |

## Close-out rule

An issue labeled `qual-11` may move to closed only when the close-out evidence path exists in-tree (or as a named hosted run URL) **and** the evidence level claimed in the issue is not higher than that file can support. Headless `--beta-smoke`, AppComposition golden paths, and cargo projection tests cannot close a level-4 or level-5 blocker.

## Related

- Sequence: `plans/p0-installed-product-sequence-v0.1.md`
- Full-pass evidence: `plans/evidence/production/WS-P0/2026-08-31-release-gap-full-pass.md`
- Ledger: `plans/product-readiness-ledger.md`
- Template: `.github/ISSUE_TEMPLATE/release-blocker.yml`
