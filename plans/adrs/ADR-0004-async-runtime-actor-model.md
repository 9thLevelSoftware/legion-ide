# ADR-0004: Select Async Runtime and Subsystem Actor Model

## Status
Accepted

## Context
Legion IDE runs multiple long-lived subsystems (indexing, AI orchestration, file watching, tracker) that must not block the editor core or UI thread. Shared mutable state across these subsystems would violate Rust's safety guarantees and architectural clarity.

## Decision
Use Tokio as the async runtime. Subsystems communicate via typed messages over bounded async channels. Each major subsystem owns its state in a dedicated task or small task pool. Cross-subsystem queries use request/response patterns with cancellation support.

## Consequences
- **Positive**: Mature ecosystem, excellent performance, and strong cancellation semantics.
- **Positive**: Actor-style ownership maps cleanly to Rust's ownership model.
- **Negative**: Backpressure and deadlock scenarios must be explicitly designed and tested.
- **Negative**: Debugging async message flows is harder than direct function calls; observability crate must capture causal traces.
