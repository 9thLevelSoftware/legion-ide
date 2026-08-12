# ADR-0043: ACP Host Scope Is the Local Adapter Bridge

## Status
Accepted — WS13.T4 scope clarification.

## Context

ADR-0039 already ratifies ACP host support as part of the agent interop plan. The current implementation seam in `legion-app` owns an optional `AcpHostCommand`, records lifecycle metadata, and runs delegated-task host work through the existing sandbox/proposal envelope. What remained ambiguous was whether ACP should grow into a separate long-lived authority layer or remain inside the existing app/desktop adapter boundary.

## Decision

ACP is scoped as a local adapter bridge, not as a separate authority layer.

- `legion-app` owns configuration, launch, supervision, and failure reporting for the optional ACP host command.
- `legion-desktop` may project ACP bridge status and relay intents, but it does not own ACP state.
- External agent work still enters through the proposal/evidence envelope and the sandbox/capability broker.
- No new `legion-acp` workspace crate or UI ownership path is authorized by this ADR.

## Why this decision

1. The current implementation already treats ACP as an app-owned command seam, so the narrowest honest scope is a local bridge rather than a new daemon boundary.
2. Keeping ACP inside the existing app/desktop path preserves the projection-only UI invariant and avoids creating a second authority surface for agent launches.
3. The bridge remains easy to fail closed: if the host command fails, the app reports an error instead of inventing success-shaped state.
4. A standalone ACP service can still be introduced later if warranted, but it would need a new ADR and dependency-policy update.

## Consequences

- The app-owned `AcpHostCommand` seam can remain the implementation anchor for delegated tasks.
- ACP activity stays metadata-first and proposal-mediated.
- `legion-desktop` continues to be a projection surface, not the owner of host lifecycle state.
- Any future move to a separate ACP service or crate must be ratified explicitly before implementation.
