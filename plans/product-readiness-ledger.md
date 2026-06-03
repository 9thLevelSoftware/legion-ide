# Legion IDE Product Readiness Ledger

This ledger is the product-readiness track for Legion as an enterprise AI-native IDE with required VS Code extension compatibility. It is separate from substrate phase acceptance: a phase can prove architectural safety without proving that the product is ready for daily IDE use.

## Product Target

- Primary v1 customer: enterprise teams that need local or self-hosted AI, auditability, security policy, remote and team workflows, and low-latency native editing.
- Required compatibility target: VS Code extension compatibility staged through manifest ingestion, contribution mapping, activation routing, isolated extension-host planning, and later policy-gated sidecar execution.
- Non-negotiable controls: native Rust core, projection-only UI shell, proposal-mediated mutation, metadata-only defaults, default-deny capability policy, and opt-in cloud or hosted egress.
- Compatibility references: VS Code extension manifest, contribution points, and extension-host model; LSP 3.17; Debug Adapter Protocol.

## Gate Rules

- "Accepted" substrate evidence under Phases 0-8 does not mean the user-facing IDE surface is product-ready.
- Product-ready claims require an explicit gate in this file, current evidence references, passing targeted tests, and a working UX path.
- Extension compatibility starts metadata-only. Runtime extension host sidecars, webviews, notebooks, custom editors, extension storage, and marketplace execution remain deferred until separate ADR, policy, sandbox, and test evidence exists.
- AI may propose, explain, plan, and verify, but workspace edits remain proposal-mediated unless a later signed enterprise policy explicitly enables autonomous apply.

## Readiness Matrix

| Track | Gate | Acceptance Criteria | Current Status | Evidence Required |
| --- | --- | --- | --- | --- |
| Performance, UI, accessibility | PR-UI-001 renderer latency and input accessibility | p50/p95 input-to-paint budgets, IME, clipboard, focus, high contrast, screen-reader projection, font sizing, keyboard remapping, floating windows, and multi-monitor restore are tested against renderer-backed workflows. | Not started | Renderer latency harness, accessibility smoke evidence, platform parity runs |
| Performance, UI, accessibility | PR-UI-002 large workspace behavior | 100MB degraded-mode file path, large tree open, workspace search, diagnostics fanout, extension load, and AI context retrieval do not block typing. | Not started | Performance suite with release-blocking budgets |
| Language, debug, test, SCM | PR-LANG-001 Rust language workflow | Rust LSP supervision covers completion, hover, diagnostics, rename, format, code actions, semantic tokens, folding, project-wide errors, and structural search through protocol DTOs. | Not started | LSP contract tests and Rust smoke workspace |
| Language, debug, test, SCM | PR-LANG-002 debug, tests, and SCM | DAP boundary, debugger projections, breakpoint reliability, zero-config Rust debug, test explorer, Git and jj views, diffs, conflicts, review comments, and local history are product workflows. | Not started | DAP/test/SCM contract tests and GUI smoke evidence |
| AI control plane | PR-AI-001 inspectable local-first AI | Local and self-hosted providers are real first-class paths; cloud providers are opt-in. Context manifests expose files, symbols, diagnostics, terminal excerpts, memory, privacy labels, and egress status before invocation. | Not started | Provider policy tests, context manifest tests, no-egress evidence |
| AI control plane | PR-AI-002 proposal safety and evals | Streaming UI, inline diff review, semantic retrieval provenance, run ledger, cancellation, verification commands, rollback-linked proposals, and adversarial eval fixtures exist. | Not started | AI eval suite and proposal safety evidence |
| VS Code compatibility | PR-VSC-001 manifest and contribution compatibility | VSIX/package manifest ingestion, extension identity, contribution mapping, activation-event routing, enable/disable/update metadata, API coverage reporting, and compatibility diagnostics are implemented without runtime execution. | In progress | `devil-vscode-compat` tests and dependency-policy gate |
| VS Code compatibility | PR-VSC-002 isolated extension host | Node-based extension-host sidecar, `vscode` API facade, versioned RPC bridge, per-command capability checks, proposal-mediated mutation, health UX, crash blame, extension bisect, permission review, and audit logs are implemented. | Not started | Accepted ADR, sandbox tests, extension fixture matrix |
| Remote, collaboration, enterprise | PR-ENT-001 remote development UX | SSH/container connection flow, encrypted transport, reconnect/offline recovery, remote terminal/LSP/filesystem, and visible health are product workflows. | Not started | Remote reconnect and save-conflict evidence |
| Remote, collaboration, enterprise | PR-ENT-002 collaboration and admin controls | Presence, shared workspace state, CRDT/operation-log reconciliation, shared proposals, inline review, replay, provider policy, extension allowlist, audit export, retention controls, telemetry consent, raw-source vault controls, and self-hosted diagnostics are implemented. | Not started | Collaboration merge tests and admin policy denial tests |
| Packaging, licensing, release | PR-REL-001 product promise and installability | Free/local core behavior, paid enterprise controls, transparent AI data use, no hidden code egress, signed installers, auto-update/rollback, Windows/macOS/Linux parity, crash-report controls, and beta feedback capture are documented and verified. | Not started | Release runbooks, installer evidence, rollback evidence |

## Beta Acceptance Scenario

The beta loop is not accepted until a user can open a large repository, install an approved VSIX, run Rust LSP completion, ask AI for a multi-file change, inspect the context manifest, review the proposal diff, run tests, debug a failure, collaborate on review, save safely, and export audit evidence without bypassing policy or proposal gates.
