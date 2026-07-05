# Contributing to Legion IDE

> **Proprietary codebase.** The source in this repository is proprietary software. All rights reserved. The workspace `publish` flag is `false` (see `Cargo.toml` `[workspace.package]`). This is not an open-source project. External contributions, pull requests, and public issue reports are not accepted. This document exists for internal developers and authorized collaborators working on a private fork under the terms of their existing distribution agreement.

## Scope of this guide

This guide is for engineers and agents who already have a working internal checkout and are authorized to make changes. It covers the working agreements that are not already documented in `AGENTS.md`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`, and the operator runbook.

If you are not an authorized collaborator, please contact the project owner through the channel you were given; do not open issues or pull requests on this repository.

## Start here

Before you do anything else, read and understand:

1. `AGENTS.md` — concise agent/developer invariants and required phase gates.
2. `docs/INDEX.md` — entry point for the canonical documentation set.
3. `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` — canonical ownership rules across the UI, app composition, workspace/project, and AI/provider layers.
4. `docs/OPERATOR_RUNBOOK.md` — operator-oriented gate list and subagent execution pattern.
5. `plans/phase-status-ledger.md` — the authoritative phase status ledger. Phase 8 is **substrate accepted**; product GA / release readiness is a separate, post-substrate track and is not implied by Phase 8 substrate acceptance.

## Required local gates

Run the full local gate set before claiming code work is complete:

```sh
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

When `cargo-deny` is installed, also run the full phase-gate scripts which include `cargo deny check`:

```sh
sh scripts/run-phase-gates.sh
# or on Windows:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-phase-gates.ps1
```

See `docs/OPERATOR_RUNBOOK.md` "Local verification gates" and "Supply-chain gate prerequisite" for installation and PATH notes.

## Working agreements

- **Respect authority boundaries.** Changes that cross a layer boundary (UI, app composition, workspace/project, AI/provider, etc.) must preserve the ownership rules in `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`. If a change seems to require breaking a boundary, stop and discuss before implementing.
- **Saves are proposal-mediated.** All file mutation routes through `SaveWorkflowService` → `AppComposition::save_active_buffer` → `WorkspaceActor::save_file_with_proposal`. Do not reintroduce direct writes.
- **UI is projection-only.** `legion-ui` accepts snapshots and emits typed `CommandDispatchIntent` values. Do not move editor session or text ownership back into UI.
- **Manual mode must remain tested.** Any change touching AI, worker, cloud, or trace code must keep Manual mode exclusion, proposal-only mutation, metadata-only default retention, and consent-gated raw trace path under test. See `docs/OPERATOR_RUNBOOK.md` "Safety checks".
- **Subagent execution pattern.** For non-trivial implementation tasks, follow the subagent execution pattern in `docs/OPERATOR_RUNBOOK.md` "Subagent execution pattern": dispatch one implementer subagent with exact files and commands, require a failing test first, run the task-specific gate, then dispatch spec-compliance and quality/security reviewers before committing.
- **Do not mass-edit historical rename markers in `plans/evidence/`.** Archived evidence may contain old Devil-era crate names or command transcripts; validators intentionally accept them where they are explicitly historical. See `docs/LEGION_RENAME.md`. New user-facing docs must use Legion naming.
- **Do not commit without authorization.** This codebase is proprietary. Follow your internal review and approval process before pushing.

## Status and progress

- Current repo state: see `README.md` "Current Status" and `plans/phase-status-ledger.md`.
- Substrate vs. product readiness: see `plans/phase-status-ledger.md` and `plans/product-readiness-ledger.md`.
- Historical audit snapshot: `ENGINEERING_STATUS.md` is a **historical** record of the 2026-06-03 audit cycle, not a current status report.

## Licensing

By making changes to this repository, you confirm that your changes are made under the same proprietary terms that govern the rest of the codebase (see `Cargo.toml` `[workspace.package] license = "Proprietary"` and any accompanying `LICENSE` file or internal distribution agreement). Do not add or rely on open-source license headers; this is not an open-source project.
