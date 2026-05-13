# Architecture Freeze: Devil IDE Spike 1A Prerequisites v0.1

## Status

Draft, pre-implementation freeze criteria.

## Scope

This freeze defines the minimum contractual, architectural, and sequencing conditions that must be satisfied before broad implementation for Spike 1A proceeds.

## Required Gates

### Gate 1: Dependency Direction Validation

- `devil-ai` depends on `devil-protocol` and does not depend on `devil-ai-providers`.
- `devil-ai-providers` depends on `devil-ai`.
- `devil-editor` and `devil-ui` consume protocol contracts for cross-domain interaction.
- No hard dependency `devil-editor -> devil-project` is introduced.

Evidence check:
- `crates/devil-ai/Cargo.toml` dependency list.
- `crates/devil-ai-providers/Cargo.toml` dependency list.
- `crates/devil-protocol/src/lib.rs` exposes shared contract DTOs and `ProjectInfoPort`.
- `plans/architecture-charter-v0.1.md` mermaid and rule sections updated to show interface boundary.

### Gate 2: Protocol Contract Stability

- `ProjectInfoQuery`, `ProjectInfo`, and `EditorTransactionEvent` are defined in `devil-protocol`.
- `ProjectInfoPort` is used as the contract abstraction for editor/project lookup and transaction notification.
- Changes to protocol types require review before dependent crates are extended.

Evidence check:
- `crates/devil-protocol/src/lib.rs` contains all boundary data structures and the trait.
- Plan includes a lightweight regression strategy for protocol changes.

### Gate 3: Text-Model Stress Validation

- The stress criteria for large-file edit throughput and deterministic rollback behavior are included in charter gates.

Evidence check:
- `plans/architecture-charter-v0.1.md §16.8` defines explicit validation metrics.

### Gate 4: Platform Boundary Proofing

- `devil-platform` scope is restricted to OS abstraction duties.
- Platform-boundary spike proof is documented and accepted.

Evidence check:
- `plans/SPIKE-000-platform-boundary-proof.md` populated and reviewed.
- `plans/architecture-charter-v0.1.md` keeps platform responsibilities aligned with OS-level concerns.

### Gate 5: UI Spike Dependency

- Front-end hiring or UI scaling is blocked until Spike 1A validates editor latency and rendering behavior.

Evidence check:
- `plans/architecture-charter-v0.1.md §16.1`.
 - `plans/SPIKE-001A-native-shell-proof.md` completed and accepted.

## Architecture Freeze Condition

Implementation is allowed to scale only when all five gates above are satisfied and documented. Until then:

- Keep `devil-agent`, `devil-memory`, `devil-ai-providers`, `devil-cli`, and `devil-observability` at minimal scope.
- Defer broad AI orchestration and agent expansion to approved post-freeze milestones.
