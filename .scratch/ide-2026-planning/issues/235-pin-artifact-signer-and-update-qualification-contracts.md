# 235: Pin artifact, signer and update qualification contracts

Status: resolved

**What to build:** Name supported package formats, clean hosts, signer/feed ownership and immutable candidate/artifact rules, preserving existing unassessed status and no-production-signer until authorized.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 235.

**Traceability:** M7; XQ-01/07. **Verification target:** A19.

## Acceptance criteria

- [x] Name supported package formats, clean hosts, signer/feed ownership and immutable candidate/artifact rules, preserving existing unassessed status and no-production-signer until authorized.
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

- 2026-10-08: Published bounded [distribution contract](../../../plans/completion/ide-2026-distribution-contract.md) after inspection at 9a024a4 and merge of integration dfcdd42. Own only this issue and contract. Planned shared xtask docs-hygiene, verify-completion-register --root ., and staged git diff --cached --check once each; independent review pending. No provisioning, code, canonical-register/status or production-acceptance changes.

- 2026-10-08 verification: D:/legion-ide/target/debug/xtask.exe docs-hygiene exit 0 (documentation hygiene passed); D:/legion-ide/target/debug/xtask.exe verify-completion-register --root . exit 0 (structure only: 143 implemented / 233 partial / 43 absent; all 419 acceptance values unassessed). Shared existing validator binary used from this worktree, each once. No runtime/release checks or acceptance inferred. Integration dfcdd42 merged; independent review active, no commit.
- 2026-10-08: git diff --cached --check exit 0, run once with both owned files staged (including the new contract); whitespace passed. Git reported LF-to-CRLF normalization notices during staging. Subsequent issue update records this result only; successful checks are not repeated. Await coordinator independent review clearance before commit.

- 2026-10-09: Coordinator reported independent ticket235 review PASS with no findings and authorized resolution, commit of only the two owned files, and fast-forward integration. Bounded documentation contract accepted; signer/feed/clean-host prerequisites and all canonical unassessed acceptance remain unchanged. Planned checks passed once as recorded above; no successful checks repeated.
