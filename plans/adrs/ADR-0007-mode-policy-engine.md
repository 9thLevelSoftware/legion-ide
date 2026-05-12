# ADR-0007: Define Mode Policy Engine and Action Broker Capability Model

## Status
Accepted

## Context
AI agency must be granularly controlled. The system must support User-Driven, AI-Assisted, and Semi-Automated Agent modes with explicit policy enforcement rather than scattered conditional checks.

## Decision
Represent mode policy as structured data with dimensions: context access, mutation access, execution access, network access, approval model, and memory access. All AI actions pass through the Action Broker, which consults the Policy Engine. The AI Orchestrator requests capabilities; the broker grants, denies, redacts, or escalates for approval.

## Consequences
- **Positive**: Policy is inspectable, testable, and versionable.
- **Positive**: Prevents AI from self-policing; enforcement is external and deterministic.
- **Negative**: Policy schema evolution must be backward-compatible or explicitly migrated.
- **Negative**: Fine-grained policies may create UX friction; approval UX must be fast and clear.
