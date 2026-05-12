# ADR-0010: Define Air-Gap Mode and Outbound Network Enforcement Model

## Status
Accepted

## Context
Air-gap mode is a core privacy promise. It must be a runtime-enforced policy profile, not a configuration flag that can be silently overridden by a provider adapter or agent workflow.

## Decision
Air-gap mode disables all outbound network calls except explicitly allowed local loopback for local model endpoints. It disables hosted telemetry, update checks, cloud providers, hosted embeddings, and remote gateway calls. The UI must display persistent air-gap status. The Policy Engine enforces this at the router and Action Broker layers. Agent workflows cannot invoke network-capable commands unless separately sandboxed and approved.

## Consequences
- **Positive**: Strong, verifiable privacy boundary for sensitive environments.
- **Positive**: Enforcement at multiple layers makes bypass difficult.
- **Negative**: Users in air-gap mode lose cloud model access and hosted services; UX must make this clear.
- **Negative**: Local model loopback exceptions must be narrowly scoped to prevent tunneling.
