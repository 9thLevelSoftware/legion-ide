# Legion IDE — Roadmap

<!-- Rendered by `legion start --finalize` from intake session itk_20260808-035125463. -->
<!-- This file is a view of .legion/project/requirements. Edit the requirements, not this file. -->

## Overview

A secure IDE built entirely in Rust which various levels of configurable automation, from entirely manual to fully automated workflows.

**Problem.** Nearly every semi-popular IDE on the market right now is a) a fork of VSCode and b) more AI than IDE.  This application looks to bridge the gap.

**Who has it.** Most seasoned software engineers

**Done looks like.** The application functions flawlessly in full manual mode and successfully incorporates configurable levels of AI tools through its four levels of automation.

## Non-Goals

- VS Code extension host sidecar / runtime extension execution
- Remote development (SSH, devcontainers, cloud workspaces)
- Real-time collaboration / shared editing
- Admin policy management console
- Marketplace / extension store
- Proprietary next-edit prediction models
- Multi-language LSP parity beyond Rust-first

## Constraints

- Rust-only codebase, edition 2024, MSRV 1.92
- egui/eframe 0.34 for desktop rendering
- No hosted/cloud egress without explicit user consent
- All AI provider interactions are metadata-only by default
- Every file mutation goes through the proposal pipeline
- Sandbox enforcement gaps must be honestly reported per-platform
- Terminal output must be redacted before projection
- Plugin WASM modules execute in wasmtime sandbox with capability limits
- Dependency policy enforced via xtask check-deps
- 21 standing gates must remain merge-blocking

## Phases

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 1 | The application must be able to facilitate the writing and… | req_the-application-must-be-able-to-facilitate-the-w-1 | Pending |
| 2 | All automated and AI-assisted operations must be sandboxed… | req_all-automated-and-ai-assisted-operations-must-be-2 | Pending |

## Phase 1: The application must be able to facilitate the writing and refining of code in all languages/file formats.

**Requirement:** `req_the-application-must-be-able-to-facilitate-the-w-1` (must, behavior)

**Artifact:** `.legion/project/requirements/req_the-application-must-be-able-to-facilitate-the-w-1.json`

**Acceptance criteria**

- [ ] All file types related to coding can be created/edited.
  - `cargo test --workspace -F desktop -- file_type` must exit 0

## Phase 2: All automated and AI-assisted operations must be sandboxed behind trust boundaries and require explicit human approval before mutating the main workspace.

**Requirement:** `req_all-automated-and-ai-assisted-operations-must-be-2` (must, security)

**Artifact:** `.legion/project/requirements/req_all-automated-and-ai-assisted-operations-must-be-2.json`

**Acceptance criteria**

- [ ] Delegated agent tasks run in isolated git worktree sandboxes and cannot write to the main workspace without an approved proposal.
  - `cargo test -p legion-sandbox -- escape` must exit 0
