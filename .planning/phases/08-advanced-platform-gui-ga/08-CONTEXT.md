# Phase 8: Advanced Platform GUI GA -- Context

## Workflow Inputs

- Command: `$legion plan 8 --auto-refine`
- Roadmap phase: Phase 8, "Advanced Platform GUI GA"
- Requirements: R-015
- Requirements source: `.planning/ROADMAP.md` and `.planning/PROJECT.md`. `.planning/REQUIREMENTS.md` is absent.
- Planning mode: auto-refine enabled.
- Settings: no `settings.json` was found, so the Legion default of at most three implementation tasks per plan applies.
- Control modes: `.planning/config/control-modes.yaml` is absent, so workflow-common guarded defaults apply.
- Agent directory: `C:/Users/dasbl/.legion/agents`; assigned agent ids were validated there.
- GitHub issue creation: skipped during this local planning run because `gh auth status` succeeded but `git remote get-url origin` failed with no `origin` remote.

## Codebase Map

The codebase map exists but is stale relative to live source and the dirty worktree:

- Map generated at `2026-05-27T12:57:55.9718684-04:00`
- Map commit: `beb896492685fadbb4d1669250f0a5f5a145f613`
- Current HEAD during planning: `f44932aeeeeaa6cc9c7521d0fed24227f10358a8`
- Worktree note: Phase 6 and Phase 7 implementation/evidence files are present in the dirty worktree and are treated as current workflow state.

Use `.planning/CODEBASE.md` and `.planning/codebase/` for orientation only. Build agents must read live source before editing.

Relevant map chunks used during planning:

- `map:app-composition:001`: `crates/devil-app/src/lib.rs` owns workspace open, file open, UI intent dispatch, save, search, language, terminal, plugin, collaboration, and remote orchestration.
- `map:desktop-workflow:001`: `crates/devil-desktop/src/workflow.rs` owns desktop runtime open, diagnostics, session, action routing, and app outcome mapping.
- `map:desktop-bridge:001`: `crates/devil-desktop/src/bridge.rs` maps desktop actions to `CommandDispatchIntent` or app requests.
- `map:desktop-view:001`: `crates/devil-desktop/src/view.rs` builds testable view models for editor, search, proposal, language, terminal, assistant, plugin, collaboration, and health rows.
- `map:protocol-ports:001`: `crates/devil-protocol/src/lib.rs` owns shared DTOs, validation helpers, and ports for plugin, collaboration, remote, terminal, storage, and proposals.
- `map:plugins-collab-remote:001`: plugin, collaboration, and remote runtime crates exist as app-owned/protocol-mediated surfaces.
- `map:remote-transport:001`: `crates/devil-remote-transport/src/lib.rs` owns production remote transport handshake, frame, replay, package, and diagnostic metadata.
- `map:cli-diagnostics:001` and `map:xtask-gates:001`: evidence gates already distinguish GUI Phase 6/7 productization evidence from accepted legacy substrate phase evidence.

## Phase Goal

Expose accepted advanced runtime surfaces through production-grade GUI workflows.

Roadmap success criteria:

- Plugin management and contribution views preserve sandbox, capability, and metadata-only audit boundaries.
- Collaboration presence, shared proposal review, reconnect, and conflict surfaces are usable.
- Remote workspace connection manager, remote terminal/LSP/session status, reconnect/offline indicators, and remote proposal review are usable.
- Delegated task command center remains bounded and approval-gated.
- Cross-platform release, update, rollback, and incident response procedures are documented and evidenced.
- Windows, macOS, and Linux platform parity evidence exists before GA claims.

## Current State Evidence

- Phase 7 GUI local beta is accepted in `.planning/ROADMAP.md`, `.planning/STATE.md`, and `plans/evidence/gui-productization/phase-7-local-ide-beta.md`.
- Legacy `plans/evidence/phase-8/` is already accepted production GA runtime substrate evidence, including `phase-8-architecture-map.md`, `platform-matrix-evidence.txt`, and `release-readiness-review.md`.
- The GUI productization Phase 8 must not overwrite, reopen, or reinterpret legacy accepted Phase 8 evidence. It needs a separate GUI evidence gate under `plans/evidence/gui-productization/`.
- `devil-app` already has app-owned plugin command invocation, local collaboration session/presence routing, deterministic remote session connection/projections, and proposal-mediated save/language/AI flows.
- `devil-ui` already carries plugin contribution projections, collaboration presence projections, and delegated-task projection DTOs as projection-only data.
- `devil-desktop` currently renders compact plugin and collaboration rows and summarizes delegated tasks, but does not yet provide production-grade plugin manager, collaboration review/reconnect/conflict, remote manager, delegated task command center, or GA operations evidence surfaces.
- Phase 7 health currently lists plugin/collaboration/remote as unsupported limitations; Phase 8 must replace those limitations with evidence-backed supported GUI statuses where accepted.

## Non-Negotiable Constraints

