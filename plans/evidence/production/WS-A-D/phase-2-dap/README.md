# Phase 2 — Real DAP evidence

**Current cut line:** Microsoft DAP wire + fake-adapter CI green; persistent live
session with non-blocking continue and desktop auto-poll; optional system
adapter handshake dogfood. Human windowed GUI journal and full system debugee
launch/step remain residual.

**Implementation homes:**

- `crates/legion-debug` — framing, live session, adapter resolve, fake adapter
- `crates/legion-app` — `DebugWorkflow` (trust, dual-mode, poll worker)
- `crates/legion-desktop` — dual-mode banners, frame auto-poll, debug toolbar

## Packets (B0–B11)

| File | Role |
| --- | --- |
| `../../../../adrs/ADR-0044-dap-client-architecture.md` | Architecture decision |
| `B0-adr-0044-proposal.md` | B0 ADR packet |
| `B1-framing-fake-adapter.md` | Content-Length + fake adapter CI |
| `B2-breakpoints-stack-step.md` | Breakpoints / stack / step |
| `B3-resolution-trust-dual-mode.md` | Resolve, trust deny, dual-mode banner |
| `B4-microsoft-dap-codec.md` | Microsoft DAP codec |
| `B5-persistent-live-session.md` | Persistent live handle |
| `B6-continue-stop.md` | Continue-until-stop + disconnect |
| `B7-nonblocking-continue-poll.md` | Non-blocking continue + `:debug-poll` |
| `B8-desktop-auto-poll.md` | Frame auto-poll |
| `B9-system-adapter-dogfood.md` | Optional system handshake dogfood |
| `B10-headless-continue-auto-poll.md` | Headless continue→auto-poll dogfood |
| `B11-debug-controls-honesty.md` | Debug toolbar + residual honesty |
| `B12-live-prebuild-cargo.md` | Cargo prebuild before non-fake live launch |
| `B13-system-launch-step-dogfood.md` | Optional system launch+step dogfood |

## Residual

- Human windowed GUI dogfood journal
- Sandbox wrap of adapter spawn (Phase 3 deferred)
