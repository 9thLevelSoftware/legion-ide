# Project Architecture Rules (Non-Obvious Only)

- Architecture changes must preserve proposal-mediated mutation: UI/app cannot directly clobber disk; workspace writes require version/fingerprint preconditions and fail-closed fallback semantics.
- `devil-protocol` is the boundary crate for cross-domain IDs/DTOs/ports; changing boundary symbols requires updating `plans/dependency-policy.md` because `xtask` validates their presence literally.
- The dependency-policy gate is incomplete by design and has hardcoded rules; adding runtime behavior to placeholder crates should include a policy expansion, not just a Cargo dependency.
- New LSP, terminal, plugin, filesystem/VFS, and workspace-state runtime surfaces are ADR/phase-gated; do not implement them as ad hoc helpers in app/UI crates.
- Future AI remains isolated until editor/workspace proposal and observability paths are complete; generated edits should enter as `WorkspaceProposal`, provider activity should emit redacted events.
- Store replay/audit metadata, not full source snapshots by default; observability defaults to metadata-only retention and redaction.
