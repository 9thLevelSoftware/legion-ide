# 02: Ratify the Windows/Rust pilot configuration

Status: resolved

**What to build:** Pin one real Legion workspace, OS, toolchain, rust-analyzer, debugger, hardware and native-input route; publish prerequisites and frozen pilot scenarios.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 02.

**Traceability:** M0; S0-02; S2-01. **Verification target:** A01/A16.

## Acceptance criteria

- [x] Pin one real Legion workspace, OS, toolchain, rust-analyzer, debugger, hardware and native-input route.
- [x] Publish prerequisites and frozen pilot scenarios.
- [x] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [x] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.

- 2026-10-08: Bounded configuration draft published in [pilot configuration](../../../plans/completion/ide-2026-pilot-configuration.md); based on c2a6578, fast-forwarded integration 87580fa. Owner selections and outside-repo prerequisite evidence consumed; canonical acceptance unchanged. Independent review pending; no commit. Planned docs-hygiene, completion-register validation and diff whitespace check once each. Candidate package build remains coordinator-owned and unverified here.

- 2026-10-08 verification: `cargo run -p xtask -- docs-hygiene` exit 0, documentation hygiene checks passed; `cargo run -p xtask -- verify-completion-register --root .` exit 0, structure only (143 implemented / 233 partial / 43 absent; all 419 acceptance values unassessed). Each ran once. Candidate evidence updated from external pilot-candidate-87580fa.md: build/package exit 0, MSI digest pinned, package verifier exit 1; search and offline Assist smoke failures remain downstream prerequisites. No runtime checks repeated; independent review pending and no commit.
- 2026-10-08: `git diff --check` exit 0; tracked patch whitespace check passed (Git emitted an LF-to-CRLF normalization notice). New configuration document is untracked and is not covered by that Git check; docs-hygiene passed. Review-ready, independent PASS still pending.
- 2026-10-08 review P2 fixed: explicitly map SC-MANUAL-OFFLINE-30M-WIN to CFG-WIN11-X64-MANUAL-OFFLINE; preserve separately labelled installed offline artifact/hash/inventory and pre-launch whole-process-tree OS network-capture prerequisites, including blocked outcomes. Focused PowerShell canonical reference/prerequisite check passed once; no successful validators repeated. Fast-forwarded integration 9a024a4 before handoff; review clearance pending, no commit.

- 2026-10-08: Independent reviewer PASS; sole P2 offline configuration mapping resolved. Bounded configuration deliverable accepted and ticket02 resolved; runtime/package/native prerequisites remain as documented, canonical acceptance unchanged. No successful checks repeated. Coordinator authorized commit of only the two owned files and fast-forward integration.

