# ADR-0044: DAP Client Architecture (Real Adapter Path)

## Status

**Accepted** — WS-A-D Phase 2 B0–B3 (2026-07-21).  
B1–B2 landed live framing + fake adapter; B3 lands resolution/trust/dual-mode. Dependency-policy authorizes `serde`/`serde_json` + std process (no `legion-platform` edge yet).

## Context

Legion already ships:

- Protocol DTOs for debug sessions, breakpoints, stack frames, variables, watches, console, and audit records (`legion-protocol`).
- A **metadata-only / simulated** DAP client in `crates/legion-debug/src/dap.rs` that never spawns an adapter and never speaks the DAP wire protocol (honest cut line: `plans/evidence/production/WS-P0/T3-dap-honest-cut-line.md`).
- App/desktop projections that surface debug UI with a **SIMULATED DAP** banner.
- Kanban tasks `P2.F3.T1`–`T3` (real adapter lifecycle, CodeLLDB resolution, breakpoint hit).
- WS-A-D campaign charter: `plans/evidence/production/WS-A-D/campaign-charter.md`.

LSP precedent (`ADR-0034`, `ADR-0018`) shows the preferred pattern: **hand-rolled stdio JSON-RPC**, supervised process lifecycle, protocol DTOs only at the app boundary, no UI ownership of process state, and CI that does not require third-party binaries.

Dependency policy today allows `legion-debug` → `legion-protocol` **only**, and explicitly forbids platform/process until a later gate adds policy, sandbox/process evidence, and contract tests (`plans/dependency-policy.md` § `legion-debug`).

## Decision

Legion will implement a **real DAP client** owned by `legion-debug`, composed by `legion-app`, projected by `legion-desktop`, with dual paths:

1. **Fixture path** (default for CI / untrusted / missing adapter) — keep simulated metadata behavior and honest labels.
2. **Live path** (opt-in when adapter binary resolves and workspace is trusted) — spawn adapter process, speak DAP over stdio JSON-RPC, map events into existing protocol DTOs.

### 1. Ownership boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| `legion-debug` | Framing, request correlation, adapter process lifecycle (when authorized), session state machine, mapping DAP messages → protocol DTOs / audit summaries | Editor buffers, workspace VFS, UI, proposal apply |
| `legion-app` | Trust/capability checks before live launch, session selection, projection assembly, composition of `legion-debug` | Raw adapter I/O ownership long-term (may hold ports) |
| `legion-desktop` | Debug panel projections, user actions → intents | Adapter process, DAP wire state |
| `legion-security` / policy | Launch allow/deny, principal + trust + capability | Process pipes |
| `legion-sandbox` (later) | Optional wrap of adapter spawn (Phase 3) | DAP protocol semantics |

UI remains **projection-only** (`ADR-0030`). Debug mutations that edit source (e.g. set variable → write) if ever supported must be **proposal-mediated** (`ADR-0016`); v1 live DAP is **inspect/control only** (launch, breakpoints, step, continue, disconnect).

### 2. Transport and framing

- **Stdio** transport first (DAP default for local adapters).
- Framing: `Content-Length` headers + JSON body (same family as LSP).
- Prefer reusing framing patterns from `legion-lsp` where practical (shared internal helper module later is OK; do not force a premature shared crate).
- Optional TCP later is out of B0–B2 scope.

### 3. Protocol surface (minimum viable product)

| Direction | Messages (minimum) |
| --- | --- |
| Client → adapter | `initialize`, `launch` and/or `attach`, `setBreakpoints`, `configurationDone`, `threads`, `stackTrace`, `scopes`, `variables`, `continue`, `next`, `stepIn`, `stepOut`, `pause`, `disconnect` / `terminate` |
| Adapter → client | `initialized`, `stopped`, `continued`, `terminated`, `exited`, `output` (console projection, redacted), `breakpoint` |

Do not implement the full DAP catalog before one end-to-end adapter works (`P2.F3.T1` stop condition).

### 4. Process lifecycle and supervision

Inspired by `ADR-0018`:

- Spawn only after **app-level** trust + capability grant.
- Supervised kill on: session end, timeout, workspace close, drop of runtime handle, circuit-breaker after N crashes.
- No orphan adapters: tests must assert process exit.
- Correlation/causality/event sequence on audit records non-zero (existing observability rules).

### 5. Security and trust

