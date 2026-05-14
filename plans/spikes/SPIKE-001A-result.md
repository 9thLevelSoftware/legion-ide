# SPIKE-001A Result — Native Shell + Text Model

Status: Accepted

Accepted at: 2026-05-14T02:07:05Z

## Scope

- Native shell state is represented as projection-only layouts, explorer projections, active buffer projections, status messages, and command dispatch intents.
- Editable text operations are owned by `EditorEngine` through buffer IDs and transaction descriptors.
- Open/save operations route through `WorkspaceActor` identity and file authority.
- `EditorSession` remains compatibility-only and is not owned by new UI/application constructors.
- No AI, memory, agents, embeddings, provider adapters, semantic index, or unrestricted plugin runtime is part of this spike result.

## Acceptance commands

| Command | Evidence | Result |
|---|---|---|
| `cargo run -p xtask -- check-deps` | `../evidence/phase-0/check-deps.txt` | Passed |
| `cargo fmt --all --check` | `../evidence/phase-0/fmt-check.txt` | Passed |
| `cargo check --workspace --all-targets` | `../evidence/phase-0/cargo-check-workspace-all-targets.txt` | Passed |
| `cargo test --workspace --all-targets` | `../evidence/phase-0/cargo-test-workspace-all-targets.txt` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | `../evidence/phase-0/cargo-clippy-workspace-all-targets.txt` | Passed |

## Findings

- The shell no longer owns direct editor session state; it emits command intents and accepts projection snapshots.
- Application edit commands route to `EditorEngine::apply_edit`.
- Save operations route through `EditorEngine::request_save` and `WorkspaceActor::write_file_text`.
- Workspace tree snapshots feed explorer projection models.
- Observability event sinks validate schema, severity, retention, and redaction metadata while redacting source text when metadata-only retention is required.
- Security, path-boundary, watcher-recovery, storage reload/corruption, DTO, editor atomicity, and app integration tests all passed.

## Latency notes

- p50: the current accepted non-ignored harness does not emit p50 in captured output; this is a reporting reservation for the renderer-backed harness.
- p95: `ci_typical_edit_latency_on_budget_sized_file` asserts p95 below 250ms and passed in the global test run.
- Undo/redo: `ci_undo_redo_burst_small_deterministic_sample` asserts undo and redo totals below 500ms and passed in the global test run.
- Heavy ignored benchmark output is archived in `../evidence/phase-0/editor-performance-suite.txt` and records large-file/retained-history follow-up requirements.

## Platform caveats

- Windows 11 is the validated platform for this evidence set.
- GPU utilization, compositor frame variance, native IME, native clipboard, native focus, and accessibility tree validation are renderer-integration follow-ups.
- `devil-platform` remains OS-bound and is not the owner of window state, editor state, domain models, or request routing.

## Fallback criteria

- If renderer-backed p95 input-to-paint exceeds the accepted budget, keep UI projection-only and defer renderer expansion until a bounded draw pipeline is proven.
- If native IME/clipboard/focus/accessibility integration regresses, route those features through platform-specific adapters while preserving application command-intent boundaries.
- If large-file editing exceeds memory budgets, keep current full-cache protection and require degraded/streaming mode before enabling 100MB-class editing.

## Owner signoff

- UI/runtime owner: accepted with renderer reservations.
- Editor/text owner: accepted with large-file and retained-history benchmark reservations.
- Workspace/platform/security owner: accepted.
- Architecture owner: accepted for Phase 0 closure.

## Decision

PASS WITH RESERVATIONS
