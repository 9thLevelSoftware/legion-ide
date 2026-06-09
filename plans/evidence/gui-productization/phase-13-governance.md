# Phase 13 Governance Evidence

## Scope
Defines the accepted activation boundary for Phase 13 Legion Workflow Orchestration.

## Accepted Runtime Boundary
Legion Workflow Orchestration uses metadata-only tracking via protocol DTOs.
The app composition (`legion-app`) holds execution authority over tracking dependencies, validating sign-offs, and managing proposal lifecycle states.
The agent coordinator (`legion-agent`) leverages sandbox orchestration for local workers, and explicit routes to external models.

## Forbidden Behavior
- Direct mutation of the main workspace is strictly forbidden.
- UI (`legion-ui`) and desktop (`legion-desktop`) components are projection and event-trigger layers only; they are strictly forbidden from executing workflows, invoking AI, or gaining direct control over editor states.
- Raw generated source, prompts, logs, or external provider details must not be persisted indiscriminately; tracking retains metadata exclusively by default.

## Verification
- `cargo run -p xtask -- check-deps`

## Owner Roles
- Governance planner
- Security boundary reviewer
- QA gate owner

## Markers
Autonomous merge: unsupported until approval

## Residual Risks
- Model-generated proposals require review to detect sub-optimal code paths not caught by deterministic bounds.
