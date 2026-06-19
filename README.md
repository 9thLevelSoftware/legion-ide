# Legion IDE

> **License notice:** This codebase is proprietary software. All rights reserved. The source in this repository is provided for internal development and review only; it is not open source, not OSI-licensed, and the workspace `publish` flag is `false` (see `Cargo.toml` `[workspace.package]`). Do not redistribute, sublicense, or treat the contents as permissively licensed. See `LICENSE` (or your internal distribution agreement) for the terms that govern use of this code.

Legion IDE is a control-first, AI-native Rust IDE substrate that keeps human authority, proposal review, and metadata-only evidence at the center of local and delegated development workflows.

The current codebase is a Rust workspace that validates the core architecture for editor state, workspace mutation, projection-only UI, desktop rendering, local/hosted AI boundaries, workflow orchestration, and evidence gates.

## Current Status

Legion is not yet a general-availability desktop product. The current repo is best understood as a validated substrate with explicit phase gates and known productization cut lines.

Use these docs first:

- `AGENTS.md` — concise agent/developer invariants and required gates.
- `docs/INDEX.md` — entry point for the canonical documentation set.
- `docs/USER_GUIDE.md` — end-user guide for the current product paths.
- `docs/KEYBOARD_REFERENCE.md` — projected shortcut labels that are currently surfaced by the product UI.
- `docs/TROUBLESHOOTING.md` — diagnostics bundle guidance for smoke, package, and release failures.
- `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` — canonical ownership rules (UI projection-only, app composition, workspace authority, AI/provider boundary).
- `docs/SECURITY.md` — public-facing security model, egress policy, plugin isolation, and disclosure policy.
- `docs/LEGION_PIVOT.md` — product direction and pivot context.
- `docs/MODES.md` — Manual, Assist, Delegate, and Legion Workflow mode boundaries.
- `docs/OPERATOR_RUNBOOK.md` — operator-oriented gate/runbook notes.
- `plans/product-readiness-ledger.md` — readiness matrix and remaining product gaps.
- `plans/legion-production-master-plan-v0.2.md` — the current production master plan (current-state rebaseline, 2026 market/technology comparison, product-workflow gaps, workstreams, milestones to production utility).
- `plans/legion-production-master-plan-v0.1.md` — historical production master plan retained for audit traceability; do not treat its current-state assessment as authoritative without checking the v0.2 rebaseline and product-readiness ledger.
- `plans/legion-customizable-autonomy-continuation-plan-v0.1.md` — continuation plan: an old-school customizable manual IDE with a configurable autonomy continuum from zero AI to gated full automation (ADR-0041..0045, WS-21..WS-29, milestones C1..C5).
- `.almanac/pages/getting-started.md` — local Almanac wiki entry point, if the wiki is checked out locally.

## Architecture at a Glance

- `legion-protocol` defines DTOs and shared contracts.
- `legion-app` is the composition root and owns application authority.
- `legion-ui` is projection-only: it accepts snapshots and emits typed `CommandDispatchIntent` values.
- `legion-desktop` is the eframe/egui renderer edge and must not own product state.
- `legion-editor` and `legion-text` own buffer, snapshot, degraded-mode, and text-edit behavior.
- `legion-project` owns trust-aware workspace/VFS behavior and proposal-mediated file mutation.
- `legion-agent`, `legion-ai`, `legion-ai-providers`, `legion-index`, `legion-memory`, and `legion-tracker` contain active, gated behavior and tests.

## Required Local Gates

Run these before claiming code work is complete:

```bash
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
```

The full phase-gate scripts also run cargo-deny when installed locally:

```bash
sh scripts/run-phase-gates.sh
# or on Windows:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-phase-gates.ps1
```

### Supply-chain gate prerequisite

The full phase-gate scripts require `cargo-deny` locally:

```bash
cargo install cargo-deny --locked
cargo deny --version
```

If `cargo deny --version` is not found immediately after installation, ensure Cargo's binary directory, usually `$HOME/.cargo/bin`, is on `PATH`.

CI runs cargo-deny through `EmbarkStudios/cargo-deny-action` on the Linux matrix leg, so local developer machines must install the CLI separately before using `scripts/run-phase-gates.*`.

## CLI Proof

The current CLI proof opens a trusted workspace and supports only `:w` and `:q`:

```bash
cargo run -p legion-app -- .
```

This is not the full renderer-backed desktop product.

## Desktop / GUI Evidence

The desktop crate is `legion-desktop`. GUI phase evidence and limitations live under:

- `plans/evidence/gui-productization/`
- `audit-reports/`

Do not infer production GUI readiness from substrate tests alone; check the product readiness ledger and known limitations.

## Historical Devil Naming

This repository was renamed from its previous Devil-era product identity to Legion IDE. Current user-facing docs should use Legion naming. Archived evidence may still contain historical Devil-era markers, and validators intentionally accept historical markers when checking archived evidence. See `docs/LEGION_RENAME.md`.

## Repository Hygiene

Generated build outputs, local IDE state, Hermes local workspaces, and local Almanac runtime databases should not be committed. Durable reports belong under `audit-reports/` or `plans/`; local working memory belongs in `.hermes/`, `.serena/`, or `.almanac/` according to the current project policy.
