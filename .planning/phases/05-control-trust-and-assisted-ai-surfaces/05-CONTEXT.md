# Phase 5: Control, Trust, and Assisted AI Surfaces -- Context

## Workflow Inputs

- Command: `$legion plan 5 --auto-refine`
- Roadmap phase: Phase 5, "Control, Trust, and Assisted AI Surfaces"
- Requirements: R-010, R-011, R-013
- Requirements source: `.planning/ROADMAP.md` and `.planning/PROJECT.md`. `.planning/REQUIREMENTS.md` is absent.
- Planning mode: auto-refine enabled.
- Settings: no `settings.json` was found at the project root, so the workflow default of at most three implementation tasks per plan applies.
- Control modes: `.planning/config/control-modes.yaml` is absent, so workflow-common guarded defaults apply.
- GitHub issue creation: skipped because `git remote get-url origin` returned no origin remote.
- Agent directory: `C:/Users/dasbl/.legion/agents`; assigned agent ids were validated there.

## Codebase Map

The codebase map is fresh for the current checkout:

- Map generated at `2026-05-27T12:57:55.9718684-04:00`
- Map commit: `beb896492685fadbb4d1669250f0a5f5a145f613`
- Current HEAD during planning: `beb896492685fadbb4d1669250f0a5f5a145f613`
- Source fingerprint: `d659f105ff658b23cd599dfe39bd98ec6aedb99082fc2840467a462783e2b584`

Use `.planning/CODEBASE.md` and `.planning/codebase/` for orientation, but every build plan still requires live source reads before edits.

## Phase Goal

Make the control-first differentiator visible and usable in the renderer-backed GUI.

Roadmap success criteria:

- Proposal ledger, proposal details, diff/target summary, approval checklist, rollback/checkpoint, context manifest, privacy inspector, and permission/risk/cost budget panels are usable.
- Assisted AI explain/propose flows use local-first/default-deny provider routing.
- AI-generated edits are proposals only and never self-applied.
- Users can see what context was used, what was redacted or denied, and what risk labels apply.
- Approval, rejection, cancellation, stale, conflict, failed, applied, and rolled-back states are visible.

## Current State Evidence

- `crates/devil-protocol/src/lib.rs` already defines rich Phase 5 trust DTOs, including `ContextManifestProjection`, `PrivacyInspectorProjection`, `PermissionBudgetProjection`, `ProposalApprovalChecklistProjection`, `CheckpointRollbackProjection`, `AssistedAiProjection`, and `ProposalLedgerProjection`.
- `crates/devil-protocol/src/lib.rs` also exposes helper functions such as `privacy_inspector_from_context_manifest_projection`, `permission_budget_projection_from_contracts`, `checkpoint_rollback_projection_from_proposal`, `approval_checklist_from_trust_projections`, and `assisted_ai_projection_from_metadata`.
- `crates/devil-ui/src/ui.rs` already carries the trust and assisted-AI projections in `ShellProjectionSnapshot`, and it has projection-only command intents for proposal lifecycle commands plus AI start/cancel/replay/inspect.
- `crates/devil-app/src/lib.rs` currently builds proposal ledger rows from `AppProposalCoordinator`, has Phase 4 assisted-AI local-provider routing, and populates shell trust projections from `phase4_projection_state`.
- `crates/devil-app/src/lib.rs` has `CommandDispatcher::route_proposal_intent`, but generic `dispatch_ui_intent` currently maps proposal lifecycle intents to `Noop` before app-owned proposal handling. Phase 5 must route those intents through app/proposal authority.
- `crates/devil-desktop/src/view.rs` renders only summary rows for proposal, trust, and assistant projections. It does not yet provide usable detail rows for context, redaction/denial, budgets, approval gates, rollback/checkpoint limitations, or assisted-AI previews.
- `crates/devil-desktop/src/bridge.rs` has desktop actions for language and terminal flows, but no adapter-local actions for proposal lifecycle controls or assisted-AI control commands.
- `xtask/src/main.rs` still treats Phase 5 as the legacy plugin architecture gate through `plans/evidence/phase-5/plugin-architecture-map.md`. The active GUI roadmap Phase 5 is different, so governance must be rebaselined before acceptance claims.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only. It may carry snapshots and emit `CommandDispatchIntent`, but it must not own editor text, workspace state, proposal lifecycle authority, provider runtime state, storage, or renderer behavior.
- `devil-desktop` may render, collect adapter-local view state, and translate UI events, but it must not own proposal lifecycle state, provider routing, editor text, workspace mutation, storage, or security policy.
- Proposal preview, approve, reject, cancel, apply, and rollback commands must route through `AppComposition` and `AppProposalCoordinator` or existing workspace proposal authorities.
- AI-generated edits remain proposals only. No assisted-AI flow may apply editor or disk mutations directly.
- Provider routing remains local-first and default-deny. Hosted/remote/default-denied provider paths must become visible refusal/projection data, not silent success.
- Context, privacy, permission, budget, risk, cost, checkpoint, rollback, and audit projections must remain metadata-only and redacted.
- Final acceptance must not overwrite or delete the legacy accepted plugin Phase 5 evidence. The GUI Phase 5 evidence must be explicitly distinguished.

