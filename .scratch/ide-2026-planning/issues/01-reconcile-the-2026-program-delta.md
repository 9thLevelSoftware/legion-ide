# 01: Reconcile the 2026 program delta

Status: resolved

**What to build:** Map the 36 decisions and new outcomes onto existing canonical requirements; preserve IDs and expose unmapped or conflicting obligations without recreating the completion register.

**Blocked by:** None (can start immediately).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 01.

**Traceability:** M0; S0-01/06; F0. **Verification target:** A20.

## Acceptance criteria

- [x] Map the 36 decisions and new outcomes onto existing canonical requirements.
- [x] Preserve IDs and expose unmapped or conflicting obligations without recreating the completion register.
- [x] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [x] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Answer

Delivered [IDE 2026 reconciliation](../../../plans/completion/ide-2026-reconciliation.md):
Q1–Q36 mapped to existing canonical anchors and approved ticket owners, nine explicit
delta/conflict handoffs, all 22 retained families, F0–F8, and unresolved configuration,
authority and external prerequisites. Anchors are distinguished from missing atomic
rows. The limited pilot/native-equivalent approval in the execution record is
acknowledged without ratifying remaining fields. Canonical records and product
acceptance are unchanged. No downstream feature was implemented.

Independent review PASS (Einstein), reported by the coordinator on 2026-10-08:
all 36 decisions, 22 families, F0–F8 references and handoffs validated, with no
authority or acceptance issues. `resolved` applies only to this bounded
reconciliation deliverable on `codex/ide-2026-ticket-001`.

### Actual local verification (2026-10-08)

Commands ran from `D:/legion-ide-2026-workers/ticket-001`. The shared xtask source
and Cargo inputs matched this worktree, and its executable was newer than those
inputs. No duplicate build or broad Cargo suite was run. Each xtask check ran once.

```text
& D:/legion-ide/target/debug/xtask.exe docs-hygiene
documentation hygiene checks passed
exit 0

& D:/legion-ide/target/debug/xtask.exe verify-completion-register --root .
implementation status counts:
  absent: 43
  implemented: 143
  partial: 233
acceptance status counts:
  unassessed: 419
verify-completion-register passed: register structure only, no evidence or acceptance was assessed
exit 0

& D:/legion-ide-2026-notes/ticket-001-structural-check.ps1
structural mapping passed: 36 unique decisions match source Q1-Q36; 82 requirements, 19 scenarios, 2 configurations, 37 package references valid (S1-03B explicitly unregistered); 254 approved ticket numbers across 74 owner rows; 22 families, F0-F8 and D1-D9 retained; 8 canonical SHA256 baselines unchanged
exit 0

git diff --check
no output; exit 0 (before this verification/status entry)
```

The initial inline structural check returned exit 1 with these actual messages:

```text
Expected owner columns in 36 decision, 9 delta, 22 family, 7 prerequisite rows; found 67
Delta coverage differs from D1-D9
```

Focused diagnosis showed checker errors: prerequisite owners are in column two,
and a D4–D6 prerequisite reference was counted as a tenth delta row. Corrected the
checker to use the actual owner column and scope delta counting to its table;
reran only that failed check using the external script above. No reconciliation
content change was needed. Successful xtask checks were not repeated.

These checks validate document links/register structure and mapping/preservation,
not live providers, native product workflows, configuration ratification, signing,
experiments or release readiness. Downstream owners and their unresolved gates
are explicit in the reconciliation.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
- 2026-10-08: Claimed for implementation on `codex/ide-2026-integration`; isolated worker branch owns reconciliation.
- 2026-10-08: Bounded reconciliation implemented and locally verified; resolved on the review-ready worker branch. Independent review is underway; no commit yet and no canonical promotion.
- 2026-10-08: Independent review PASS (Einstein); coordinator authorized committing only the three owned files. Integration tip `356c59f` is merged; execution-record updates remain coordinator-owned. Successful checks were not repeated.
