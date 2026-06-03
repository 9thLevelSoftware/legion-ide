# AGENTS.md

This file provides guidance to agents when working with code in this repository.

- Phase gates: `cargo run -p xtask -- check-deps`; `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets`; `cargo clippy --workspace --all-targets -- -D warnings`; CI also runs `cargo deny check` with warning-level policy in `deny.toml`.
- Single test examples: `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`; list editor perf tests with `cargo test -p legion-editor --test performance_suite -- --list`.
- `xtask check-deps` parses `plans/dependency-policy.md` sections 1/2 and also hardcodes required deps plus forbidden `legion-editor -> legion-project`; update policy/protocol symbols and `xtask` together.
- Current UI shell is projection-only: `legion-ui` emits `CommandDispatchIntent` and accepts snapshots; do not put editor session/text ownership back into UI.
- Saves are proposal-mediated: `AppComposition::save_active_buffer` -> `SaveWorkflowService` -> `WorkspaceActor::save_file_with_proposal`; stale/conflict/denial returns `Ok(AppSaveOutcome::Rejected(_))` while preserving dirty editor text.
- Workspace saves require expected fingerprint, file content version, workspace generation, buffer version, snapshot id, and non-zero correlation/causality; non-atomic write fallback is intentionally disabled/fail-closed.
- `legion-text` snapshots materialize a full cache and line index with a 5 MiB budget; the ignored 100MB performance workload is a known degraded/streaming-mode gap, not a green benchmark.
- Observability sinks default to metadata-only redaction and reject zero `CorrelationId`, nil `CausalityId`, or zero `EventSequence`; tests assert event ordering for save/proposal flows.
- Active crates: `legion-index`, `legion-agent`, `legion-tracker`, `legion-memory`, `legion-ai`, and `legion-ai-providers` have real behavior and contract tests and are no longer inert placeholders. Still-deferred surfaces within the AI/provider stack and elsewhere remain protected. Future surfaces in any crate require an ADR/phase gate, dependency-policy entry, and contract tests before activation.
- Some architecture review docs describe earlier gaps; verify against current code/tests before repeating claims about UI owning editor state or saves bypassing proposals.
- `cargo run -p legion-app -- <path>` opens the current directory as a trusted workspace and supports only `:w`/`:q`; it is a CLI shell proof, not the real renderer.