## Key Design Decisions

- Architecture proposals were skipped because the direct user command plus `--auto-refine` means generate executable plans from current source evidence.
- The roadmap estimate of five plans is treated as an estimate, not a cap. Seven waves are required because governance, protocol/UI contracts, app lifecycle routing, AI routing, desktop controls, safety evidence, and final acceptance each have separate owners and verification.
- The first wave is governance because `xtask` still points Phase 5 at legacy plugin evidence. Product-code waves must not create an acceptance path that conflicts with the active repo gate.
- Proposal lifecycle routing is separated from assisted-AI routing. Proposal controls affect all proposal sources; assisted-AI explain/propose adds provider/refusal behavior and must remain proposal-only.
- Desktop usability is separated from app authority so renderer code can remain a projection consumer and intent producer.
- Final acceptance is its own wave and may update roadmap/state only after full gates pass.

## Plan Structure

- **Plan 05-01 (Wave 1)**: Governance And Acceptance Gate Rebaseline -- distinguish GUI Phase 5 from legacy plugin Phase 5 and create the not-accepted GUI evidence gate.
- **Plan 05-02 (Wave 2)**: Trust Projection Contract Completion -- complete protocol/UI trust projection contracts and DTO tests needed by later app and desktop work.
- **Plan 05-03 (Wave 3)**: Proposal Lifecycle App Routing And Details Population -- route proposal control intents through app authority and populate selected proposal trust details.
- **Plan 05-04 (Wave 4)**: Assisted AI Explain And Propose Routing -- add explain/propose local-first/default-deny assisted-AI flows with visible refusals and proposal-only edit output.
- **Plan 05-05 (Wave 5)**: Desktop Control Trust Panels And Actions -- render usable control/trust/assistant panels and translate proposal/AI actions into intents.
- **Plan 05-06 (Wave 6)**: Control Surface Safety Regression Suite -- prove lifecycle states, proposal-only AI behavior, metadata-only trust data, and no UI/desktop authority creep.
- **Plan 05-07 (Wave 7)**: Phase 5 Evidence And Acceptance Gate -- run full gates, archive acceptance evidence, and update planning state after proof.

## Auto-Refine Summary

The `--auto-refine` pass identified and addressed these planning risks before finalization:

1. The active GUI Phase 5 collides with legacy plugin Phase 5 evidence in `xtask` and `plans/dependency-policy.md`. Plan 05-01 now makes this the first gate and requires compatibility with existing accepted plugin evidence.
2. Existing protocol DTOs are deep, but desktop/app usability is shallow. Plans 05-02 through 05-05 require source-backed projection population and detail rows instead of adding parallel renderer-owned state.
3. Proposal lifecycle UI intents currently do not dispatch through app-owned proposal handling. Plan 05-03 now owns that routing and lifecycle visibility.
4. Assisted AI currently has a Phase 4 propose-only path and errors on provider refusal. Plan 05-04 requires explain/propose flows plus visible refusal projections.
5. The desktop bridge lacks proposal/AI actions. Plan 05-05 adds adapter-local action translation while keeping app authority.
6. Final acceptance spans governance, app, protocol, desktop, AI/provider, security, and full repository gates. Plan 05-06 owns safety regression evidence; Plan 05-07 owns final acceptance and state updates.
