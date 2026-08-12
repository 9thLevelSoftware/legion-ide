# GUI Productization Baseline

Date: 2026-05-26

## Sources Read

- `.planning/CODEBASE.md`
- `.planning/codebase/index.jsonl`
- `.planning/codebase/symbols.json`
- `Cargo.toml`
- `crates/legion-ui/Cargo.toml`
- `crates/legion-app/Cargo.toml`
- `crates/legion-ui/src/lib.rs`
- `crates/legion-ui/src/ui.rs`
- `crates/legion-app/src/lib.rs`
- `crates/legion-app/src/main.rs`
- `crates/legion-protocol/src/lib.rs`
- `crates/legion-storage/src/lib.rs`
- `plans/phase-status-ledger.md`
- `plans/evidence/phase-8/phase-8-architecture-map.md`
- `plans/evidence/phase-8/platform-matrix-evidence.txt`
- `plans/evidence/phase-8/release-readiness-review.md`
- `plans/adrs/ADR-0002-ui-editor-rendering.md`
- `plans/spikes/SPIKE-001A-result.md`
- `plans/dependency-policy.md`
- `xtask/src/main.rs`

## Current Product Shape

The current runnable product is `legion-app`, a CLI shell proof. `crates/legion-app/src/main.rs` opens the current directory as a trusted workspace, accepts an optional file path, and supports only `:w` and `:q` in the interactive loop. This is useful as a composition proof but is not a renderer-backed desktop IDE.

`legion-app` is the authority and composition layer. Its library owns orchestration across workspace, editor, proposal, security, storage, observability, AI, plugin, collaboration, remote, and UI projection surfaces. UI-originated commands flow through `CommandDispatchIntent` and `AppCommandRequest`; they do not execute inside the UI crate.

`legion-ui` remains projection-only. Its manifest depends on `legion-protocol` and `thiserror`, not on `legion-app`, `legion-editor`, `legion-project`, or `legion-storage`. Its public surface exports `ShellProjectionSnapshot`, `CommandDispatchIntent`, active-buffer, explorer, layout, status, proposal, trust, AI, delegated-task, plugin, and collaboration projections. `Shell::handle_command` explicitly emits typed dispatch intents without mutating editor or workspace state.

The root workspace has no active `legion-desktop` member and no GUI renderer dependency. The product gap is therefore a desktop adapter and renderer integration, not a rebuild of editor, workspace, save, proposal, policy, telemetry, or provider runtime substrates.

## Preserved Invariants

- `legion-ui` must stay projection-only. It may render snapshots and emit `CommandDispatchIntent`, but it must not own editor text, workspace state, save decisions, provider state, telemetry storage, file mutation, or persistent app authority.
- Saves must remain proposal-mediated. The app save path continues through `AppComposition::save_active_buffer`, `SaveWorkflowService`, and `WorkspaceActor::save_file_with_proposal`; stale, conflict, and denial paths must preserve dirty editor text and return rejected outcomes rather than silently applying writes.
- Workspace save requests must preserve expected fingerprint, file content version, workspace generation, buffer version, snapshot id, and non-zero correlation/causality identity.
- Observability and storage remain metadata-only by default. Storage validation rejects zero correlation ids, nil causality ids, zero event sequences, and raw payload leakage in metadata-only records.
- Runtime surfaces remain gated. Placeholder or policy-gated crates must not become active through GUI convenience unless their ADR, dependency-policy entry, protocol contracts, contract tests, ownership tests, and evidence exist.
- Phase 8 substrate evidence is accepted. GUI productization starts after that acceptance and must not reuse stale ledger language that described Phase 8 as future-gated.

## Renderer And Desktop Gaps

ADR-0002 is accepted with reservations. It points at a native Rust GPU path as the primary direction but still requires renderer-backed p50/p95 input-to-paint, IME, clipboard, focus, and accessibility evidence. SPIKE-001A validated projection-only shell behavior and text-model boundaries, not a real desktop renderer.

There is no `legion-desktop` crate yet. The planned adapter must consume `ShellProjectionSnapshot`, own only renderer/window/native input resources, and route user actions back into app-owned command handling. It must never make renderer state authoritative for editor text, dirty state, proposal lifecycle, workspace persistence, provider execution, retention, telemetry, or plugin/collaboration/remote authority.

## Dependency Policy Gap

`plans/dependency-policy.md` currently describes `legion-ui` as a projection-only crate and forbids hard edges from `legion-ui` to app/editor/project/storage. It does not yet define `legion-desktop`, authorized renderer dependency categories, or a renderer-specific `xtask` gate.

Phase 1 therefore needs a policy and `xtask` update before Phase 2 can add any renderer dependency or desktop crate. The policy must authorize renderer dependencies only in the planned adapter, while `legion-ui` and core crates continue to fail closed if they gain renderer/windowing dependencies or forbidden internal edges.

## Phase 2 Entry Criteria

- ADR-0002 names the accepted renderer path, fallback triggers, and required proof obligations.
- ADR-0030 defines `legion-desktop` as an adapter boundary that may depend on app/UI/protocol and approved renderer crates while core crates do not depend on it.
- `plans/dependency-policy.md` documents the desktop adapter rule and conservative renderer dependency denial for `legion-ui`.
- `xtask` includes a renderer dependency gate preserving the projection boundary.
- `cargo run -p xtask -- check-deps`, formatting, workspace check, targeted `xtask` tests, `legion-ui` check, and `legion-app` check pass in this checkout.
- Phase 2 starts from a trusted-workspace desktop shell that renders projections and routes intents through app authority, not from a new editor/workspace owner in UI.
