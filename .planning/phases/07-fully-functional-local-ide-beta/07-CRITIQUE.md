# Phase 7 Plan Critique -- Fully Functional Local IDE Beta

## Verdict

PASS after auto-refine.

## Method

The auto-refine pass applied the Legion plan-critique checks in-process against the generated Phase 7 files: schema conformance, wave overlap, pre-mortem risks, assumption gaps, completeness, decision completeness, and unsupported-claim handling.

## Rule-Chain Trace

| Plan | Verdict | Rule trace |
| --- | --- | --- |
| 07-01 | OK | Required frontmatter present; no files_modified/files_forbidden overlap; governance-first ordering prevents collision with accepted legacy remote Phase 7 evidence. |
| 07-02 | OK | Required frontmatter present; beta smoke writes are isolated under `target/gui-phase7-beta-workspace`; app/UI/protocol authority files are forbidden. |
| 07-03 | OK | Required frontmatter present; operational health is metadata-only and blocks on raw-payload diagnostics. |
| 07-04 | OK | Required frontmatter present; documentation-only scope forbids code/script/CI/legacy evidence edits and requires unsupported-surface limitations. |
| 07-05 | OK | Required frontmatter present; final acceptance is blocked unless required evidence, limitations, smoke, CLI/xtask checks, and full repository gates pass. |

## Critique Findings And Refinements

1. Legacy Phase 7 evidence collision:
   - Finding: `plans/evidence/phase-7/remote-architecture-map.md` is already accepted remote-development evidence, while GUI Phase 7 is local IDE beta.
   - Refinement: Plan 07-01 creates a separate GUI Phase 7 evidence path and forbids edits under `plans/evidence/phase-7/`.

2. Real-repo mutation risk:
   - Finding: Beta smoke could accidentally edit the user's current checkout while proving edit/save.
   - Refinement: Plan 07-02 requires an isolated Rust workspace under `target/gui-phase7-beta-workspace` for all write actions and reserves real-repo evidence for non-mutating/manual launch proof.

3. Terminal/language overclaim risk:
   - Finding: "checked through terminal or language tooling" could be misread as direct PTY/LSP mutation support.
   - Refinement: Plan 07-02 requires explicit metadata evidence for default-deny terminal behavior, bounded status, language cancellation/status, and proposal-only mutation paths.

4. Diagnostics leakage risk:
   - Finding: Operational health and diagnostics could leak raw dirty text, source bodies, prompts, terminal payloads, provider payloads, or status-message bodies.
   - Refinement: Plan 07-03 restricts diagnostics to counts, labels, booleans, ids, schema versions, and unsupported-surface labels, with secret-marker tests.

5. Unsupported beta claim risk:
   - Finding: The beta could accidentally claim remote/collaboration/plugin/hosted-provider/autonomous behavior that is not in local GUI Phase 7 scope.
   - Refinement: Plan 07-04 requires known limitation markers, and Plan 07-05 blocks acceptance unless those markers remain present.

6. False final-gate blocker:
   - Finding: Initial documentation/final plans used `git diff --quiet` over code/script paths even though earlier Phase 7 waves intentionally modify those paths.
   - Refinement: Plans 07-04 and 07-05 now require result-file statements that their own edits are documentation/acceptance-scoped, while preserving earlier planned diffs.

7. Acceptance from docs alone:
   - Finding: Phase 7 could be marked accepted from generated documentation without executable evidence.
   - Refinement: Plan 07-05 blocks acceptance on plan results, required artifacts, targeted desktop tests, smoke wrappers, CLI/xtask evidence checks, and full repository gates.

## Parallelization Decision

The phase remains sequential across five waves. Governance must establish the GUI Phase 7 gate first; beta workflow smoke produces evidence consumed by operational health; docs depend on the smoke and diagnostics outputs; final acceptance depends on every prior result. Parallel execution would create avoidable conflicts in `devil-desktop` smoke/workflow files, scripts, CI, and the main Phase 7 evidence file.

## Residual Risks For Build

- Native-window smoke can still be blocked by the local host. The plans require exact blocked evidence rather than silent acceptance.
- The automated beta workflow uses an isolated Rust fixture for safe writes; manual real-repo evidence must still be captured before acceptance.
- OS accessibility inspection and signed installer evidence remain limited unless later build work produces direct proof.
- Existing legacy Phase 7 remote evidence remains accepted but is not GUI local-beta evidence.
