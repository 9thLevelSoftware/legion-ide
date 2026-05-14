# Project Coding Rules (Non-Obvious Only)

- Route durable writes through `SaveWorkflowService` and `WorkspaceActor::save_file_with_proposal`; callers should see rejected saves as `AppSaveOutcome::Rejected`, not thrown errors.
- Preserve proposal preconditions on save changes: expected disk fingerprint, file content version, workspace generation, buffer version, snapshot id, required `fs.write` capability, principal, and correlation id all matter.
- Do not make `devil-editor` depend on `devil-project`; cross-domain DTOs/ports belong in `devil-protocol`, and `xtask` has hardcoded enforcement beyond the markdown policy.
- UI code should mutate projections and enqueue `CommandDispatchIntent` only; app/editor/workspace own command execution, text transactions, and file authority.
- Prefer fallible `try_*` text/snapshot paths when payload size may exceed the 5 MiB full-cache budget; compatibility constructors intentionally panic on oversize content.
- New observability events must be metadata-only safe and include non-zero `CorrelationId`, non-nil `CausalityId`, and non-zero `EventSequence` or the in-memory sink rejects them.
- Keep placeholder crates implementation-free unless the matching ADR/phase gate, dependency-policy section, owner, and contract tests are added in the same change.

