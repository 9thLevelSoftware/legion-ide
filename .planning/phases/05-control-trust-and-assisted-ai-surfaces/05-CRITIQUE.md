# Phase 5 Plan Critique -- Control, Trust, and Assisted AI Surfaces

## Verdict

PASS after auto-refine.

## Rule-Chain Trace

| Plan | Verdict | Rule trace |
| --- | --- | --- |
| 05-01 | OK | Required frontmatter present; no files_modified/files_forbidden overlap; governance-first ordering prevents legacy Phase 5 collision. |
| 05-02 | OK | Required frontmatter present; no wave overlap; protocol/UI scope is isolated from app and desktop implementation. |
| 05-03 | OK | Required frontmatter present; app proposal routing is sequenced after protocol/UI contracts; exact `AppCommandOutcome` variants are specified after auto-refine. |
| 05-04 | OK | Required frontmatter present; assisted-AI outcome and UI intent API shapes are specified after auto-refine. |
| 05-05 | OK | Required frontmatter present; desktop actions and workflow status mapping depend on exact app/UI outcomes from prior waves. |
| 05-06 | OK | Required frontmatter present; verification wave is test/evidence-only and forbids product-code edits. |
| 05-07 | OK | Required frontmatter present; final acceptance is blocked unless prior results and full gates pass. |

## Critique Findings And Refinements

1. Legacy Phase 5 governance collision:
   - Finding: `xtask/src/main.rs` still validates legacy plugin Phase 5 evidence, while the GUI roadmap Phase 5 is control/trust/assisted-AI.
   - Refinement: Plan 05-01 owns a compatibility-preserving GUI Phase 5 evidence branch and forbids deleting the legacy plugin evidence.

2. App proposal control gap:
   - Finding: `CommandDispatcher::route_proposal_intent` exists, but generic app dispatch currently maps proposal lifecycle intents to `Noop`.
   - Refinement: Plan 05-03 now requires app-level detection before generic routing, app-owned proposal lookup, and exact `AppCommandOutcome::ProposalLifecycleUpdated` / `ProposalDetailsOpened` variants.

3. Assisted-AI API ambiguity:
   - Finding: The first draft left explain/propose API shape and refusal outcome representation to the executor.
   - Refinement: Plan 05-04 now specifies `run_assisted_ai_operation`, `AppAiRunOutcome` optional proposal/refusal fields, and exact UI intents `StartAiExplain` and `StartAiProposal` while preserving `StartAiRun`.

4. Desktop bridge gap:
   - Finding: Current desktop actions cover language/terminal but not proposal or assisted-AI controls.
   - Refinement: Plan 05-05 now specifies exact desktop actions and status mapping for proposal and assisted-AI outcomes.

5. False-failing verifier:
   - Finding: A blocker-scan verification line used an `rg` command that would exit nonzero on success when no blockers are present.
   - Refinement: Plan 05-07 now uses a PowerShell blocker-scan command that exits zero only when no blocker markers are found.

6. Manifest no-change assertion:
   - Finding: Plan 05-01 initially used a non-assertive `git diff --` display command for manifest non-modification.
   - Refinement: Plan 05-01 now uses `git diff --quiet -- Cargo.toml Cargo.lock`.

## Parallelization Decision

The phase remains sequential across seven waves. Product code touches high-risk shared files (`crates/devil-app/src/lib.rs`, `crates/devil-ui/src/ui.rs`, `crates/devil-desktop/src/{view,bridge,workflow}.rs`, and `xtask/src/main.rs`), and later waves depend on exact contracts from earlier waves. Parallel same-wave execution would create avoidable merge and authority-boundary risk.

## Residual Risks For Build

- `xtask` governance changes must preserve the accepted legacy plugin Phase 5 evidence while adding GUI Phase 5 checks.
- `devil-protocol` already contains deep trust DTOs; build agents should reuse existing types instead of adding parallel detail DTOs.
- Assisted-AI explain output may need to remain metadata/provenance-only unless a bounded, redacted text projection is explicitly added and tested.
- Full workspace gates may be expensive; final acceptance remains blocked unless required gates pass or the project owner changes the gate.
