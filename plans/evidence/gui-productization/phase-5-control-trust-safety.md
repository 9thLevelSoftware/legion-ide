# GUI Phase 5 Control Trust Safety Evidence

Status: Safety evidence complete; not final acceptance.

## Scope

This document records targeted regression proof for GUI Phase 5 control, trust, and assisted-AI surfaces. It is evidence for final acceptance to cite, not the final acceptance record.

Prior wave result files `05-01-RESULT.md` through `05-05-RESULT.md` were present and scanned for unresolved stop markers before this evidence was written. No unresolved markers were found.

## Proposal Lifecycle Visibility

Evidence:

- `crates/devil-app/tests/control_trust_surfaces.rs` covers preview, approve, reject, cancel, apply, rollback, stale, conflict, failed, and rolled-back lifecycle visibility through app-owned proposal authority and shell projections.
- `proposal_lifecycle_ui_intents_route_through_app_authority` verifies UI proposal intents route through `AppComposition` and return `ProposalLifecycleUpdated` or `ProposalDetailsOpened`.
- `proposal_states_visible_after_ui_apply_rejections_and_rollback` verifies stale/conflict/failed/rolled-back states remain visible in `ProposalLedgerProjection`.
- `proposal_details_selected_proposal_populates_trust_surfaces` verifies selected proposal details populate diff/target summary, context manifest, privacy inspector, permission budget, approval checklist, and checkpoint/rollback projections.
- `dirty_text_preserved_on_rejected_stale_and_conflict_outcomes` verifies rejected, stale, and conflict outcomes preserve dirty editor text and do not overwrite disk content incorrectly.

Commands:

- `cargo test -p devil-app --test control_trust_surfaces proposal_states_visible -- --nocapture` passed: 1 test passed.
- `cargo test -p devil-app --test control_trust_surfaces dirty_text_preserved -- --nocapture` passed: 1 test passed.
- `cargo test -p devil-app --test control_trust_surfaces proposal_details -- --nocapture` passed: 1 test passed.
- `cargo test -p devil-app --test control_trust_surfaces -- --nocapture` passed: 7 tests passed.

## Assisted AI Proposal-Only Behavior

Evidence:

- `assisted_ai_explain_routes_metadata_only_without_proposal` verifies explain runs produce metadata-only assisted-AI projections and no proposal ledger rows.
- `assisted_ai_propose_is_proposal_only` verifies propose runs create a proposal but leave the active editor buffer and disk unchanged.
- `assisted_ai_refusals_visible_for_untrusted_workspace` verifies untrusted/default-deny provider routing returns visible refusal metadata rather than silent success.
- Desktop view tests render provider, route, request, refusal, and preview rows without raw provider payloads or direct apply authority.

Commands:

- `cargo test -p devil-app --test control_trust_surfaces assisted_ai_propose_is_proposal_only -- --nocapture` passed: 1 test passed.
- `cargo test -p devil-app --test control_trust_surfaces assisted_ai -- --nocapture` passed: 3 tests passed.
- `cargo test -p devil-desktop --test control_trust_view -- --nocapture` passed: 3 tests passed.

## Metadata-Only Trust Data

Evidence:

- App tests assert trust surfaces are populated from proposal projections and selected details, not from desktop-owned state.
- Desktop view tests assert bounded rows for proposal diffs and targets, context items and permissions, privacy records and refusals, permission budgets and evaluations, approval gates and blockers, checkpoint/rollback metadata, and assisted-AI provider/request/refusal/preview metadata.
- The rendered desktop rows use counts, labels, ids, risk/privacy labels, readiness states, redaction hints, and metadata references; they do not render raw provider responses or own editor/workspace state.

Commands:

- `cargo test -p devil-desktop --test control_trust_view -- --nocapture` passed: 3 tests passed.
- `cargo test -p devil-ui control_trust -- --nocapture` passed: 2 tests passed.

## Local And Default-Deny Routing

Evidence:

- Trusted local-loopback propose/explain paths are covered by app tests and remain metadata/proposal mediated.
- Untrusted workspace routing is covered by `assisted_ai_refusals_visible_for_untrusted_workspace`, which expects `AssistedAiProviderInvocationState::Refused` and an `assisted_ai_projection.refusals` row with `capability.denied`.
- No test requires cloud credentials or network egress.

Commands:

- `cargo test -p devil-app --test control_trust_surfaces assisted_ai -- --nocapture` passed: 3 tests passed.

## No UI or desktop authority creep

Evidence:

- `devil-ui` tests verify control/trust shell projections are static and command parsing only emits `CommandDispatchIntent`.
- `devil-desktop` bridge tests verify proposal and AI controls translate into intents using projected ids and display-safe labels.
- `desktop_control_trust_bridge_preserves_projection_only_boundary` scans desktop bridge/view source to ensure it does not reference `WorkspaceProposal`, `ProviderRouter`, `WorkspaceActor`, or `EditorEngine`.
- Desktop workflow maps app outcomes to status only after app authority returns an outcome; it does not own lifecycle, provider, editor, workspace, or storage state.

Commands:

- `cargo test -p devil-desktop --test control_trust_bridge -- --nocapture` passed: 5 tests passed.
- `cargo test -p devil-ui control_trust -- --nocapture` passed: 2 tests passed.

## Residual Risk

- This evidence is intentionally targeted. Full workspace build, clippy, dependency policy, and final acceptance evidence remain owned by Plan 05-07.
- The exact `cargo test -p devil-desktop --test projection_rendering control -- --nocapture` command from Plan 05-05 matches no tests in that target; the unfiltered projection rendering target passed separately in 05-05.
