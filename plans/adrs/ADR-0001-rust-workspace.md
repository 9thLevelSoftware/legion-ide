# ADR-0001: Adopt Rust 2024 Multi-Crate Workspace and Proprietary Distribution Model

## Status
Accepted

## Context
Devil IDE is a proprietary, cross-platform development environment targeting Windows, macOS, and Linux. The product requires a clean-slate architecture with no VS Code fork or extension compatibility. The codebase must support high-concurrency, memory safety, and deterministic AI interactions. A single-crate structure would create tight coupling between UI, editor core, indexing, AI orchestration, and security boundaries.

## Decision
Adopt a Cargo workspace with Rust Edition 2024, separating the codebase into 17+ focused crates: protocol, text primitives, platform abstractions, storage, observability, security, editor core, project model, indexing engine, AI providers, AI orchestrator, agent workflows, local tracker, memory, UI shell, application binary, and CLI tooling.

## Consequences
- **Positive**: Enforces strict dependency direction, enables independent testing, supports incremental compilation, and prevents UI or provider logic from leaking into core crates.
- **Positive**: Proprietary license and `publish = false` prevent accidental open-source publication.
- **Negative**: Workspace coordination overhead increases; CI must validate cross-crate API stability.
- **Negative**: New engineers must understand crate boundaries before making changes.
