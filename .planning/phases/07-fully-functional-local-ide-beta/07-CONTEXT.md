# Phase 7: Fully Functional Local IDE Beta -- Context

## Workflow Inputs

- Command: `$legion plan 7 --auto-refine`
- Roadmap phase: Phase 7, "Fully Functional Local IDE Beta"
- Requirements: R-014
- Requirements source: `.planning/ROADMAP.md` and `.planning/PROJECT.md`. `.planning/REQUIREMENTS.md` is absent.
- Planning mode: auto-refine enabled.
- Settings: no `settings.json` was found, so the Legion default of at most three implementation tasks per plan applies.
- Control modes: `.planning/config/control-modes.yaml` is absent, so workflow-common guarded defaults apply.
- Agent directory: `C:/Users/dasbl/.legion/agents`; assigned agent ids were validated there.
- GitHub issue creation: skipped during this local planning run. The core artifacts are written in-repo, and any external GitHub write should be revalidated from current remote/auth state before use.

## Codebase Map

The codebase map exists but is stale relative to live source:

- Map generated at `2026-05-27T12:57:55.9718684-04:00`
- Map commit: `beb896492685fadbb4d1669250f0a5f5a145f613`
- Current HEAD during planning: `f44932aeeeeaa6cc9c7521d0fed24227f10358a8`
- Worktree note: Phase 6 implementation and evidence files are present in the dirty worktree and are treated as current workflow state.

Use `.planning/CODEBASE.md` and `.planning/codebase/` for orientation only. Build agents must read live source before editing.

Relevant map chunks used during planning:

- `map:desktop-workflow:001`: `crates/devil-desktop/src/workflow.rs` owns desktop runtime open, session, diagnostics, and action routing.
- `map:desktop-bridge:001`: `crates/devil-desktop/src/bridge.rs` maps desktop actions to `CommandDispatchIntent` or app requests.
- `map:desktop-view:001`: `crates/devil-desktop/src/view.rs` builds testable view models for editor, search, proposal, language, terminal, assistant, plugin, and collaboration rows.
- `map:language-workflow:001`: `crates/devil-app/src/lib.rs` owns diagnostics, hover, completion, definitions, references, outline, and proposal-producing language actions.
- `map:terminal-workflow:001`: `crates/devil-app/src/lib.rs` owns default-deny terminal workflow and metadata audit.
- `map:save-workflow:001`: `AppComposition::save_active_buffer` routes through proposal-mediated workspace save authority and preserves dirty text on rejection.
- `map:storage-observability:001`: metadata-only storage and health patterns.

## Phase Goal

Reach a beta that can be used as a local IDE for normal development.

Roadmap success criteria:

- A real Rust repository can be opened, browsed, edited, searched, saved, checked through terminal or language tooling, and reviewed through proposal surfaces.
- GUI-visible diagnostics and operational health are available.
- Privacy-safe logs, redacted diagnostics export, release-readiness checklist, launch docs, and known limitations are complete.
- Critical workflows have automated smoke coverage and manual evidence.
- The beta does not claim unsupported remote/collaboration/plugin/autonomy behavior.

## Current State Evidence

- Phase 6 is marked complete and reviewed in `.planning/ROADMAP.md` and `.planning/STATE.md`.
- `devil-desktop` launches through `crates/devil-desktop/src/main.rs` and `crates/devil-desktop/src/workflow.rs`.
- Existing desktop tests cover open/edit/save, conflict preservation, save-all, dirty-close prompts, search, session restore, large-file guardrails, language/terminal projection, control/trust proposal actions, platform smoke, package dry-run, and diagnostics export.
- Phase 6 added `crates/devil-desktop/src/package.rs`, `platform.rs`, and `diagnostics.rs`, plus `scripts/package-windows.ps1`, `scripts/gui-smoke.ps1`, `scripts/gui-smoke.sh`, and GUI Phase 6 evidence under `plans/evidence/gui-productization/`.
- `crates/devil-cli/src/main.rs` and `xtask/src/main.rs` currently have a GUI Phase 6 evidence gate but no GUI Phase 7 local-beta gate.
- Legacy `plans/evidence/phase-7/remote-architecture-map.md` is already accepted for a remote development substrate. It is not GUI local-beta evidence and must remain untouched.
- `plans/dependency-policy.md` has a legacy Phase 7 remote-development activation note. GUI Phase 7 needs a compatibility note for local IDE beta evidence without changing remote acceptance.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only. It must not own editor text, workspace state, proposal lifecycle state, storage, terminal sessions, provider calls, plugin hosts, collaboration sessions, or remote sessions.
- `devil-desktop` may own renderer resources, adapter-local presentation state, smoke harnesses, metadata-only diagnostics, and native platform observations. It must not own app/editor/workspace/proposal/security/storage/runtime authority.
- Saves stay proposal-mediated through `AppComposition::save_active_buffer` -> `SaveWorkflowService` -> `WorkspaceActor::save_file_with_proposal`.
- Terminal workflows stay policy-gated and metadata-audited. Beta evidence may prove denial, bounded output, and fixture status, but must not claim direct terminal mutation authority.
- Language edit-producing actions must remain proposal previews before mutation.
- Diagnostics, smoke reports, session records, and docs must be metadata-only and must not persist raw source, dirty buffer text, prompts, provider payloads, terminal payloads, or secrets.
- Phase 7 GUI beta must not overwrite or repurpose legacy remote-development Phase 7 evidence under `plans/evidence/phase-7/`.
- Unsupported plugin, collaboration, remote, hosted provider, delegated autonomy, signed installer, and GA platform parity behavior must be labeled as limitations unless separately accepted by later phases.

