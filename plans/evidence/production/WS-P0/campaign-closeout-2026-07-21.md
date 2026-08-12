# WS-P0 product-wiring campaign close-out

**Date:** 2026-07-21  
**Scope:** Product wiring / truth repair (not full deferred surfaces: VSIX host, remote UX, collab admin, signed installers).

## Delivered slices (evidence)

| Slice | Evidence |
| --- | --- |
| Tier 0 ledger / docs honesty | `T0-A-ledger-claim-repair.md` |
| Tier 0 simulated UI cut lines | `T0-B-honest-simulated-ui.md` |
| Tier 0 smoke promotion criteria | `T0-D-smoke-promotion-criteria.md` |
| Tier 0 synthetic gate honesty | `T0-E-synthetic-gate-honesty.md` |
| T1 editor keys + clipboard | `T1-A1-A2-editor-keys-clipboard.md` |
| T1 storage / watcher / terminal | `T1-A8-A10-A11-storage-watcher-terminal.md` |
| T2 delegate UI + keyring | `T2-delegate-ui-keyring.md` |
| T2 assist real provider | `T2-assist-real-provider.md` |
| T2 BYOK UI + sandbox report | `T2-byok-ui-sandbox-enforcement.md` |
| T2 delegate chat + LSP URI paths | `T2-delegate-chat-lsp-uri.md` |
| T2 local-first provider preference | `T2-local-first-provider-preference.md` |
| T2 product AI streaming | `T2-product-ai-streaming.md` |
| T2 progressive SSE + live sink | `T2-progressive-sse-live-stream.md` |
| T3 DAP honest cut line | `T3-dap-honest-cut-line.md` |
| Background Delegate chat worker | this close-out (+ USER_GUIDE) |

## Explicitly deferred (cut lines remain)

| Item | Why deferred |
| --- | --- |
| Real DAP adapter process / wire protocol | Large product surface; simulated cut line shipped |
| Windows FS/network sandbox isolation; Linux network Landlock | OS enforcement work beyond honesty labels |
| Assist proposal fully off UI thread | **Shipped** as polish follow-on: live Assist streams on a worker; proposal registers on `poll_product_ai_stream` (fixture/Deterministic stays sync for tests) |
| Signed installers / cargo-dist / update server | WS17 release track |
| VSIX runtime, collab transport, SSH remote UX | Ledger deferred rows |

## Verification posture

Local targeted suites exercised throughout the campaign (assist, control trust, delegated, debug, LSP read-side, desktop input/sandbox/provider keys, docs-hygiene, claim-audit, no-egui-textedit). Full standing 20-gate matrix remains the merge authority per `AGENTS.md`.