- `devil-ui` remains projection-only. It must not own editor text, workspace state, proposal lifecycle state, storage, terminal sessions, provider calls, plugin hosts, collaboration sessions, remote sessions, telemetry, retention, or update authority.
- `devil-desktop` may own renderer resources, adapter-local presentation state, smoke harnesses, metadata-only diagnostics, and native platform observations. It must not own app/editor/workspace/proposal/security/storage/runtime authority.
- Plugin commands remain app-owned and protocol-mediated through manifest, capability, quota, sandbox, storage, and metadata-only audit boundaries.
- Collaboration GUI workflows must not apply editor mutations directly from UI or desktop code. Mutating collaboration operations remain app/editor-authority operations and shared proposal review remains proposal-mediated.
- Remote GUI workflows must not mutate local disk or editor state from transport/session views. Remote writes remain proposal-mediated and remote status/terminal/LSP rows must be descriptor/status metadata only.
- Delegated task GUI workflows are command-center and review surfaces only. They may show plan rows, blockers, trust gates, proposal-preview links, and audit readiness, but they must not activate autonomous apply.
- Diagnostics, smoke reports, release docs, update/rollback records, and incident response artifacts must be metadata-only and must not persist raw source, dirty buffer text, prompts, provider payloads, terminal output bodies, remote payload bodies, transport frames, secrets, or private keys.

## Key Design Decisions

- Architecture proposals were skipped because the direct command plus `--auto-refine` means generate executable plans from live repository evidence.
- The roadmap estimate of five plans is treated as an estimate, not a cap. Seven sequential plans are required because governance, plugin GUI, collaboration GUI, remote GUI, delegated task GUI, GA operations docs, and final acceptance have separate write scopes and verification gates.
- Plan 08-01 comes first because GUI Phase 8 collides by number with already accepted legacy Phase 8 substrate evidence. A distinct GUI Phase 8 gate prevents accidental reopening or overwriting of accepted runtime evidence.
- Plugin, collaboration, remote, and delegated-task GUI work are separate waves because they touch shared `devil-app`, `devil-ui`, `devil-desktop`, and evidence files and must not run concurrently.
- GA operations and release/update/rollback/incident evidence is split from runtime GUI code so final documentation can reflect the actual Phase 8 GUI outputs instead of aspirational claims.
- Final acceptance is its own wave and may update roadmap/state only after full gates, GUI Phase 8 evidence checks, platform parity evidence, and required artifacts pass.
- Plan critique was run in-process. No subagents were spawned because this runtime only permits multi-agent spawning when explicitly requested by the user.

## Plan Structure

- **Plan 08-01 (Wave 1)**: GUI Phase 8 Governance And Evidence Gate -- add a GUI Phase 8 evidence path distinct from accepted legacy Phase 8 substrate evidence.
- **Plan 08-02 (Wave 2)**: Plugin Management And Contribution GUI Workflow -- expose plugin management and command contribution views without weakening sandbox, capability, quota, or metadata-only audit boundaries.
- **Plan 08-03 (Wave 3)**: Collaboration Presence And Shared Proposal GUI Workflow -- make collaboration presence, reconnect, conflict, and shared proposal review usable through app-owned collaboration/proposal authority.
- **Plan 08-04 (Wave 4)**: Remote Workspace Manager And Remote Status GUI Workflow -- expose remote session, terminal/LSP/status, reconnect/offline, and remote proposal review metadata through app-owned remote authority.
- **Plan 08-05 (Wave 5)**: Delegated Task Command Center -- surface bounded plan rows, trust gates, proposal-preview links, blockers, approvals, and audit readiness without autonomous apply.
- **Plan 08-06 (Wave 6)**: GA Release Update Rollback Incident Evidence -- document and evidence cross-platform release, update, rollback, canary, incident, and parity procedures.
- **Plan 08-07 (Wave 7)**: Phase 8 GUI Evidence Capture And Acceptance Gate -- archive final evidence, run all gates, update planning status, and mark GUI Phase 8 accepted only with proof.

## Auto-Refine Summary

The `--auto-refine` pass identified and addressed these planning risks before finalization:

1. Legacy Phase 8 substrate evidence is already accepted and must not be reused as GUI Phase 8 acceptance. Plan 08-01 creates a separate GUI Phase 8 gate and forbids edits under `plans/evidence/phase-8/`.
2. Plugin GUI could accidentally imply direct plugin authority in UI/desktop. Plan 08-02 keeps plugin commands routed through `AppComposition::invoke_plugin_command` and requires capability/quota/audit evidence.
3. Collaboration GUI could overclaim mutation behavior. Plan 08-03 requires shared proposal review and conflict/reconnect metadata, with editor mutation only through app/editor authority.
4. Remote GUI could confuse remote session views with local disk/editor ownership. Plan 08-04 requires descriptor-only status rows and proposal-mediated remote mutation evidence.
5. Delegated task command-center wording could imply autonomous execution. Plan 08-05 requires `DelegatedTaskRuntimeActivationState::NotEncoded`, proposal-preview links, approval gates, blockers/refusals, and explicit no-autonomous-apply evidence.
6. GA claims could be made from docs alone. Plan 08-07 blocks acceptance on plan result files, evidence artifacts, CLI/xtask GUI Phase 8 checks, targeted tests, smoke commands, full repository gates, and platform parity evidence.