## Key Design Decisions

- Architecture proposals were skipped because the direct command plus `--auto-refine` means generate executable plans from live repository evidence.
- The roadmap estimate of four plans is treated as an estimate, not a cap. Five sequential plans are required because governance, beta workflow smoke, operational health, docs/readiness, and final acceptance have separate write scopes and verification gates.
- Plan 07-01 comes first because GUI Phase 7 local-beta evidence collides by number with the already accepted legacy Phase 7 remote evidence. The build must add a distinct GUI Phase 7 acceptance path before marking beta complete.
- Automated beta smoke should use an isolated Rust workspace fixture for edit/save safety while also supporting non-mutating launch/search/manual evidence against this repository.
- Operational health is split from workflow smoke because diagnostics and GUI-visible health must remain metadata-only and need explicit privacy tests.
- Documentation and known limitations are a separate wave so they can reflect actual smoke/health outputs and avoid unsupported beta claims.
- Final acceptance is its own wave and may update roadmap/state only after full gates, GUI Phase 7 evidence checks, and required beta artifacts pass.

## Plan Structure

- **Plan 07-01 (Wave 1)**: GUI Phase 7 Governance And Evidence Gate -- add a local-beta evidence path distinct from legacy remote Phase 7 evidence.
- **Plan 07-02 (Wave 2)**: End-To-End Local IDE Beta Smoke Harness -- add deterministic beta workflow smoke for open/browse/edit/search/save/language/terminal/proposal flows.
- **Plan 07-03 (Wave 3)**: Operational Health And Privacy-Safe Diagnostics -- add GUI-visible operational health rows and richer redacted diagnostics export.
- **Plan 07-04 (Wave 4)**: Beta Launch Docs Known Limitations And Release Readiness -- document launch, workflow evidence, limitations, and beta readiness.
- **Plan 07-05 (Wave 5)**: Phase 7 Evidence Capture And Acceptance Gate -- archive evidence, run full gates, and mark GUI Phase 7 accepted only with proof.

## Auto-Refine Summary

The `--auto-refine` pass identified and addressed these planning risks before finalization:

1. Legacy Phase 7 evidence is remote-development substrate evidence, not GUI local-beta evidence. Plan 07-01 creates a separate GUI Phase 7 gate and forbids edits under `plans/evidence/phase-7/`.
2. A beta smoke could mutate the user's checkout if it edits the live repository. Plan 07-02 requires an isolated Rust workspace fixture for write actions and non-mutating evidence for real-repo launch/search.
3. "Checked through terminal or language tooling" could be overclaimed because terminal is policy-gated and language edit actions are proposal-based. Plan 07-02 requires explicit denial/bounded/proposal evidence instead of direct mutation claims.
4. Diagnostics could leak raw source or status bodies. Plan 07-03 requires metadata-only operational health fields and tests with secret-like dirty text.
5. The beta could accidentally claim plugin/collaboration/remote/autonomy readiness. Plan 07-04 requires known limitations and unsupported-surface labels before acceptance.
6. Final acceptance could be marked from generated docs alone. Plan 07-05 blocks acceptance on plan results, required artifacts, CLI/xtask evidence checks, smoke scripts, and full repository gates.