- **Untrusted** workspaces: live launch **denied**; fixture path may still project simulated UI with honest banner.
- Capability id (illustrative): `debug.adapter.launch` — must be deny-by-default and recognized by broker (same class of fix as Assist `ai.provider.invoke`).
- Adapter binary resolution (CodeLLDB / `lldb-dap`): path config, `PATH`, documented install; **no** “trust all adapters” switch (`P2.F3.T2` stop condition).
- Adapter stdout/stderr: metadata + bounded console rows; raw source payloads stay out of default retention (align with redaction).
- Network: adapters may open ports for debugee; policy should document residual risk until sandbox wrap (Phase 3 optional B3.5).

### 6. CI strategy: fake adapter

- Ship a **fake DAP adapter** binary (workspace crate or `legion-debug` test bin) that speaks enough DAP to exercise initialize → setBreakpoints → stopped → stackTrace → continue → disconnect.
- CI **always** uses the fake adapter (or fixture path).
- Real CodeLLDB / lldb-dap: optional local job, dogfood journal, never required on standing gates.

### 7. Feature / path selection

Recommended (names illustrative):

- Default product: try live if a **Legion-compatible** adapter resolves **and** trust allows; else fixture.
- Env override: `LEGION_DAP_MODE=fixture|live|auto` for operators and tests (`live` fails closed — no fixture stack after a live request).
- Wire note (B4): live substrate uses **Microsoft DAP** envelopes (`seq`/`type`/`command`/`arguments`) over `Content-Length`. Contract coverage is the in-tree `fake_dap_adapter` (same shape as CodeLLDB / `lldb-dap`). PATH auto-resolution of vendor adapters is allowed when mode is `auto`/`live`.
- Desktop banner:
  - Fixture / simulated: keep **SIMULATED DAP** (or equivalent honesty).
  - Live connected: clear “live adapter” status (no simulated claim).
  - Live failed: explicit unavailable reason, no fake stack frames.

### 8. Dependency-policy activation (required before B1 code)

Before process spawn lands in `legion-debug`, update `plans/dependency-policy.md` to authorize (in a later PR with this ADR accepted):

- `legion-debug` → `legion-platform` (or minimal process spawn surface) for supervised child processes.
- Optionally `legion-debug` → `legion-security` only via **app-owned** checks first (prefer app broker; avoid circular policy ownership).
- Contract tests + evidence packet under `plans/evidence/production/WS-A-D/phase-2-dap/`.

Until that update, B1 code must not land.

### 9. Mapping to protocol DTOs

- Prefer existing `Debug*` types; extend only with ADR-noted schema bumps and contract tests.
- `DebugAdapterAuditRecord` remains metadata-first.
- Breakpoint `verified` must reflect adapter response, not unconditional `true` (today’s fixture sets simulated verified).

### 10. Non-goals (this ADR)

- Full multi-language multi-adapter marketplace.
- Remote debug / embedded / flash.
- Test explorer (`P2.F3.T4`) — separate feature.
- VSIX / VS Code debug adapter host.
- Perfect Windows sandbox of debugee (Phase 3).

## Consequences

### Positive

- Users can actually stop in Rust when an adapter is installed.
- CI stays hermetic via fake adapter / fixture dual path.
- Aligns with LSP supervision lessons.

### Negative / costs

- Process and wire complexity in `legion-debug`.
- Dependency-policy expansion.
- Residual security surface of native debug adapters.

### Follow-on slices (WS-A-D Phase 2)

| Slice | Scope |
| --- | --- |
| **B1** | Framing + process + fake adapter e2e |
| **B2** | Breakpoints, stack, variables, step, disconnect |
| **B3** | CodeLLDB/lldb-dap resolution + untrusted deny + USER_GUIDE |
| **B3.5** | Optional sandbox wrap of spawn (after Phase 3) |

## Verification (when implementing)

```text
cargo test -p legion-debug
cargo test -p legion-app --test debug_workflow
cargo test -p legion-desktop --test debug_workflow
cargo test -p legion-desktop --test breakpoint_hit
# plus new fake-adapter integration tests
standing gates (AGENTS.md)
```

## References

- `crates/legion-debug/src/dap.rs` (current fixture)
- `plans/evidence/production/WS-P0/T3-dap-honest-cut-line.md`
- `plans/evidence/production/WS-A-D/campaign-charter.md`
- `ADR-0034-lsp-client-architecture.md`, `ADR-0018-lsp-runtime-supervision.md`
- `ADR-0030-desktop-adapter-boundary.md`, `ADR-0016-generalized-proposal-service.md`
- Kanban `P2.F3.T1`–`T3`
